// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The settings window (the counterpart of Kando's `settings-window.ts` and the
//! settings-related IPC handlers of `app.ts`). It is created on demand and destroyed when
//! closed, which ends its WebView2 renderer process and frees its memory.
//!
//! The window has no caption: like Kando's frameless window, the page draws its own
//! header (`app-region: drag`) and window buttons. With Kando's transparent flavors, the
//! window uses the acrylic system backdrop on Windows 11 22H2 and later.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromRect,
    MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForWindow, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Interface, PCWSTR, w};

use crate::app::{self, App, Page, ShowRequest};
use crate::apps;
use crate::config::{self, Action, GeneralSettings, MenuItem, MenuSettings, Workflow};
use crate::dialogs::{self, Kind};
use crate::ipc::Request;
use crate::log;
use crate::shell;
use crate::strings as s;
use crate::themes;
use crate::webview;

const CLASS_NAME: PCWSTR = w!("KandoAirSettingsWindow");

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Bounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
struct SidebarWidths {
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<i64>,
}

/// Persisted in `settings-window-state.json`. Bounds are physical pixels.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
struct WindowState {
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<Bounds>,
    maximized: bool,
    sidebar_widths: SidebarWidths,
}

pub struct SettingsWindow {
    pub hwnd: HWND,
    /// The window's own WebView2 environment. Its browser processes exit when the window
    /// is destroyed, which releases all memory used by the settings.
    environment: RefCell<Option<ICoreWebView2Environment>>,
    pub controller: RefCell<Option<ICoreWebView2Controller>>,
    pub webview: RefCell<Option<ICoreWebView2>>,
    state: RefCell<WindowState>,
    shown: Cell<bool>,
    maximized: Cell<bool>,
}

// DWM attributes which are not in all SDK versions.
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
const DWMWCP_ROUND: u32 = 2;
const DWMSBT_NONE: u32 = 1;
const DWMSBT_TRANSIENTWINDOW: u32 = 3;

fn set_dwm_u32(hwnd: HWND, attribute: u32, value: u32) {
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(attribute as i32),
            &value as *const u32 as *const _,
            4,
        );
    }
}

/// Resolves Kando's 'auto' flavor like Kando does on Windows.
fn resolve_flavor(app: &App) -> String {
    let flavor = app.general.borrow().settings_window_flavor.clone();
    if flavor != "auto" {
        return flavor;
    }
    let resolved = if shell::windows_build() >= 22621 {
        "transparent-system"
    } else {
        "sakura-system"
    };
    let mut value = app.general.borrow().to_value();
    value["settingsWindowFlavor"] = json!(resolved);
    app.set_general_settings(&value, None);
    resolved.to_string()
}

fn use_dark_frame(app: &App) -> bool {
    let general = app.general.borrow();
    match general.settings_window_color_scheme.as_str() {
        "dark" => true,
        "light" => false,
        _ => {
            if general.settings_window_flavor.ends_with("-dark") {
                true
            } else if general.settings_window_flavor.ends_with("-light") {
                false
            } else {
                shell::dark_mode()
            }
        }
    }
}

fn dpi_scale_at(rect: &RECT) -> f64 {
    unsafe {
        let monitor = MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST);
        let (mut x, mut y) = (96u32, 96u32);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y);
        x as f64 / 96.0
    }
}

/// The work area of the monitor under the pointer.
fn pointer_work_area() -> RECT {
    unsafe {
        let mut point = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut point);
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        info.rcWork
    }
}

fn work_area_of(rect: &RECT) -> RECT {
    unsafe {
        let monitor = MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        info.rcWork
    }
}

/// Shrinks and moves restored bounds so that they fit the work area of their monitor.
fn fit_bounds(bounds: Bounds) -> Bounds {
    let rect = RECT {
        left: bounds.x,
        top: bounds.y,
        right: bounds.x + bounds.width,
        bottom: bounds.y + bounds.height,
    };
    let area = work_area_of(&rect);
    let width = bounds.width.min(area.right - area.left).max(200);
    let height = bounds.height.min(area.bottom - area.top).max(200);
    let x = bounds
        .x
        .clamp(area.left, (area.right - width).max(area.left));
    let y = bounds
        .y
        .clamp(area.top, (area.bottom - height).max(area.top));
    Bounds {
        x,
        y,
        width,
        height,
    }
}

impl SettingsWindow {
    fn create(app: &App) -> windows::core::Result<SettingsWindow> {
        let state: WindowState = config::read_json(&app.files().window_state)
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let bounds = match state.bounds {
            Some(b) if b.width > 0 && b.height > 0 => fit_bounds(b),
            _ => {
                let area = pointer_work_area();
                let scale = dpi_scale_at(&area);
                let width = ((1350.0 * scale) as i32).min(area.right - area.left);
                let height = ((900.0 * scale) as i32).min(area.bottom - area.top);
                Bounds {
                    x: area.left + (area.right - area.left - width) / 2,
                    y: area.top + (area.bottom - area.top - height) / 2,
                    width,
                    height,
                }
            }
        };

        unsafe {
            let instance = GetModuleHandleW(None)?;
            let icon_res = PCWSTR(std::ptr::without_provenance(1));
            let icon = LoadIconW(Some(instance.into()), icon_res).ok();
            let sm_icon = LoadImageW(
                Some(instance.into()),
                icon_res,
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_DEFAULTCOLOR,
            )
            .ok()
            .map(|h| HICON(h.0));
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(settings_wndproc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hIcon: icon.unwrap_or_default(),
                hIconSm: sm_icon.or(icon).unwrap_or_default(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassExW(&class);

            let title = HSTRING::from(app.i18n.borrow().t(s::SETTINGS_WINDOW_TITLE));
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                &title,
                WS_POPUP
                    | WS_THICKFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_CLIPCHILDREN,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            Ok(SettingsWindow {
                hwnd,
                environment: RefCell::new(None),
                controller: RefCell::new(None),
                webview: RefCell::new(None),
                maximized: Cell::new(state.maximized),
                state: RefCell::new(state),
                shown: Cell::new(false),
            })
        }
    }

    fn controller(&self) -> Option<ICoreWebView2Controller> {
        self.controller.borrow().clone()
    }

    fn fit(&self) {
        if let Some(controller) = self.controller() {
            webview::fit_to_window(&controller, self.hwnd);
        }
    }

    fn apply_frame(&self, transparent: bool, dark: bool) {
        set_dwm_u32(self.hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
        set_dwm_u32(
            self.hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE.0 as u32,
            dark as u32,
        );
        let margins = if transparent {
            MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            }
        } else {
            MARGINS::default()
        };
        unsafe {
            let _ = DwmExtendFrameIntoClientArea(self.hwnd, &margins);
        }
        if shell::windows_build() >= 22621 {
            set_dwm_u32(
                self.hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                if transparent {
                    DWMSBT_TRANSIENTWINDOW
                } else {
                    DWMSBT_NONE
                },
            );
        }
        if let Some(controller) = self.controller()
            && let Ok(controller2) = controller.cast::<ICoreWebView2Controller2>()
        {
            let bg = if transparent {
                COREWEBVIEW2_COLOR {
                    A: 0,
                    R: 0,
                    G: 0,
                    B: 0,
                }
            } else if dark {
                COREWEBVIEW2_COLOR {
                    A: 255,
                    R: 32,
                    G: 32,
                    B: 32,
                }
            } else {
                COREWEBVIEW2_COLOR {
                    A: 255,
                    R: 245,
                    G: 245,
                    B: 245,
                }
            };
            unsafe {
                let _ = controller2.SetDefaultBackgroundColor(bg);
            }
        }
    }

    /// Shows the window once the page is ready (avoids flashing an empty window).
    fn show_when_ready(&self) {
        if self.shown.replace(true) {
            return;
        }
        unsafe {
            let command = if self.maximized.get() {
                SW_SHOWMAXIMIZED
            } else {
                SW_SHOWNORMAL
            };
            let _ = ShowWindow(self.hwnd, command);
            let _ = SetForegroundWindow(self.hwnd);
            if let Some(controller) = self.controller() {
                // The web view was created while the window was hidden; it does not
                // become visible on its own.
                webview::fit_to_window(&controller, self.hwnd);
                let _ = controller.SetIsVisible(true);
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }

    fn focus(&self) {
        unsafe {
            if IsIconic(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(self.hwnd);
        }
    }

    /// Persists the bounds, the maximized state and the sidebar widths. A window which was
    /// never shown has no meaningful placement (it is created hidden and restored), so
    /// nothing is written for it.
    pub fn save_state(&self, app: &App) {
        if !self.shown.get() {
            return;
        }
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        let mut state = self.state.borrow_mut();
        unsafe {
            if GetWindowPlacement(self.hwnd, &mut placement).is_ok() {
                // rcNormalPosition is in workspace coordinates (offset by top/left taskbar).
                // Convert back to screen coordinates so CreateWindowExW does not drift.
                let r = placement.rcNormalPosition;
                let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let (dx, dy) = if GetMonitorInfoW(monitor, &mut mi).as_bool() {
                    (mi.rcWork.left - mi.rcMonitor.left, mi.rcWork.top - mi.rcMonitor.top)
                } else {
                    (0, 0)
                };
                state.bounds = Some(Bounds {
                    x: r.left + dx,
                    y: r.top + dy,
                    width: r.right - r.left,
                    height: r.bottom - r.top,
                });
                state.maximized =
                    placement.showCmd == SW_SHOWMAXIMIZED.0 as u32 || IsZoomed(self.hwnd).as_bool();
            }
        }
        if let Err(e) = config::write_json(
            &app.files().window_state,
            &serde_json::to_value(&*state).unwrap_or_default(),
        ) {
            log!("failed to save the settings window state: {e}");
        }
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
        self.environment.borrow_mut().take();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Shows the settings window, creating it if necessary.
pub fn show(app: &App) {
    if let Some(window) = app.settings_window.borrow().clone() {
        window.focus();
        return;
    }

    let flavor = resolve_flavor(app);
    let transparent = flavor.starts_with("transparent") && shell::windows_build() >= 22621;
    let window = match SettingsWindow::create(app) {
        Ok(w) => Rc::new(w),
        Err(e) => {
            log!("failed to create the settings window: {e}");
            return;
        }
    };
    window.apply_frame(transparent, use_dark_frame(app));
    *app.settings_window.borrow_mut() = Some(window.clone());

    // Every failure below discards the window so that the next request starts over
    // instead of focusing a window which never becomes visible. Cleanup in WebView2
    // callbacks is deferred to the message loop.
    let started = std::time::Instant::now();
    let user_data = app.paths.settings_webview_data();
    let args = app.browser_args(1);
    let window_for_environment = window.clone();
    let result = webview::create_environment(&user_data, &args, move |environment| {
        let Some(app) = app::app() else { return };
        let window = window_for_environment;
        // The window may have been closed in the meantime.
        if !is_current(&app, &window) {
            return;
        }
        let environment = match environment {
            Ok(e) => e,
            Err(e) => {
                log!("settings environment creation failed: {e}");
                app.defer(move |app| discard(app, &window));
                return;
            }
        };
        *window.environment.borrow_mut() = Some(environment.clone());
        let window_for_callback = window.clone();
        let environment_for_icons = environment.clone();
        let result = webview::create_controller(&environment, window.hwnd, move |controller| {
            let Some(app) = app::app() else { return };
            let window = window_for_callback;
            if !is_current(&app, &window) {
                if let Ok(controller) = controller {
                    app.defer(move |_| unsafe {
                        let _ = controller.Close();
                    });
                }
                return;
            }
            let controller = match controller {
                Ok(c) => c,
                Err(e) => {
                    log!("settings controller creation failed: {e}");
                    app.defer(move |app| discard(app, &window));
                    return;
                }
            };
            let options = webview::ConfigureOptions {
                app_folder: &app.paths.web_dir,
                user_folder: &app.paths.config_dir,
                transparent,
                non_client_regions: true,
            };
            let webview = match webview::configure(&controller, &options) {
                Ok(w) => w,
                Err(e) => {
                    log!("settings web view configuration failed: {e}");
                    // The controller is not stored in the window yet; close it explicitly.
                    app.defer(move |app| {
                        unsafe {
                            let _ = controller.Close();
                        }
                        discard(app, &window);
                    });
                    return;
                }
            };
            app.attach_page(&webview, Page::Settings, window.hwnd);
            app.attach_icons(&webview, environment_for_icons);
            webview::fit_to_window(&controller, window.hwnd);
            unsafe {
                // Made visible when the page is ready (see show_when_ready).
                let _ = controller.SetIsVisible(false);
                let _ = webview.Navigate(&HSTRING::from(webview::app_url("settings/index.html")));
            }
            *window.controller.borrow_mut() = Some(controller);
            *window.webview.borrow_mut() = Some(webview);
            // The frame was applied before the controller existed; apply it again so that
            // opaque flavors get their background color (instead of WebView2's white).
            window.apply_frame(transparent, use_dark_frame(&app));
            log!("settings web view created after {:?}", started.elapsed());
        });
        if let Err(e) = result {
            log!("CreateCoreWebView2Controller (settings) failed: {e}");
            app.defer(move |app| discard(app, &window));
        }
    });
    if let Err(e) = result {
        log!("CreateCoreWebView2EnvironmentWithOptions (settings) failed: {e}");
        app.defer(move |app| discard(app, &window));
    }
}

/// Whether `window` is still the app's settings window.
fn is_current(app: &App, window: &Rc<SettingsWindow>) -> bool {
    app.settings_window
        .borrow()
        .as_ref()
        .is_some_and(|w| Rc::ptr_eq(w, window))
}

/// Destroys the settings window if it is still the current one. The window state is not
/// saved here; callers which close a shown window save it first (see WM_CLOSE).
pub fn discard(app: &App, window: &Rc<SettingsWindow>) {
    if !is_current(app, window) {
        return;
    }
    let _ = app.take_settings_window();
    window.destroy();
    apps::forget();
    app.on_settings_closed();
}

/// Discards the settings window if `hwnd` belongs to it (used after process failures).
pub fn discard_window(app: &App, hwnd: HWND) {
    if let Some(window) = window(app).filter(|w| w.hwnd == hwnd) {
        discard(app, &window);
    }
}

/// Updates the settings window's DWM dark mode, acrylic backdrop and WebView2 background
/// immediately when the system color scheme or settings window flavor changes.
pub fn on_dark_mode_changed(app: &App, _dark: bool) {
    if let Some(window) = app.settings_window.borrow().clone() {
        let flavor = app.general.borrow().settings_window_flavor.clone();
        let transparent = flavor.starts_with("transparent") && shell::windows_build() >= 22621;
        window.apply_frame(transparent, use_dark_frame(app));
    }
}

fn window(app: &App) -> Option<Rc<SettingsWindow>> {
    app.settings_window.borrow().clone()
}

fn owner(app: &App) -> HWND {
    window(app).map(|w| w.hwnd).unwrap_or_default()
}

/// Channels whose replies are sent asynchronously.
pub fn is_deferred(channel: &str) -> bool {
    matches!(
        channel,
        "settings-window.get-installed-apps"
            | "settings-window.open-file-picker"
            | "settings-window.export-menu"
            | "settings-window.import-menu"
            | "common.create-menu-item-for-file"
    )
}

fn installed_apps_json(cache: bool) -> Value {
    let apps = apps::installed();
    let _ = cache;
    Value::Array(
        apps.iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "name": a.name,
                    "command": format!("start \"\" \"{}\"", a.launch_target()),
                    "icon": a.name,
                    "iconTheme": "system",
                })
            })
            .collect(),
    )
}

/// Builds a menu item for a file dropped onto the editor.
fn item_for_dropped_file(name: &str, path: &str) -> Option<Value> {
    let launch_icon = ("rocket_launch", "material-symbols-rounded");
    let lower = path.to_ascii_lowercase();
    let is_program = [".exe", ".lnk", ".bat", ".cmd", ".com", ".appref-ms"]
        .iter()
        .any(|e| lower.ends_with(e));

    let button = |item_name: &str, icon: &str, theme: &str, action: Action| {
        serde_json::to_value(MenuItem::Button {
            name: item_name.to_string(),
            icon: icon.to_string(),
            icon_theme: theme.to_string(),
            angle: None,
            select_workflow: Some(Workflow {
                quick_select_key: None,
                actions: vec![action],
            }),
        })
        .ok()
    };

    if path.is_empty() || !is_program {
        // Apps dragged from the start menu may arrive without a path.
        if let Some(app) = apps::find(name).or_else(|| apps::find(path)) {
            return button(
                &app.name,
                &app.name,
                "system",
                Action::LaunchApp {
                    target: app.launch_target(),
                },
            );
        }
    }
    if path.is_empty() {
        return None;
    }

    if is_program {
        let stem = std::path::Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        return match apps::find(&stem) {
            Some(app) => button(
                &stem,
                &app.name,
                "system",
                Action::LaunchApp {
                    target: path.to_string(),
                },
            ),
            None => button(
                &stem,
                launch_icon.0,
                launch_icon.1,
                Action::LaunchApp {
                    target: path.to_string(),
                },
            ),
        };
    }

    let is_dir = std::path::Path::new(path).is_dir();
    let extension = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let icon = if is_dir {
        "folder"
    } else {
        match extension.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tif" | "tiff"
            | "heic" | "avif" => "image",
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" => "video_file",
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "opus" => "audio_file",
            "txt" | "md" | "log" | "csv" | "json" | "xml" | "ini" | "yaml" | "yml" => {
                "text_snippet"
            }
            _ => "draft",
        }
    };
    button(
        name,
        icon,
        "material-symbols-rounded",
        Action::OpenFile {
            path: path.to_string(),
        },
    )
}

/// Handles the channels of `SETTINGS_WINDOW_API`. Returns `None` for deferred replies
/// and unknown channels.
pub fn handle_request(app: &App, request: &Request) -> Option<Result<Value, String>> {
    let id = request.id;
    let result: Result<Value, String> = match request.channel.as_str() {
        "settings-window.ready" => {
            if let Some(window) = window(app) {
                window.show_when_ready();
            }
            Ok(Value::Null)
        }
        "settings-window.get-backend-info" => Ok(json!({
            "name": "Windows",
            "supportsShortcuts": true,
        })),
        "settings-window.get-version" => Ok(json!({
            "kandoVersion": config::VERSION,
            "chromeVersion": shell::webview2_version().unwrap_or_default(),
        })),
        "settings-window.get-wm-info" => {
            let p = crate::menu_window::Placement::at_pointer();
            let wa = p.work_area;
            let fg = unsafe { GetForegroundWindow() };
            let (app_name, window_name) = shell::window_app_and_title(fg);
            Ok(json!({
                "windowName": window_name,
                "appName": app_name,
                "pointerX": p.pointer.x,
                "pointerY": p.pointer.y,
                "workArea": { "x": wa.left, "y": wa.top, "width": wa.right - wa.left, "height": wa.bottom - wa.top },
            }))
        }
        "settings-window.get-sidebar-widths" => Ok(window(app)
            .map(|w| serde_json::to_value(&w.state.borrow().sidebar_widths).unwrap_or_default())
            .unwrap_or_else(|| json!({}))),
        "settings-window.set-sidebar-width" => {
            let width = request
                .arg(1)
                .as_f64()
                .map(|w| w.round() as i64)
                .filter(|w| *w > 0);
            if let (Some(window), Some(width)) = (window(app), width) {
                {
                    let mut state = window.state.borrow_mut();
                    match request.str_arg(0) {
                        "left" => state.sidebar_widths.left = Some(width),
                        "right" => state.sidebar_widths.right = Some(width),
                        _ => {}
                    }
                }
                window.save_state(app);
            }
            Ok(Value::Null)
        }
        "settings-window.get-current-menu" => Ok(json!(app.current_menu_index())),
        "settings-window.get-config-directory" => Ok(json!(app.paths.config_dir.to_string_lossy())),
        "settings-window.get-menu-themes-directory" => {
            let dir = app.paths.user_menu_themes();
            let _ = std::fs::create_dir_all(&dir);
            Ok(json!(dir.to_string_lossy()))
        }
        "settings-window.get-all-menu-themes" => Ok(themes::all_menu_themes(&app.paths)),
        "settings-window.get-menu-theme-presets" => {
            Ok(themes::presets(&app.paths, request.str_arg(0)))
        }
        "settings-window.export-menu-theme-preset" => {
            let theme_id = request.str_arg(0);
            let theme_id = if themes::valid_theme_id(theme_id) {
                theme_id
            } else {
                themes::DEFAULT_THEME
            };
            let colors = request.arg(1).clone();
            let name = request.str_arg(2).trim().to_string();
            let result = if !themes::valid_preset_name(&name) {
                Err("Invalid preset name. Only letters, numbers, hyphens, underscores, and spaces are allowed.".to_string())
            } else {
                let file = app.paths.user_presets(theme_id).join(format!("{name}.json"));
                config::write_json(&file, &json!({ "colors": colors }))
            };
            match result {
                Ok(()) => app.post_event(Page::Settings, "settings-window.presets-changed", vec![]),
                Err(error) => {
                    let i18n = app.i18n.borrow();
                    let title = i18n.t(s::SAVE_PRESET_ERROR_TITLE);
                    let message = i18n.t_with(s::SAVE_PRESET_ERROR_MESSAGE, &[("error", &error)]);
                    drop(i18n);
                    app.defer(move |app| {
                        dialogs::message(owner(app), Kind::Error, &title, &message, None);
                    });
                }
            }
            Ok(Value::Null)
        }
        "settings-window.open-menu-theme-presets-directory" => {
            let theme_id = request.str_arg(0);
            let theme_id = if themes::valid_theme_id(theme_id) {
                theme_id
            } else {
                themes::DEFAULT_THEME
            };
            let dir = app.paths.user_presets(theme_id);
            let _ = std::fs::create_dir_all(&dir);
            open_path(dir.to_string_lossy().into_owned());
            Ok(Value::Null)
        }
        "settings-window.reload-menu-theme" => {
            app.post_event(Page::Menu, "menu-window.reload-menu-theme", vec![]);
            app.post_event(
                Page::Settings,
                "settings-window.menu-themes-changed",
                vec![themes::all_menu_themes(&app.paths)],
            );
            Ok(Value::Null)
        }
        "settings-window.get-all-sound-themes" => Ok(themes::all_sound_themes(&app.paths)),
        "settings-window.get-sound-themes-directory" => {
            let dir = app.paths.user_sound_themes();
            let _ = std::fs::create_dir_all(&dir);
            Ok(json!(dir.to_string_lossy()))
        }
        "settings-window.reload-sound-theme" => {
            app.post_event(Page::Menu, "menu-window.reload-sound-theme", vec![]);
            app.post_event(
                Page::Settings,
                "settings-window.sound-themes-changed",
                vec![themes::all_sound_themes(&app.paths)],
            );
            Ok(Value::Null)
        }
        "settings-window.open-path" => {
            let path = request.str_arg(0).to_string();
            if !path.is_empty() {
                open_path(path);
            }
            Ok(Value::Null)
        }
        "settings-window.get-installed-apps" => {
            if let Some(id) = id {
                app::reply_from_worker(Page::Settings, id, || Ok(installed_apps_json(true)));
            }
            return None;
        }
        "common.create-menu-item-for-file" => {
            if let Some(id) = id {
                let name = request.str_arg(0).to_string();
                let path = request.files.first().cloned().unwrap_or_default();
                app::reply_from_worker(Page::Settings, id, move || {
                    Ok(item_for_dropped_file(&name, &path).unwrap_or(Value::Null))
                });
            }
            return None;
        }
        "settings-window.open-menu" => {
            if let Some(index) = request.arg(0).as_u64() {
                app.show_menu(ShowRequest::Index(index as usize));
            }
            Ok(Value::Null)
        }
        "settings-window.show-dev-tools" => {
            let page = if request.str_arg(0) == "menu-window" {
                Page::Menu
            } else {
                Page::Settings
            };
            app.open_dev_tools(page);
            Ok(Value::Null)
        }
        "settings-window.reload-icon-themes" => {
            app.post_event(Page::Menu, "menu-window.reload-icon-themes", vec![]);
            app.post_event(Page::Settings, "settings-window.reload-icon-themes", vec![]);
            Ok(Value::Null)
        }
        "settings-window.open-file-picker" => {
            if let Some(id) = id {
                let options = request.arg(0).clone();
                app.defer(move |app| {
                    let path = dialogs::open_file(owner(app), &options).unwrap_or_default();
                    app.reply(Page::Settings, id, Ok(json!(path)));
                });
            }
            return None;
        }
        "settings-window.backup-general-settings" | "settings-window.backup-menu-settings" => {
            let general = request.channel.ends_with("general-settings");
            app.defer(move |app| backup(app, general));
            Ok(Value::Null)
        }
        "settings-window.restore-general-settings" | "settings-window.restore-menu-settings" => {
            let general = request.channel.ends_with("general-settings");
            app.defer(move |app| restore(app, general));
            Ok(Value::Null)
        }
        "settings-window.export-menu" => {
            if let Some(id) = id {
                let index = request.arg(0).as_u64().map(|i| i as usize);
                app.defer(move |app| {
                    let result = index.map(|i| export_menu(app, i)).unwrap_or(false);
                    app.reply(Page::Settings, id, Ok(json!(result)));
                });
            }
            return None;
        }
        "settings-window.import-menu" => {
            if let Some(id) = id {
                app.defer(move |app| {
                    let result = import_menu(app);
                    app.reply(Page::Settings, id, Ok(json!(result)));
                });
            }
            return None;
        }
        "settings-window.minimize" => {
            if let Some(window) = window(app) {
                unsafe {
                    let _ = ShowWindow(window.hwnd, SW_MINIMIZE);
                }
            }
            Ok(Value::Null)
        }
        "settings-window.toggle-maximize" => {
            if let Some(window) = window(app) {
                unsafe {
                    let command = if IsZoomed(window.hwnd).as_bool() {
                        SW_RESTORE
                    } else {
                        SW_MAXIMIZE
                    };
                    let _ = ShowWindow(window.hwnd, command);
                }
            }
            Ok(Value::Null)
        }
        "settings-window.is-maximized" => Ok(json!(
            window(app)
                .map(|w| unsafe { IsZoomed(w.hwnd).as_bool() })
                .unwrap_or(false)
        )),
        "settings-window.close" => {
            if let Some(window) = window(app) {
                unsafe {
                    let _ = PostMessageW(Some(window.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            Ok(Value::Null)
        }
        _ => return None,
    };
    Some(result)
}

fn open_path(path: String) {
    app::spawn_worker(move || {
        let _ = shell::shell_execute(&path, None, None);
    });
}

fn backup(app: &App, general: bool) {
    let (source, name) = if general {
        (app.files().general.clone(), "general-settings-backup.json")
    } else {
        (app.files().menus.clone(), "menu-settings-backup.json")
    };
    app.flush_settings();
    let options = json!({
        "defaultPath": app.paths.config_dir.join(name).to_string_lossy(),
        "filters": [{ "name": "JSON", "extensions": ["json"] }],
    });
    if let Some(target) = dialogs::save_file(owner(app), &options)
        && let Err(e) = std::fs::copy(&source, &target)
    {
        log!("backup failed: {e}");
    }
}

fn restore(app: &App, general: bool) {
    let (warn_title, warn_message, error_title, ok_title, ok_message, name) = if general {
        (
            s::RESTORE_GENERAL_WARNING_TITLE,
            s::RESTORE_GENERAL_WARNING_MESSAGE,
            s::RESTORE_GENERAL_ERROR_TITLE,
            s::RESTORE_GENERAL_SUCCESS_TITLE,
            s::RESTORE_GENERAL_SUCCESS_MESSAGE,
            "general-settings-backup.json",
        )
    } else {
        (
            s::RESTORE_MENUS_WARNING_TITLE,
            s::RESTORE_MENUS_WARNING_MESSAGE,
            s::RESTORE_MENUS_ERROR_TITLE,
            s::RESTORE_MENUS_SUCCESS_TITLE,
            s::RESTORE_MENUS_SUCCESS_MESSAGE,
            "menu-settings-backup.json",
        )
    };
    let t = |key: &str| app.i18n.borrow().t(key);
    let confirmed = dialogs::message(
        owner(app),
        Kind::Warning,
        &t(warn_title),
        &t(warn_message),
        Some((&t(s::CANCEL), &t(s::RESTORE_BACKUP))),
    );
    if !confirmed {
        return;
    }
    let options = json!({
        "defaultPath": app.paths.config_dir.join(name).to_string_lossy(),
        "filters": [{ "name": "JSON", "extensions": ["json"] }],
    });
    let Some(path) = dialogs::open_file(owner(app), &options) else {
        return;
    };
    let value = match config::read_json(std::path::Path::new(&path)) {
        Ok(Some(value)) => value,
        Ok(None) => return,
        Err(e) => {
            dialogs::message(owner(app), Kind::Error, &t(error_title), &e, None);
            return;
        }
    };
    let result = if general {
        app.set_general_settings(&GeneralSettings::from_value(&value).to_value(), None);
        Ok(())
    } else {
        MenuSettings::from_value(&value)
            .and_then(|settings| app.set_menu_settings(&settings.to_value(), None))
    };
    match result {
        Ok(()) => {
            dialogs::message(owner(app), Kind::Info, &t(ok_title), &t(ok_message), None);
        }
        Err(e) => {
            dialogs::message(owner(app), Kind::Error, &t(error_title), &e, None);
        }
    }
}

fn export_menu(app: &App, index: usize) -> bool {
    let Some(menu) = app.menus.borrow().menus.get(index).cloned() else {
        return false;
    };
    let options = json!({
        "defaultPath": format!("{}.json", menu.root.name()),
        "filters": [{ "name": "JSON", "extensions": ["json"] }],
    });
    let Some(path) = dialogs::save_file(owner(app), &options) else {
        return false;
    };
    match config::write_json(
        std::path::Path::new(&path),
        &config::exported_menu_value(&menu),
    ) {
        Ok(()) => true,
        Err(e) => {
            log!("failed to export menu: {e}");
            false
        }
    }
}

fn import_menu(app: &App) -> bool {
    let options = json!({ "filters": [{ "name": "JSON", "extensions": ["json"] }] });
    let Some(path) = dialogs::open_file(owner(app), &options) else {
        return false;
    };
    let result = config::read_json(std::path::Path::new(&path))
        .and_then(|v| v.ok_or_else(|| "The file is empty.".to_string()))
        .and_then(|v| config::menu_from_exported(&v));
    match result {
        Ok(menu) => {
            let mut settings = app.menus.borrow().clone();
            settings.menus.push(menu);
            match app.set_menu_settings(&settings.to_value(), None) {
                Ok(()) => true,
                Err(e) => {
                    log!("failed to import menu: {e}");
                    false
                }
            }
        }
        Err(detail) => {
            let i18n = app.i18n.borrow();
            let title = i18n.t(s::IMPORT_MENU_ERROR_TITLE);
            let message = format!("{}\n\n{detail}", i18n.t(s::IMPORT_MENU_ERROR_MESSAGE));
            drop(i18n);
            dialogs::message(owner(app), Kind::Error, &title, &message, None);
            false
        }
    }
}

fn min_track_size(hwnd: HWND) -> (i32, i32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
    ((1000.0 * dpi) as i32, (700.0 * dpi) as i32)
}

extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some(app) = app::app() else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    let this = window(&app).filter(|w| w.hwnd == hwnd);
    match msg {
        WM_NCCALCSIZE => {
            if wparam.0 != 0 {
                let params = unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) };
                if unsafe { IsZoomed(hwnd).as_bool() } {
                    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                    let border_x = unsafe {
                        GetSystemMetricsForDpi(SM_CXFRAME, dpi)
                            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
                    };
                    let border_y = unsafe {
                        GetSystemMetricsForDpi(SM_CYFRAME, dpi)
                            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
                    };
                    params.rgrc[0].left += border_x;
                    params.rgrc[0].top += border_y;
                    params.rgrc[0].right -= border_x;
                    params.rgrc[0].bottom -= border_y;
                }
                return LRESULT(0);
            }
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            let (width, height) = min_track_size(hwnd);
            info.ptMinTrackSize.x = width;
            info.ptMinTrackSize.y = height;
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(monitor, &mut mi).as_bool() } {
                info.ptMaxPosition.x = mi.rcWork.left - mi.rcMonitor.left;
                info.ptMaxPosition.y = mi.rcWork.top - mi.rcMonitor.top;
                info.ptMaxSize.x = mi.rcWork.right - mi.rcWork.left;
                info.ptMaxSize.y = mi.rcWork.bottom - mi.rcWork.top;
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(window) = this {
                window.fit();
                // Hide the web view while minimized so that it stops rendering.
                if window.shown.get()
                    && let Some(controller) = window.controller()
                {
                    let minimized = wparam.0 as u32 == SIZE_MINIMIZED;
                    unsafe {
                        let _ = controller.SetIsVisible(!minimized);
                    }
                }
                let maximized = wparam.0 as u32 == SIZE_MAXIMIZED;
                if wparam.0 as u32 != SIZE_MINIMIZED
                    && window.maximized.replace(maximized) != maximized
                {
                    app.post_event(
                        Page::Settings,
                        "settings-window.maximized-changed",
                        vec![json!(maximized)],
                    );
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let rect = unsafe { &*(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(window) = this {
                window.save_state(&app);
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                discard(&app, &window);
            } else {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_files_become_open_file_items() {
        let dir = std::env::temp_dir();
        let item = item_for_dropped_file("tmp", &dir.to_string_lossy()).unwrap();
        assert_eq!(item["type"], "button");
        assert_eq!(item["icon"], "folder");
        assert_eq!(item["selectWorkflow"]["actions"][0]["type"], "open-file");

        let item = item_for_dropped_file("photo.JPG", r"C:\x\photo.JPG").unwrap();
        assert_eq!(item["icon"], "image");
    }

    #[test]
    fn dropped_programs_become_launch_app_items() {
        let item = item_for_dropped_file("Tool.exe", r"C:\Tools\Tool.exe").unwrap();
        assert_eq!(item["name"], "Tool");
        assert_eq!(item["selectWorkflow"]["actions"][0]["type"], "launch-app");
        assert_eq!(
            item["selectWorkflow"]["actions"][0]["target"],
            r"C:\Tools\Tool.exe"
        );
    }
}
