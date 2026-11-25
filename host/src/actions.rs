// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Execution of the five single-step actions. All functions in this module block and
//! are meant to run on worker threads (with COM initialized for the shell functions).

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::Globalization::{CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar};
use windows::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
};

use crate::config::MacroEvent;
use crate::keycodes;
use crate::shell;
use crate::themes;

/// Values for the `{{app_name}}`, `{{window_name}}`, `{{pointer_x}}` and `{{pointer_y}}`
/// placeholders, captured when the menu was opened.
#[derive(Clone, Debug, Default)]
pub struct Placeholders {
    pub app_name: String,
    pub window_name: String,
    pub pointer_x: i32,
    pub pointer_y: i32,
}

impl Placeholders {
    pub fn apply(&self, text: &str) -> String {
        text.replace("{{app_name}}", &self.app_name)
            .replace("{{window_name}}", &self.window_name)
            .replace("{{pointer_x}}", &self.pointer_x.to_string())
            .replace("{{pointer_y}}", &self.pointer_y.to_string())
    }

    /// Replaces placeholders for URI templates, percent-encoding text values so spaces,
    /// `&`, `#`, `?` and non-ASCII characters do not break URL parsing.
    pub fn apply_uri(&self, uri: &str) -> String {
        uri.replace("{{app_name}}", &themes::encode_path_segment(&self.app_name))
            .replace(
                "{{window_name}}",
                &themes::encode_path_segment(&self.window_name),
            )
            .replace("{{pointer_x}}", &self.pointer_x.to_string())
            .replace("{{pointer_y}}", &self.pointer_y.to_string())
    }
}

/// Why an action failed. The host turns this into a localized notification.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// A file, folder or program does not exist.
    NotFound(String),
    /// Any other error with a human-readable description.
    Other(String),
    /// Simulated keys were not sent because the target window is elevated.
    TargetElevated,
}

const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_PATH_NOT_FOUND: u32 = 3;
const ERROR_BAD_PATHNAME: u32 = 161;
const ERROR_INVALID_NAME: u32 = 123;

fn system_message(code: u32) -> String {
    windows::core::Error::from(windows::core::HRESULT::from_win32(code)).message()
}

fn shell_open(target: &str, directory: Option<&str>) -> Result<(), Failure> {
    shell::shell_execute(target, None, directory).map_err(|code| match code {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_BAD_PATHNAME | ERROR_INVALID_NAME => {
            Failure::NotFound(target.to_string())
        }
        _ => Failure::Other(system_message(code)),
    })
}

/// Starts an app: a `shell:AppsFolder\<AUMID>` link, a path to an executable or
/// shortcut, or a program name registered in App Paths / found on the PATH.
pub fn launch_app(target: &str) -> Result<(), Failure> {
    let target = shell::expand_env(target.trim());
    if target.is_empty() {
        return Err(Failure::NotFound(String::new()));
    }
    let path = Path::new(&target);
    if path.is_absolute() {
        if !path.exists() {
            return Err(Failure::NotFound(target));
        }
        let directory = path.parent().map(|p| p.to_string_lossy().into_owned());
        return shell_open(&target, directory.as_deref());
    }
    shell_open(&target, None)
}

/// Opens a file or folder with its default application.
pub fn open_file(path: &str) -> Result<(), Failure> {
    let path = shell::expand_env(path.trim());
    if path.is_empty() {
        return Err(Failure::NotFound(String::new()));
    }
    if Path::new(&path).is_absolute() && !Path::new(&path).exists() {
        return Err(Failure::NotFound(path));
    }
    shell_open(&path, None)
}

/// Opens a URI with the application registered for its scheme.
pub fn open_uri(uri: &str, placeholders: &Placeholders) -> Result<(), Failure> {
    let uri = placeholders.apply_uri(uri.trim());
    if uri.is_empty() {
        return Err(Failure::NotFound(String::new()));
    }
    shell_open(&uri, None)
}

fn decode_oem(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let len = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(
            CP_OEMCP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut wide),
        );
        String::from_utf16_lossy(&wide)
    }
}

fn command_failure(code: Option<i32>, stderr: &[u8]) -> Failure {
    let text = decode_oem(stderr).trim().to_string();
    let text = if text.is_empty() {
        format!("exit code {}", code.unwrap_or(-1))
    } else {
        text.chars().take(200).collect()
    };
    Failure::Other(text)
}

/// The amount of error output which is kept for failure messages.
const MAX_STDERR: usize = 4096;

/// Reads the error output of a detached command until the command closes it. The first
/// 4 KiB (or all of it, if the output ends earlier) are sent once to `tx`. Once `stop` is
/// set, nobody waits for them any more and the rest is discarded. The pipe is drained
/// to the end (like Kando does) because closing it early makes later writes of
/// long-running commands to stderr fail.
fn read_stderr(pipe: &mut impl Read, stop: &AtomicBool, tx: Sender<Vec<u8>>) {
    let mut tx = Some(tx);
    let mut output = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let n = match pipe.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if tx.is_none() {
            continue;
        }
        if stop.load(Ordering::Relaxed) {
            tx = None;
            output = Vec::new();
            continue;
        }
        let take = n.min(MAX_STDERR - output.len());
        output.extend_from_slice(&buf[..take]);
        if output.len() >= MAX_STDERR
            && let Some(tx) = tx.take()
        {
            let _ = tx.send(std::mem::take(&mut output));
        }
    }
    if let Some(tx) = tx {
        let _ = tx.send(output);
    }
}

/// Runs a command with `cmd.exe /d /s /c` without a console window. Detached commands are
/// considered successful if they are still running after one second; their error output
/// is drained in the background. Attached commands are awaited.
pub fn execute_command(
    command: &str,
    detached: bool,
    placeholders: &Placeholders,
) -> Result<(), Failure> {
    let command = placeholders.apply(command.trim());
    if command.is_empty() {
        return Err(Failure::NotFound(String::new()));
    }
    let home = std::env::var_os("USERPROFILE").unwrap_or_default();
    let flags = if detached {
        DETACHED_PROCESS.0 | CREATE_NEW_PROCESS_GROUP.0
    } else {
        CREATE_NO_WINDOW.0
    };

    let mut child = Command::new("cmd.exe")
        .raw_arg(format!("/d /s /c \"{command}\""))
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(flags)
        .spawn()
        .map_err(|e| Failure::Other(e.to_string()))?;

    let mut stderr = child.stderr.take();

    if !detached {
        let mut output = Vec::new();
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_end(&mut output);
        }
        let status = child.wait().map_err(|e| Failure::Other(e.to_string()))?;
        return if status.success() {
            Ok(())
        } else {
            Err(command_failure(status.code(), &output))
        };
    }

    // Collect up to 4 KiB of stderr while we wait up to 1 s for an early startup failure.
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(mut pipe) = stderr {
        let stop_flag = stop.clone();
        std::thread::spawn(move || read_stderr(&mut pipe, &stop_flag, tx));
    }

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    stop.store(true, Ordering::Relaxed);
                    return Ok(());
                }
                let output = rx
                    .recv_timeout(Duration::from_millis(200))
                    .unwrap_or_default();
                stop.store(true, Ordering::Relaxed);
                return Err(command_failure(status.code(), &output));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                return Err(Failure::Other(e.to_string()));
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    Ok(())
}

/// Converts a hotkey like `ControlLeft+KeyV` into scan codes.
pub fn parse_hotkey(hotkey: &str) -> Result<Vec<u16>, String> {
    let codes: Result<Vec<u16>, String> = hotkey
        .split('+')
        .filter(|k| !k.is_empty())
        .map(|k| keycodes::scan_code(k).ok_or_else(|| format!("Unknown key: {k}")))
        .collect();
    match codes {
        Ok(codes) if codes.is_empty() => Err("Empty hotkey".into()),
        other => other,
    }
}

fn send_scan_code(code: u16, up: bool) {
    let mut flags = KEYEVENTF_SCANCODE;
    if code > 0xFF {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code & 0xFF,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn modifiers_held() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000 != 0)
}

/// Waits until the target window is in the foreground, its thread has a focus window and
/// no modifier keys are physically held (each step with a timeout).
pub fn wait_for_target(target: HWND) {
    let start = Instant::now();
    let limit = Duration::from_millis(300);
    while unsafe { GetForegroundWindow() } != target && start.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(5));
    }

    // UWP ApplicationFrameWindow and desktop windows host focus on a separate thread or
    // have no child hwndFocus; avoid burning the full 300ms timeout waiting on them.
    let mut class = [0u16; 64];
    let class_len = unsafe { GetClassNameW(target, &mut class) };
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class[..class_len as usize])
    } else {
        String::new()
    };
    let external_focus_host = matches!(
        class_name.as_str(),
        "ApplicationFrameWindow" | "Progman" | "WorkerW"
    );

    if external_focus_host {
        std::thread::sleep(Duration::from_millis(15));
    } else {
        let thread = unsafe { GetWindowThreadProcessId(target, None) };
        let start = Instant::now();
        loop {
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            let ready = unsafe { GetGUIThreadInfo(thread, &mut info) }.is_ok()
                && info.hwndActive == target
                && !info.hwndFocus.is_invalid();
            if ready || start.elapsed() > limit {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    let start = Instant::now();
    while modifiers_held() && start.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Presses all keys in order and then releases them in reverse order (main key before
/// modifiers), waiting 10 ms between events.
pub fn send_hotkey(codes: &[u16]) {
    for &code in codes {
        send_scan_code(code, false);
        std::thread::sleep(Duration::from_millis(10));
    }
    for &code in codes.iter().rev() {
        send_scan_code(code, true);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Focus has already been returned to `target`; waits for it and sends the hotkey.
pub fn simulate_hotkey(target: HWND, codes: &[u16]) -> Result<(), Failure> {
    if !target.is_invalid() && shell::window_elevated(target) && !shell::current_process_elevated()
    {
        return Err(Failure::TargetElevated);
    }
    if !target.is_invalid() {
        wait_for_target(target);
    }
    send_hotkey(codes);
    Ok(())
}

/// One parsed key event of an `execute-macro` action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacroKey {
    pub code: u16,
    pub down: bool,
    pub delay_ms: u64,
}

/// Converts a list of Kando macro events into scan codes and delays.
pub fn parse_macro(events: &[MacroEvent]) -> Result<Vec<MacroKey>, String> {
    if events.is_empty() {
        return Err("Empty macro".into());
    }
    events
        .iter()
        .map(|event| {
            let code = keycodes::scan_code(&event.key)
                .ok_or_else(|| format!("Unknown key: {}", event.key))?;
            let down = event.event_type != "keyUp";
            let delay_ms = event.delay.unwrap_or(10.0).clamp(0.0, 10_000.0) as u64;
            Ok(MacroKey {
                code,
                down,
                delay_ms,
            })
        })
        .collect()
}

/// Focus has already been returned to `target`; waits for it and replays the macro.
pub fn simulate_macro(target: HWND, keys: &[MacroKey]) -> Result<(), Failure> {
    if !target.is_invalid() && shell::window_elevated(target) && !shell::current_process_elevated()
    {
        return Err(Failure::TargetElevated);
    }
    if !target.is_invalid() {
        wait_for_target(target);
    }
    for key in keys {
        if key.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(key.delay_ms));
        }
        send_scan_code(key.code, !key.down);
    }
    Ok(())
}

/// Copies text (with placeholders expanded) to the Windows clipboard.
pub fn set_clipboard(text: &str, placeholders: &Placeholders) -> Result<(), Failure> {
    let text = placeholders.apply(text);
    shell::set_clipboard_text(&text).map_err(Failure::Other)
}

/// Brings the first open window matching `app_name` and/or `window_name` to the foreground.
pub fn focus_window(app_name: Option<&str>, window_name: Option<&str>) -> Result<(), Failure> {
    if shell::focus_matching_window(app_name, window_name) {
        Ok(())
    } else {
        Err(Failure::Other("No matching window found.".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_replaced() {
        let p = Placeholders {
            app_name: "code.exe".into(),
            window_name: "main.rs".into(),
            pointer_x: 10,
            pointer_y: -5,
        };
        assert_eq!(
            p.apply("x {{app_name}} {{window_name}} {{pointer_x}},{{pointer_y}}"),
            "x code.exe main.rs 10,-5"
        );
    }

    #[test]
    fn hotkeys_are_parsed() {
        assert_eq!(parse_hotkey("ControlLeft+KeyV"), Ok(vec![0x1D, 0x2F]));
        assert_eq!(parse_hotkey("MetaLeft+ArrowUp"), Ok(vec![0xE05B, 0xE048]));
        assert!(parse_hotkey("ControlLeft+Nope").is_err());
        assert!(parse_hotkey("").is_err());
    }

    #[test]
    fn missing_paths_are_reported() {
        assert_eq!(
            open_file(r"C:\definitely\not\existing\file.txt"),
            Err(Failure::NotFound(
                r"C:\definitely\not\existing\file.txt".into()
            ))
        );
        assert_eq!(
            launch_app(r"C:\definitely\not\existing\app.exe"),
            Err(Failure::NotFound(
                r"C:\definitely\not\existing\app.exe".into()
            ))
        );
    }

    #[test]
    fn error_output_is_drained_to_the_end() {
        // Long output: the first 4 KiB are reported once, the rest is still consumed.
        let mut pipe = std::io::Cursor::new(vec![b'x'; 10_000]);
        let (tx, rx) = std::sync::mpsc::channel();
        read_stderr(&mut pipe, &AtomicBool::new(false), tx);
        assert_eq!(pipe.position(), 10_000);
        assert_eq!(rx.recv().unwrap().len(), MAX_STDERR);
        assert!(rx.recv().is_err());

        // Short output is reported when it ends.
        let mut pipe = std::io::Cursor::new(b"boom".to_vec());
        let (tx, rx) = std::sync::mpsc::channel();
        read_stderr(&mut pipe, &AtomicBool::new(false), tx);
        assert_eq!(rx.recv().unwrap(), b"boom");

        // After the caller stopped waiting, the output is discarded but still drained.
        let mut pipe = std::io::Cursor::new(vec![b'x'; 10_000]);
        let (tx, rx) = std::sync::mpsc::channel();
        read_stderr(&mut pipe, &AtomicBool::new(true), tx);
        assert_eq!(pipe.position(), 10_000);
        assert!(rx.recv().is_err());
    }

    #[test]
    fn failing_commands_report_errors() {
        let p = Placeholders::default();
        assert!(execute_command("exit /b 0", false, &p).is_ok());
        assert!(matches!(
            execute_command("exit /b 3", false, &p),
            Err(Failure::Other(_))
        ));
        assert!(matches!(
            execute_command("exit /b 3", true, &p),
            Err(Failure::Other(_))
        ));
    }
}
