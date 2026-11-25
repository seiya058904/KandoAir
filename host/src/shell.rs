// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Small wrappers around Windows shell and system APIs.

use std::path::Path;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, GlobalFree, HANDLE, HWND,
    LPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, SetProcessWorkingSetSize,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetWindow,
    GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR, w};

/// Resolves the hosted UWP process PID inside an `ApplicationFrameHost.exe` window.
fn uwp_child_pid(frame_hwnd: HWND, host_pid: u32) -> Option<u32> {
    struct ChildSearch {
        host_pid: u32,
        found_pid: Option<u32>,
    }

    extern "system" fn enum_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut ChildSearch) };
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != 0 && pid != search.host_pid {
            search.found_pid = Some(pid);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = ChildSearch {
        host_pid,
        found_pid: None,
    };
    unsafe {
        let _ = EnumChildWindows(
            Some(frame_hwnd),
            Some(enum_child),
            LPARAM(&mut search as *mut _ as isize),
        );
    }
    search.found_pid
}

/// The executable file name (e.g. `notepad.exe`) and the title of a window.
pub fn window_app_and_title(hwnd: HWND) -> (String, String) {
    let mut title = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut title) };
    let title = String::from_utf16_lossy(&title[..len.max(0) as usize]);

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    let mut app = process_image_path(pid)
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    if app.eq_ignore_ascii_case("ApplicationFrameHost.exe")
        && let Some(child_pid) = uwp_child_pid(hwnd, pid)
        && let Some(child_app) = process_image_path(child_pid).and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
    {
        app = child_app;
    }
    (app, title)
}

fn process_image_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

fn token_is_elevated(process: HANDLE) -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// Whether the current process runs elevated.
pub fn current_process_elevated() -> bool {
    token_is_elevated(unsafe { GetCurrentProcess() })
}

/// Whether the process owning the window runs elevated. Simulated input to such windows
/// is silently dropped by UIPI unless KandoAir runs elevated as well.
pub fn window_elevated(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(process) => {
                let elevated = token_is_elevated(process);
                let _ = CloseHandle(process);
                elevated
            }
            Err(_) => false,
        }
    }
}

fn registry_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            &HSTRING::from(value),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut _),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

fn registry_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let key = HSTRING::from(key);
    let value = HSTRING::from(value);
    // Query the size first. The value may grow in between, so retry once.
    for _ in 0..2 {
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(root, &key, &value, RRF_RT_REG_SZ, None, None, Some(&mut size))
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        let status = unsafe {
            RegGetValueW(
                root,
                &key,
                &value,
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr() as *mut _),
                Some(&mut size),
            )
        };
        if status == ERROR_MORE_DATA {
            continue;
        }
        if status != ERROR_SUCCESS {
            return None;
        }
        let chars = (size as usize / 2).saturating_sub(1).min(buffer.len());
        return Some(String::from_utf16_lossy(&buffer[..chars]));
    }
    None
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const AUTO_START_VALUE: &str = "KandoAir";

/// Whether a `Run` value (written by [`set_auto_start`] as the quoted path) starts `exe`.
fn is_current_command(value: &str, exe: &Path) -> bool {
    let registered = value.trim().trim_matches('"');
    !registered.is_empty() && registered.to_lowercase() == exe.to_string_lossy().to_lowercase()
}

/// Whether this executable is registered to start automatically at Windows login. A value
/// which points to another copy (e.g. after the portable folder was moved) counts as
/// disabled; enabling the setting again registers this copy.
pub fn auto_start_enabled() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    registry_string(HKEY_CURRENT_USER, RUN_KEY, AUTO_START_VALUE)
        .is_some_and(|v| is_current_command(&v, &exe))
}

/// Enables or disables starting KandoAir at Windows login via `HKCU\...\Run`.
pub fn set_auto_start(enable: bool) -> Result<(), String> {
    unsafe {
        if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = format!("\"{}\"", exe.display());
            let mut key = HKEY::default();
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            );
            if status != ERROR_SUCCESS {
                return Err(format!("RegCreateKeyExW failed ({})", status.0));
            }
            let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes =
                std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            let write = RegSetValueExW(key, w!("KandoAir"), None, REG_SZ, Some(bytes));
            let _ = RegCloseKey(key);
            if write != ERROR_SUCCESS {
                return Err(format!("RegSetValueExW failed ({})", write.0));
            }
            Ok(())
        } else {
            let mut key = HKEY::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                None,
                KEY_SET_VALUE,
                &mut key,
            );
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            if status != ERROR_SUCCESS {
                return Err(format!("RegOpenKeyExW failed ({})", status.0));
            }
            let del = RegDeleteValueW(key, w!("KandoAir"));
            let _ = RegCloseKey(key);
            if del != ERROR_SUCCESS && del != ERROR_FILE_NOT_FOUND {
                return Err(format!("RegDeleteValueW failed ({})", del.0));
            }
            Ok(())
        }
    }
}

/// Releases unused working set pages of the native host process while idle.
pub fn trim_working_set() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

/// Matches a Kando condition string against an app name or window title.
///
/// Plain strings match as case-insensitive substrings. Strings wrapped in `/pattern/flags`
/// (e.g. `/code|notepad/i`, `/^Code\.exe$/`) are evaluated as regular expressions.
pub fn matches_condition(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return true;
    }
    if let Some(rest) = pattern.strip_prefix('/')
        && let Some(end) = rest.rfind('/')
    {
        let body = &rest[..end];
        let flags = &rest[end + 1..];
        let valid_flags = flags
            .chars()
            .all(|c| matches!(c, 'i' | 'm' | 's' | 'g' | 'u'));
        if valid_flags
            && let Ok(re) = regex_lite::RegexBuilder::new(body)
                .case_insensitive(flags.contains('i'))
                .multi_line(flags.contains('m'))
                .dot_matches_new_line(flags.contains('s'))
                .build()
        {
            return re.is_match(value);
        }
    }
    value.to_lowercase().contains(&pattern.to_lowercase())
}

fn is_window_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    let ok = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
    };
    ok && cloaked != 0
}

/// Finds the first visible top-level window matching `app_name` and/or `window_name` and
/// brings it to the foreground. Returns true if a matching window was found.
pub fn focus_matching_window(app_name: Option<&str>, window_name: Option<&str>) -> bool {
    let app_filter = app_name.map(str::trim).filter(|s| !s.is_empty());
    let win_filter = window_name.map(str::trim).filter(|s| !s.is_empty());
    if app_filter.is_none() && win_filter.is_none() {
        return false;
    }

    struct Search<'a> {
        app_filter: Option<&'a str>,
        win_filter: Option<&'a str>,
        found: Option<HWND>,
    }

    extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search<'_>) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || is_window_cloaked(hwnd) {
                return BOOL(1);
            }
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
                return BOOL(1);
            }
            if GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
                return BOOL(1);
            }
            let mut class = [0u16; 64];
            let class_len = GetClassNameW(hwnd, &mut class);
            if class_len > 0 {
                let class_name = String::from_utf16_lossy(&class[..class_len as usize]);
                if class_name.starts_with("KandoAir") {
                    return BOOL(1);
                }
            }
        }
        let (app, title) = window_app_and_title(hwnd);
        if app.is_empty() && title.is_empty() {
            return BOOL(1);
        }
        // Skip hidden/zero-title Chromium/Electron helper windows when matching by app only.
        if search.win_filter.is_none() && title.is_empty() {
            return BOOL(1);
        }
        let app_ok = search
            .app_filter
            .is_none_or(|p| matches_condition(p, &app));
        let win_ok = search
            .win_filter
            .is_none_or(|p| matches_condition(p, &title));
        if app_ok && win_ok {
            search.found = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = Search {
        app_filter,
        win_filter,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut search as *mut _ as isize));
    }
    if let Some(hwnd) = search.found {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        true
    } else {
        false
    }
}

/// Writes UTF-16 text to the Windows clipboard (CF_UNICODETEXT).
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * 2;

    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            return Err("Could not open the Windows clipboard.".into());
        }

        let _ = EmptyClipboard();
        let mem = match GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            Ok(h) => h,
            Err(e) => {
                let _ = CloseClipboard();
                return Err(format!("GlobalAlloc failed: {e}"));
            }
        };
        let ptr = GlobalLock(mem) as *mut u16;
        if ptr.is_null() {
            let _ = GlobalFree(Some(mem));
            let _ = CloseClipboard();
            return Err("GlobalLock failed.".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(mem);

        if let Err(e) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0))) {
            let _ = GlobalFree(Some(mem));
            let _ = CloseClipboard();
            return Err(format!("SetClipboardData failed: {e}"));
        }
        let _ = CloseClipboard();
        Ok(())
    }
}

/// Whether apps should use dark colors.
pub fn dark_mode() -> bool {
    registry_dword(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    )
    .map(|v| v == 0)
    .unwrap_or(false)
}

/// The Windows build number, e.g. 22631.
pub fn windows_build() -> u32 {
    registry_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )
    .and_then(|s| s.parse().ok())
    .unwrap_or(0)
}

/// Expands environment variables like `%USERPROFILE%`.
pub fn expand_env(text: &str) -> String {
    if !text.contains('%') {
        return text.to_string();
    }
    let source = HSTRING::from(text);
    unsafe {
        let needed = ExpandEnvironmentStringsW(&source, None);
        if needed == 0 {
            return text.to_string();
        }
        let mut buffer = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(&source, Some(&mut buffer));
        if written == 0 {
            return text.to_string();
        }
        String::from_utf16_lossy(&buffer[..(written as usize).saturating_sub(1)])
    }
}

/// Opens a file, folder, URI or program with the shell. Returns the Win32 error code on
/// failure. Must be called on a thread with COM initialized.
pub fn shell_execute(
    file: &str,
    parameters: Option<&str>,
    directory: Option<&str>,
) -> Result<(), u32> {
    let file = HSTRING::from(file);
    let parameters = parameters.map(HSTRING::from);
    let directory = directory.map(HSTRING::from);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: parameters
            .as_ref()
            .map(|p| PCWSTR(p.as_ptr()))
            .unwrap_or(PCWSTR::null()),
        lpDirectory: directory
            .as_ref()
            .map(|d| PCWSTR(d.as_ptr()))
            .unwrap_or(PCWSTR::null()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        match ShellExecuteExW(&mut info) {
            Ok(()) => Ok(()),
            Err(e) => Err((e.code().0 as u32) & 0xFFFF),
        }
    }
}

/// The version of the installed WebView2 runtime, or `None` if it is missing.
pub fn webview2_version() -> Option<String> {
    unsafe {
        let mut version = PWSTR::null();
        webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString(
            PCWSTR::null(),
            &mut version,
        )
        .ok()?;
        let text = webview2_com::take_pwstr(version);
        (!text.is_empty()).then_some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_matching_supports_substrings_and_slash_patterns() {
        assert!(matches_condition("", "anything"));
        assert!(matches_condition("code", "Code.exe"));
        assert!(!matches_condition("firefox", "Code.exe"));
        assert!(matches_condition("/code|notepad/i", "Notepad.exe"));
        assert!(matches_condition("/^Code\\.exe$/", "Code.exe"));
        assert!(!matches_condition("/^Code\\.exe$/", "CodeXexe"));
        assert!(matches_condition("/^Code/", "Code.exe"));
        assert!(!matches_condition("/^code/", "Code.exe"));
        assert!(matches_condition("/exe$/i", "Code.EXE"));
        assert!(matches_condition("/tab-\\d+/i", "My TAB-42 Window"));
        assert!(matches_condition("/home/user/file", "vim /home/user/file"));
    }

    #[test]
    fn auto_start_commands_must_point_to_this_executable() {
        let exe = Path::new(r"C:\Tools\KandoAir\KandoAir.exe");
        assert!(is_current_command(r#""C:\Tools\KandoAir\KandoAir.exe""#, exe));
        assert!(is_current_command(r#""c:\tools\kandoair\KANDOAIR.EXE""#, exe));
        assert!(!is_current_command(r#""D:\Old\KandoAir\KandoAir.exe""#, exe));
        assert!(!is_current_command("", exe));
        assert!(!is_current_command(r#""""#, exe));
        let unicode = Path::new(r"D:\工具\Ärger\KandoAir.exe");
        assert!(is_current_command(r#""D:\工具\äRGER\KandoAir.exe""#, unicode));
    }

    #[test]
    fn registry_strings_are_read() {
        // A value that exists on every Windows installation.
        let build = registry_string(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        );
        assert!(build.is_some_and(|b| b.parse::<u32>().is_ok()));
        assert!(registry_string(HKEY_CURRENT_USER, RUN_KEY, "KandoAir-does-not-exist").is_none());
    }
}
