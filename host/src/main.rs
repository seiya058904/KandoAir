// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! KandoAir: a lightweight, Windows-only pie menu. The native host (this crate) replaces
//! Kando's Electron main process; the menu and the settings are Kando's original web
//! frontends, hosted in WebView2.

#![windows_subsystem = "windows"]

mod actions;
mod app;
mod apps;
mod config;
mod dialogs;
mod hotkey;
mod i18n;
mod icons;
mod ipc;
mod keycodes;
mod log;
mod menu_window;
mod paths;
mod settings_window;
mod shell;
mod strings;
mod themes;
mod tray;
mod webview;

use std::path::PathBuf;
use std::time::Duration;

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, w};

use crate::app::{App, COPYDATA_ARGS, HOST_CLASS, Options};
use crate::paths::Paths;

/// Hands the command line to the running instance.
fn forward_to_running_instance(args: &[String]) {
    for _ in 0..40 {
        if let Ok(hwnd) = unsafe { FindWindowW(HOST_CLASS, None) } {
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                // The running instance may bring its windows to the foreground.
                let _ = AllowSetForegroundWindow(pid);
            }
            let payload = serde_json::to_vec(args).unwrap_or_default();
            let data = COPYDATASTRUCT {
                dwData: COPYDATA_ARGS,
                cbData: payload.len() as u32,
                lpData: payload.as_ptr() as *mut _,
            };
            unsafe {
                SendMessageTimeoutW(
                    hwnd,
                    WM_COPYDATA,
                    WPARAM(0),
                    LPARAM(&data as *const _ as isize),
                    SMTO_ABORTIFHUNG,
                    3000,
                    None,
                );
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = Options::parse(&args);

    // Only one instance runs; further instances forward their command line.
    let _mutex = unsafe { CreateMutexW(None, true, w!(r"Local\KandoAir.SingleInstance")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        forward_to_running_instance(&args);
        return;
    }

    let paths = Paths::resolve(options.config_dir.as_ref().map(PathBuf::from));
    log::init(&paths.log_file());
    log!(
        "KandoAir {} starting (config: {})",
        config::VERSION,
        paths.config_dir.display()
    );

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    }

    if shell::webview2_version().is_none() {
        let locale = config::load_general(&config::Files::new(&paths.config_dir)).locale;
        let i18n = i18n::I18n::load(&paths.locales_dir(), &locale);
        let title = HSTRING::from(i18n.t(strings::WEBVIEW2_MISSING_TITLE));
        let message = HSTRING::from(i18n.t(strings::WEBVIEW2_MISSING_MESSAGE));
        unsafe {
            MessageBoxW(None, &message, &title, MB_OK | MB_ICONERROR);
        }
        return;
    }

    let _app = App::start(paths, options);

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    log!("KandoAir stopped");
}

#[cfg(test)]
mod tests {
    /// A `RefMut` in the scrutinee of `if let`, `while let` or `match` lives until the end
    /// of the block. Win32 calls in the block (DestroyWindow, ShowWindow, modal loops)
    /// re-enter window procedures which borrow the same cell again, and the panic aborts
    /// the process. Take the value into a local first. (Clippy's
    /// `significant_drop_in_scrutinee` does not catch this for `RefMut`.)
    #[test]
    fn no_mutable_borrows_in_scrutinees() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (index, line) in text.lines().enumerate() {
                let code = line.trim_start();
                let scrutinee = code.contains("if let ")
                    || code.contains("while let ")
                    || code.starts_with("match ");
                if scrutinee && code.contains(".borrow_mut()") {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "take the value out of the RefCell before the block: {offenders:?}"
        );
    }
}
