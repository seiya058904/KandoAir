// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Native dialogs: file open/save dialogs and task dialogs (message boxes with custom
//! buttons). They are modal to the given owner window and must run on the UI thread,
//! outside of WebView2 event handlers (see `App::defer`).

use std::path::Path;

use serde_json::Value;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::*;
use windows::core::{HSTRING, PCWSTR};

/// Electron-style file filters: `[{ name, extensions: ["json"] }]`.
fn filters_of(options: &Value) -> Vec<(HSTRING, HSTRING)> {
    options
        .get("filters")
        .and_then(Value::as_array)
        .map(|filters| {
            filters
                .iter()
                .filter_map(|f| {
                    let name = f.get("name")?.as_str()?;
                    let extensions: Vec<String> = f
                        .get("extensions")?
                        .as_array()?
                        .iter()
                        .filter_map(|e| e.as_str())
                        .map(|e| {
                            if e == "*" {
                                "*.*".to_string()
                            } else {
                                format!("*.{e}")
                            }
                        })
                        .collect();
                    Some((HSTRING::from(name), HSTRING::from(extensions.join(";"))))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn apply_common(dialog: &IFileDialog, options: &Value) -> windows::core::Result<()> {
    unsafe {
        let filters = filters_of(options);
        if !filters.is_empty() {
            let specs: Vec<COMDLG_FILTERSPEC> = filters
                .iter()
                .map(|(name, spec)| COMDLG_FILTERSPEC {
                    pszName: PCWSTR(name.as_ptr()),
                    pszSpec: PCWSTR(spec.as_ptr()),
                })
                .collect();
            dialog.SetFileTypes(&specs)?;
        }
        if let Some(title) = options.get("title").and_then(Value::as_str) {
            dialog.SetTitle(&HSTRING::from(title))?;
        }
        if let Some(default_path) = options.get("defaultPath").and_then(Value::as_str) {
            let path = Path::new(default_path);
            let (folder, file) = if path.is_dir() {
                (Some(path), None)
            } else {
                (path.parent(), path.file_name())
            };
            if let Some(folder) = folder.filter(|f| f.is_dir())
                && let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                    &HSTRING::from(folder.as_os_str()),
                    None,
                )
            {
                let _ = dialog.SetFolder(&item);
            }
            if let Some(file) = file {
                let _ = dialog.SetFileName(&HSTRING::from(file));
            }
        }
    }
    Ok(())
}

fn result_path(dialog: &IFileDialog) -> Option<String> {
    unsafe {
        let item = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        path
    }
}

/// Shows an open dialog. `options` follows Electron's `OpenDialogOptions`.
pub fn open_file(owner: HWND, options: &Value) -> Option<String> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let properties: Vec<&str> = options
            .get("properties")
            .and_then(Value::as_array)
            .map(|p| p.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut flags = dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM;
        if properties.contains(&"openDirectory") && !properties.contains(&"openFile") {
            flags |= FOS_PICKFOLDERS;
        }
        let _ = dialog.SetOptions(flags);
        let base: IFileDialog = windows::core::Interface::cast(&dialog).ok()?;
        let _ = apply_common(&base, options);
        dialog.Show(Some(owner)).ok()?;
        result_path(&base)
    }
}

/// Shows a save dialog. `options` follows Electron's `SaveDialogOptions`.
pub fn save_file(owner: HWND, options: &Value) -> Option<String> {
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let flags = dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT;
        let _ = dialog.SetOptions(flags);
        let base: IFileDialog = windows::core::Interface::cast(&dialog).ok()?;
        let _ = apply_common(&base, options);
        if let Some(ext) = filters_of(options).first() {
            let first = ext.1.to_string_lossy();
            if let Some(e) = first.strip_prefix("*.").and_then(|e| e.split(';').next()) {
                let _ = dialog.SetDefaultExtension(&HSTRING::from(e));
            }
        }
        dialog.Show(Some(owner)).ok()?;
        result_path(&base)
    }
}

#[derive(Clone, Copy)]
pub enum Kind {
    Info,
    Warning,
    Error,
}

/// Shows a task dialog. With `buttons` (cancel label, confirm label) the dialog asks for
/// confirmation and returns true if the confirm button was pressed.
pub fn message(
    owner: HWND,
    kind: Kind,
    title: &str,
    message: &str,
    buttons: Option<(&str, &str)>,
) -> bool {
    let window_title = HSTRING::from("KandoAir");
    let instruction = HSTRING::from(title);
    let content = HSTRING::from(message);
    let labels = buttons.map(|(cancel, confirm)| (HSTRING::from(cancel), HSTRING::from(confirm)));
    const CONFIRM: i32 = 1000;
    let custom: Vec<TASKDIALOG_BUTTON> = match &labels {
        Some((cancel, confirm)) => vec![
            TASKDIALOG_BUTTON {
                nButtonID: CONFIRM,
                pszButtonText: PCWSTR(confirm.as_ptr()),
            },
            TASKDIALOG_BUTTON {
                nButtonID: 2, // IDCANCEL
                pszButtonText: PCWSTR(cancel.as_ptr()),
            },
        ],
        None => Vec::new(),
    };
    let icon = match kind {
        Kind::Info => TD_INFORMATION_ICON,
        Kind::Warning => TD_WARNING_ICON,
        Kind::Error => TD_ERROR_ICON,
    };
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: owner,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW,
        dwCommonButtons: if labels.is_some() {
            TASKDIALOG_COMMON_BUTTON_FLAGS(0)
        } else {
            TDCBF_OK_BUTTON
        },
        pszWindowTitle: PCWSTR(window_title.as_ptr()),
        Anonymous1: TASKDIALOGCONFIG_0 { pszMainIcon: icon },
        pszMainInstruction: PCWSTR(instruction.as_ptr()),
        pszContent: PCWSTR(content.as_ptr()),
        cButtons: custom.len() as u32,
        pButtons: if custom.is_empty() {
            std::ptr::null()
        } else {
            custom.as_ptr()
        },
        nDefaultButton: if labels.is_some() { 2 } else { 0 },
        ..Default::default()
    };
    let mut pressed = 0i32;
    unsafe {
        if TaskDialogIndirect(&config, Some(&mut pressed), None, None).is_err() {
            return false;
        }
    }
    pressed == CONFIRM
}
