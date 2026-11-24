// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The notification-area icon, its context menu and balloon notifications. On Windows
//! 10 and 11, balloons are shown as toast notifications.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::log;

/// The callback message of the tray icon.
pub const WM_TRAY: u32 = WM_APP + 1;
const ICON_ID: u32 = 1;

pub enum MenuEntry {
    Item {
        id: u32,
        label: String,
        checked: bool,
    },
    Separator,
}

pub struct Tray {
    hwnd: HWND,
    icon: Option<HICON>,
    /// Whether the icon is currently added to the notification area.
    added: bool,
    /// True if the icon was only added to show a notification (tray icon disabled).
    temporary: bool,
    flavor: String,
    tooltip: String,
}

fn png_for_flavor(flavor: &str) -> &'static [u8] {
    match flavor {
        "light" => include_bytes!("../../assets/icons/trayLight.png"),
        "dark" => include_bytes!("../../assets/icons/trayDark.png"),
        "black" => include_bytes!("../../assets/icons/trayBlack.png"),
        "white" => include_bytes!("../../assets/icons/trayWhite.png"),
        _ => include_bytes!("../../assets/icons/trayColor.png"),
    }
}

/// Creates an icon from PNG data. The shell scales it to the notification area size.
fn icon_from_png(png: &[u8]) -> Option<HICON> {
    unsafe { CreateIconFromResourceEx(png, true, 0x0003_0000, 0, 0, LR_DEFAULTCOLOR).ok() }
}

fn copy_wide(target: &mut [u16], text: &str) {
    let wide: Vec<u16> = text.encode_utf16().take(target.len() - 1).collect();
    target[..wide.len()].copy_from_slice(&wide);
    target[wide.len()] = 0;
}

impl Tray {
    pub fn new(hwnd: HWND, tooltip: &str) -> Tray {
        Tray {
            hwnd,
            icon: None,
            added: false,
            temporary: false,
            flavor: String::new(),
            tooltip: tooltip.to_string(),
        }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAY,
            hIcon: self.icon.unwrap_or_default(),
            ..Default::default()
        };
        copy_wide(&mut data.szTip, &self.tooltip);
        data
    }

    fn add(&mut self) {
        if self.added {
            return;
        }
        let mut data = self.data();
        unsafe {
            if Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
                self.added = true;
            } else {
                log!("Shell_NotifyIconW(NIM_ADD) failed");
            }
        }
    }

    /// Shows the icon in the given flavor, or removes it for the flavor "none".
    pub fn set_flavor(&mut self, flavor: &str) {
        if flavor == self.flavor && (self.added || flavor == "none") {
            return;
        }
        self.flavor = flavor.to_string();
        if flavor == "none" {
            self.remove();
            return;
        }
        if let Some(old) = self.icon.take() {
            unsafe {
                let _ = DestroyIcon(old);
            }
        }
        self.icon = icon_from_png(png_for_flavor(flavor));
        self.temporary = false;
        if self.added {
            let data = self.data();
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
            }
        } else {
            self.add();
        }
    }

    /// Re-adds the icon after Explorer restarted.
    pub fn readd(&mut self) {
        let was_added = self.added;
        self.added = false;
        if was_added {
            self.add();
        }
    }

    /// Shows a notification. If the tray icon is disabled, a temporary icon is added; the
    /// caller should call [`Tray::remove_temporary`] after a while.
    pub fn notify(&mut self, title: &str, message: &str, error: bool) -> bool {
        if !self.added {
            if self.icon.is_none() {
                self.icon = icon_from_png(png_for_flavor("color"));
            }
            self.add();
            self.temporary = true;
        }
        let mut data = self.data();
        data.uFlags = NIF_INFO;
        copy_wide(&mut data.szInfoTitle, title);
        copy_wide(&mut data.szInfo, message);
        data.dwInfoFlags = if error { NIIF_ERROR } else { NIIF_INFO };
        unsafe {
            if !Shell_NotifyIconW(NIM_MODIFY, &data).as_bool() {
                log!("notification failed: {title}: {message}");
            }
        }
        log!("notification: {title}: {message}");
        self.temporary
    }

    pub fn remove_temporary(&mut self) {
        if self.temporary {
            self.remove();
            self.temporary = false;
        }
    }

    pub fn remove(&mut self) {
        if self.added {
            let data = self.data();
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            }
            self.added = false;
        }
    }

    /// Shows the context menu at the pointer and returns the chosen command (0 = none).
    pub fn show_menu(&self, entries: &[MenuEntry]) -> u32 {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else {
                return 0;
            };
            for entry in entries {
                match entry {
                    MenuEntry::Item { id, label, checked } => {
                        let label = windows::core::HSTRING::from(label.as_str());
                        let mut flags = MF_STRING;
                        if *checked {
                            flags |= MF_CHECKED;
                        }
                        let _ = AppendMenuW(menu, flags, *id as usize, &label);
                    }
                    MenuEntry::Separator => {
                        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                    }
                }
            }
            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);
            // Required so that the menu closes when the user clicks elsewhere.
            let _ = SetForegroundWindow(self.hwnd);
            let command = TrackPopupMenuEx(
                menu,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0,
                point.x,
                point.y,
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            command.0 as u32
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.remove();
        if let Some(icon) = self.icon.take() {
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

/// Decodes the event of a tray callback message (NOTIFYICON_VERSION_4).
pub fn event_of(lparam: LPARAM) -> u32 {
    (lparam.0 as u32) & 0xFFFF
}
