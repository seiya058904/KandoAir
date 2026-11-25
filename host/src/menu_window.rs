// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The menu window: a transparent, top-most popup which covers the work area of the
//! monitor under the pointer. It hosts the menu web view, which stays loaded while the
//! window is hidden so that menus can be shown instantly. While hidden for a while, the
//! web view is trimmed and suspended.

use std::cell::{Cell, RefCell};

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, w};

use crate::log;
use crate::webview;

/// Geometry of the monitor on which a menu is opened.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// The work area in physical pixels.
    pub work_area: RECT,
    /// The pointer position in physical pixels.
    pub pointer: POINT,
    /// The DPI scale factor of the monitor (1.0 = 96 DPI).
    pub scale: f64,
}

impl Placement {
    /// Computes the placement for the monitor under the mouse pointer.
    pub fn at_pointer() -> Placement {
        unsafe {
            let mut pointer = POINT::default();
            let _ = GetCursorPos(&mut pointer);
            let monitor = MonitorFromPoint(pointer, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut info);
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            Placement {
                work_area: info.rcWork,
                pointer,
                scale: dpi_x as f64 / 96.0,
            }
        }
    }

    /// The size of the work area in CSS pixels for the given zoom factor.
    pub fn css_size(&self, zoom: f64) -> (f64, f64) {
        let factor = self.scale * zoom;
        (
            (self.work_area.right - self.work_area.left) as f64 / factor,
            (self.work_area.bottom - self.work_area.top) as f64 / factor,
        )
    }

    /// The pointer position relative to the work area in CSS pixels.
    pub fn css_pointer(&self, zoom: f64) -> (f64, f64) {
        let factor = self.scale * zoom;
        (
            (self.pointer.x - self.work_area.left) as f64 / factor,
            (self.pointer.y - self.work_area.top) as f64 / factor,
        )
    }
}

pub struct MenuWindow {
    pub hwnd: HWND,
    pub controller: RefCell<Option<ICoreWebView2Controller>>,
    pub webview: RefCell<Option<ICoreWebView2>>,
    /// True while a menu is shown (also during the fade-out animation).
    pub visible: Cell<bool>,
    /// The window which was in the foreground when the menu was opened.
    pub previous_foreground: Cell<HWND>,
    /// Last applied work area and DPI scale bits so repeated opens on the same monitor avoid
    /// redundant resizes while still adapting to DPI changes.
    last_geometry: Cell<Option<(RECT, u64)>>,
}

const CLASS_NAME: windows::core::PCWSTR = w!("KandoAirMenuWindow");

fn extended_style(keep_input_focus: bool) -> WINDOW_EX_STYLE {
    let mut style = WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP;
    if keep_input_focus {
        style |= WS_EX_NOACTIVATE;
    }
    style
}

impl MenuWindow {
    /// Creates the (hidden) menu window.
    pub fn create(wndproc: WNDPROC, keep_input_focus: bool) -> windows::core::Result<MenuWindow> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: wndproc,
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassExW(&class);

            let hwnd = CreateWindowExW(
                extended_style(keep_input_focus),
                CLASS_NAME,
                w!("KandoAir Menu"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            Ok(MenuWindow {
                hwnd,
                controller: RefCell::new(None),
                webview: RefCell::new(None),
                visible: Cell::new(false),
                previous_foreground: Cell::new(HWND::default()),
                last_geometry: Cell::new(None),
            })
        }
    }

    fn controller(&self) -> Option<ICoreWebView2Controller> {
        self.controller.borrow().clone()
    }

    /// Resizes the web view to the window.
    pub fn fit(&self) {
        if let Some(controller) = self.controller() {
            webview::fit_to_window(&controller, self.hwnd);
        }
    }

    /// Ensures the hidden or visible window covers `placement`'s work area and DPI before
    /// the renderer lays out the menu.
    pub fn prepare_bounds(&self, placement: &Placement) {
        let wa = placement.work_area;
        let scale_bits = placement.scale.to_bits();
        let same = self.last_geometry.get().is_some_and(|(prev, prev_scale)| {
            prev.left == wa.left
                && prev.top == wa.top
                && prev.right == wa.right
                && prev.bottom == wa.bottom
                && prev_scale == scale_bits
        });
        if same {
            return;
        }
        self.last_geometry.set(Some((wa, scale_bits)));
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                wa.left,
                wa.top,
                wa.right - wa.left,
                wa.bottom - wa.top,
                SWP_NOACTIVATE,
            );
        }
        if let Some(controller) = self.controller() {
            webview::fit_to_window(&controller, self.hwnd);
        }
    }

    /// Moves the window over the work area, makes the web view visible and activates the
    /// window. The show-menu message should be posted to the page right before.
    pub fn show(&self, placement: &Placement, keep_input_focus: bool) {
        self.visible.set(true);
        self.prepare_bounds(placement);
        let controller = self.controller();
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
            if let Some(controller) = &controller {
                let _ = controller.SetIsVisible(true);
            }
            if keep_input_focus {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            } else {
                let _ = ShowWindow(self.hwnd, SW_SHOW);
                let _ = SetForegroundWindow(self.hwnd);
                let _ = SetFocus(Some(self.hwnd));
                if let Some(controller) = &controller {
                    let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                }
            }
        }
    }

    /// Hides the window and optionally returns the input focus to the window which was in
    /// the foreground when the menu was opened.
    pub fn hide(&self, restore_focus: bool) {
        self.visible.set(false);
        let controller = self.controller();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            if let Some(controller) = &controller {
                let _ = controller.SetIsVisible(false);
            }
            let previous = self.previous_foreground.get();
            if restore_focus && !previous.is_invalid() && IsWindow(Some(previous)).as_bool() {
                let _ = SetForegroundWindow(previous);
            }
        }
    }

    pub fn set_zoom(&self, zoom: f64) {
        if let Some(controller) = self.controller() {
            unsafe {
                let _ = controller.SetZoomFactor(zoom);
            }
        }
    }

    pub fn set_keep_input_focus(&self, keep: bool) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, extended_style(keep).0 as isize);
        }
    }

    fn set_memory_level(&self, low: bool) {
        if let Some(webview) = self.webview.borrow().clone()
            && let Ok(webview19) = webview.cast::<ICoreWebView2_19>()
        {
            let level = if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            unsafe {
                if let Err(e) = webview19.SetMemoryUsageTargetLevel(level) {
                    log!("SetMemoryUsageTargetLevel failed: {e}");
                }
            }
        }
    }

    /// Trims and suspends the hidden web view. Returns true if a suspension was started.
    pub fn suspend(&self) -> bool {
        self.set_memory_level(true);
        match self.webview.borrow().clone() {
            Some(webview) => crate::app::try_suspend(&webview),
            None => false,
        }
    }

    /// Resumes a suspended web view.
    pub fn resume(&self) {
        if let Some(webview) = self.webview.borrow().clone()
            && let Ok(webview3) = webview.cast::<ICoreWebView2_3>()
        {
            unsafe {
                let _ = webview3.Resume();
            }
        }
        self.set_memory_level(false);
    }

    /// Closes the web view and destroys the window.
    pub fn destroy(&self) {
        self.webview.borrow_mut().take();
        let controller = self.controller.borrow_mut().take();
        if let Some(controller) = controller {
            unsafe {
                let _ = controller.Close();
            }
        }
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
