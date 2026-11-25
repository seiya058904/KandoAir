// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The application core: owns the settings, the tray icon, the global shortcuts and the
//! windows, and routes the messages of the renderers (the counterpart of Kando's
//! `src/main/app.ts` and `src/main/menu-window.ts`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

use serde_json::{Value, json};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::TrySuspendCompletedHandler;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Interface, w};

use crate::actions::{self, Failure, Placeholders};
use crate::config::{self, Action, Files, GeneralSettings, Menu, MenuItem, MenuSettings};
use crate::hotkey::{self, Hotkeys};
use crate::i18n::I18n;
use crate::ipc::{self, Request};
use crate::log;
use crate::menu_window::{MenuWindow, Placement};
use crate::paths::Paths;
use crate::settings_window::{self, SettingsWindow};
use crate::shell;
use crate::strings as s;
use crate::themes;
use crate::tray::{self, MenuEntry, Tray};
use crate::webview;

pub const WM_APP_WORKER: u32 = WM_APP + 2;
pub const WM_APP_DEFER: u32 = WM_APP + 3;
/// `COPYDATASTRUCT::dwData` of command lines forwarded by a second instance.
pub const COPYDATA_ARGS: usize = 0x4B41;
pub const HOST_CLASS: windows::core::PCWSTR = w!("KandoAirHostWindow");

const TIMER_MENU_HIDE: usize = 10;
const TIMER_MENU_IDLE: usize = 11;
const TIMER_SAVE_GENERAL: usize = 12;
const TIMER_SAVE_MENUS: usize = 13;
const TIMER_TRAY_CLEANUP: usize = 14;
const TIMER_RETRY_MENU: usize = 15;

/// Delay after hiding the menu before its web view is trimmed and suspended.
const MENU_IDLE_DELAY_MS: u32 = 5000;
/// Consecutive automatic attempts to recreate a failed menu web view. After that, the
/// menu is only recreated when it is requested again.
const MAX_MENU_RETRIES: u32 = 3;

const TRAY_SETTINGS: u32 = 1;
const TRAY_PAUSE: u32 = 2;
const TRAY_QUIT: u32 = 3;
const TRAY_MENU_BASE: u32 = 100;

/// Which page sent a message.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Page {
    Menu,
    Settings,
}

/// The reasons for showing a menu.
#[derive(Clone, Debug)]
pub enum ShowRequest {
    Trigger(String),
    Name(String),
    Index(usize),
}

/// Command line options (also forwarded by second instances).
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub settings: bool,
    pub menu: Option<String>,
    pub config_dir: Option<String>,
}

impl Options {
    pub fn parse(args: &[String]) -> Options {
        let mut options = Options::default();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-s" | "--settings" => options.settings = true,
                "-m" | "--menu" => {
                    options.menu = args.get(i + 1).cloned();
                    i += 1;
                }
                "--config-dir" => {
                    options.config_dir = args.get(i + 1).cloned();
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        options
    }
}

/// Results of work done on worker threads.
enum WorkerEvent {
    ActionFailed(Failure),
    HotkeyDone {
        inhibition: u32,
        failure: Option<Failure>,
    },
    Reply {
        page: Page,
        id: u64,
        result: Result<Value, String>,
    },
    Icon {
        id: u64,
        bytes: Option<Vec<u8>>,
    },
}

static WORKER_EVENTS: Mutex<Vec<WorkerEvent>> = Mutex::new(Vec::new());
static HOST_HWND: AtomicIsize = AtomicIsize::new(0);

fn post_worker_event(event: WorkerEvent) {
    WORKER_EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(event);
    let hwnd = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_WORKER, WPARAM(0), LPARAM(0));
    }
}

/// Runs `job` on a new thread with COM initialized for shell calls.
pub fn spawn_worker(job: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        }
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err() {
            log!("a worker thread panicked");
        }
        unsafe {
            CoUninitialize();
        }
    });
}

/// Runs `job` on a worker thread and sends its result as reply to an invocation.
pub fn reply_from_worker(
    page: Page,
    id: u64,
    job: impl FnOnce() -> Result<Value, String> + Send + 'static,
) {
    spawn_worker(move || {
        let result = job();
        post_worker_event(WorkerEvent::Reply { page, id, result });
    });
}

type EnvironmentCallback = Box<dyn FnOnce(&App, &ICoreWebView2Environment)>;
type DeferredTask = Box<dyn FnOnce(&App)>;

/// Bounded worker pool (3 COM-initialized threads) for rendering app icons without
/// spawning an OS thread per icon when AppPicker loads hundreds of apps.
static ICON_QUEUE: OnceLock<mpsc::Sender<(u64, String, PathBuf)>> = OnceLock::new();

fn enqueue_icon_job(id: u64, name: String, cache_dir: PathBuf) {
    let tx = ICON_QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(u64, String, PathBuf)>();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..3 {
            let rx = rx.clone();
            std::thread::spawn(move || {
                unsafe {
                    let _ =
                        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
                }
                loop {
                    let job = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                    let Ok((id, name, cache_dir)) = job else {
                        break;
                    };
                    let bytes = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::apps::icon_for_name(&cache_dir, &name)
                    }))
                    .unwrap_or(None);
                    post_worker_event(WorkerEvent::Icon { id, bytes });
                }
                unsafe {
                    CoUninitialize();
                }
            });
        }
        tx
    });
    let _ = tx.send((id, name, cache_dir));
}

#[derive(Default)]
struct MenuState {
    last_menu: Option<Menu>,
    /// Index per normalized shortcut used to cycle through menus with the same shortcut.
    menu_indices: HashMap<String, usize>,
    /// Inhibition of the open menu's own shortcut.
    inhibition: u32,
    opening_pointer: Option<POINT>,
    placement: Option<Placement>,
    placeholders: Placeholders,
    /// Whether the input focus should go back to the previous window when hiding.
    restore_focus: bool,
    /// A hotkey to simulate once the menu is hidden.
    pending_hotkey: Option<Vec<u16>>,
    /// A macro to replay once the menu is hidden.
    pending_macro: Option<Vec<actions::MacroKey>>,
    /// True after a close was requested from the renderer or the host.
    closing: bool,
}

pub struct App {
    pub paths: Paths,
    files: Files,
    host: Cell<HWND>,
    pub general: RefCell<GeneralSettings>,
    pub menus: RefCell<MenuSettings>,
    pub i18n: RefCell<I18n>,
    hotkeys: RefCell<Option<Hotkeys>>,
    tray: RefCell<Option<Tray>>,
    environment: RefCell<Option<ICoreWebView2Environment>>,
    environment_waiters: RefCell<Vec<EnvironmentCallback>>,
    environment_creating: Cell<bool>,
    menu_window: RefCell<Option<Rc<MenuWindow>>>,
    menu_ready: Cell<bool>,
    menu_retries: Cell<u32>,
    pending_show: RefCell<Option<ShowRequest>>,
    state: RefCell<MenuState>,
    pub settings_window: RefCell<Option<Rc<SettingsWindow>>>,
    dark_mode: Cell<bool>,
    pause_inhibition: Cell<u32>,
    notified_failures: RefCell<Vec<String>>,
    suspended: Cell<bool>,
    show_started: Cell<Option<Instant>>,
    taskbar_created: u32,
    deferred: RefCell<Vec<DeferredTask>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The application instance (available after startup).
pub fn app() -> Option<Rc<App>> {
    APP.with(|a| a.borrow().clone())
}

fn create_window(class: windows::core::PCWSTR, wndproc: WNDPROC) -> windows::core::Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: wndproc,
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            w!("KandoAir"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

impl App {
    /// Loads the settings, creates the host window, the tray icon and the shortcuts and
    /// starts loading the menu window.
    pub fn start(paths: Paths, options: Options) -> Rc<App> {
        let files = Files::new(&paths.config_dir);
        let mut general = config::load_general(&files);
        general.auto_start = shell::auto_start_enabled();
        let mut menus = config::load_menus(&files);
        let i18n = I18n::load(&paths.locales_dir(), &general.locale);

        let mut save_menus = false;
        if menus.menus.is_empty()
            && let Some(mut menu) = config::example_menu(&|key| i18n.t(key))
        {
            menu.shortcut = hotkey::default_shortcut(&menu.shortcut);
            menus.menus.push(menu);
            save_menus = true;
        }

        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

        let app = Rc::new(App {
            files,
            host: Cell::new(HWND::default()),
            general: RefCell::new(general),
            menus: RefCell::new(menus),
            i18n: RefCell::new(i18n),
            hotkeys: RefCell::new(None),
            tray: RefCell::new(None),
            environment: RefCell::new(None),
            environment_waiters: RefCell::new(Vec::new()),
            environment_creating: Cell::new(false),
            menu_window: RefCell::new(None),
            menu_ready: Cell::new(false),
            menu_retries: Cell::new(0),
            pending_show: RefCell::new(None),
            state: RefCell::new(MenuState::default()),
            settings_window: RefCell::new(None),
            dark_mode: Cell::new(shell::dark_mode()),
            pause_inhibition: Cell::new(0),
            notified_failures: RefCell::new(Vec::new()),
            suspended: Cell::new(false),
            show_started: Cell::new(None),
            taskbar_created,
            deferred: RefCell::new(Vec::new()),
            paths,
        });
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));

        let host = create_window(HOST_CLASS, Some(host_wndproc))
            .expect("failed to create the host window");
        app.host.set(host);
        HOST_HWND.store(host.0 as isize, Ordering::SeqCst);

        *app.hotkeys.borrow_mut() = Some(Hotkeys::new(host));
        *app.tray.borrow_mut() = Some(Tray::new(host, "KandoAir"));
        let flavor = app.general.borrow().tray_icon_flavor.clone();
        app.with_tray(|t| t.set_flavor(&flavor));

        app.bind_shortcuts();
        if save_menus {
            app.schedule_save_menus();
            app.notify_first_start();
        }

        if !app.general.borrow().lazy_initialization {
            app.ensure_menu_window();
        }
        app.handle_options(&options, false);
        app
    }

    fn with_tray<R>(&self, f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
        self.tray.borrow_mut().as_mut().map(f)
    }

    fn with_hotkeys<R>(&self, f: impl FnOnce(&mut Hotkeys) -> R) -> Option<R> {
        self.hotkeys.borrow_mut().as_mut().map(f)
    }

    // ---------------------------------------------------------------------------------- //
    // Notifications
    // ---------------------------------------------------------------------------------- //

    pub fn notify(&self, title: &str, message: &str, error: bool) {
        let temporary = self
            .with_tray(|t| t.notify(title, message, error))
            .unwrap_or(false);
        if temporary {
            unsafe {
                SetTimer(Some(self.host.get()), TIMER_TRAY_CLEANUP, 10_000, None);
            }
        }
    }

    fn notify_failure(&self, failure: &Failure) {
        let i18n = self.i18n.borrow();
        let title = i18n.t(s::ACTION_FAILED_TITLE);
        let message = match failure {
            Failure::NotFound(path) => i18n.t_with(s::PATH_NOT_FOUND, &[("path", path)]),
            Failure::Other(error) => i18n.t_with(s::COMMAND_FAILED, &[("error", error)]),
            Failure::TargetElevated => i18n.t(s::TARGET_ELEVATED),
        };
        drop(i18n);
        self.notify(&title, &message, true);
    }

    fn report_shortcut_failures(&self, failures: Vec<hotkey::Failure>) {
        let mut notified = self.notified_failures.borrow_mut();
        let previous = std::mem::take(&mut *notified);
        let mut new = Vec::new();
        for failure in failures {
            // A shortcut may fail twice (taken by another program and an input method).
            if notified.contains(&failure.shortcut) {
                continue;
            }
            notified.push(failure.shortcut.clone());
            if !previous.contains(&failure.shortcut) {
                new.push(failure);
            }
        }
        drop(notified);
        for failure in new {
            let i18n = self.i18n.borrow();
            let title = i18n.t(s::SHORTCUT_CONFLICT_TITLE);
            let key = if failure.input_method {
                s::SHORTCUT_IME_CONFLICT_MESSAGE
            } else {
                s::SHORTCUT_CONFLICT_MESSAGE
            };
            let shortcut = hotkey::display(&failure.shortcut);
            let message = i18n.t_with(key, &[("shortcut", &shortcut)]);
            drop(i18n);
            self.notify(&title, &message, true);
        }
    }

    /// Tells new users how to open the example menu (unless its shortcut is unavailable).
    fn notify_first_start(&self) {
        let Some(shortcut) = self
            .menus
            .borrow()
            .menus
            .first()
            .map(|m| m.shortcut.clone())
        else {
            return;
        };
        if shortcut.is_empty() || self.notified_failures.borrow().contains(&shortcut) {
            return;
        }
        let i18n = self.i18n.borrow();
        let title = i18n.t(s::FIRST_START_TITLE);
        let message = i18n.t_with(
            s::FIRST_START_MESSAGE,
            &[("shortcut", &hotkey::display(&shortcut))],
        );
        drop(i18n);
        self.notify(&title, &message, false);
    }

    // ---------------------------------------------------------------------------------- //
    // Settings
    // ---------------------------------------------------------------------------------- //

    fn schedule_save_general(&self) {
        unsafe {
            SetTimer(Some(self.host.get()), TIMER_SAVE_GENERAL, 300, None);
        }
    }

    fn schedule_save_menus(&self) {
        unsafe {
            SetTimer(Some(self.host.get()), TIMER_SAVE_MENUS, 300, None);
        }
    }

    fn save_general_now(&self) {
        unsafe {
            let _ = KillTimer(Some(self.host.get()), TIMER_SAVE_GENERAL);
        }
        let value = self.general.borrow().to_value();
        if let Err(e) = config::write_json(&self.files.general, &value) {
            log!("failed to save general settings: {e}");
        }
    }

    fn save_menus_now(&self) {
        unsafe {
            let _ = KillTimer(Some(self.host.get()), TIMER_SAVE_MENUS);
        }
        let value = self.menus.borrow().to_value();
        if let Err(e) = config::write_json(&self.files.menus, &value) {
            log!("failed to save menu settings: {e}");
        }
    }

    pub fn flush_settings(&self) {
        self.save_general_now();
        self.save_menus_now();
    }

    pub fn files(&self) -> &Files {
        &self.files
    }

    fn bind_shortcuts(&self) {
        let shortcuts: Vec<String> = self
            .menus
            .borrow()
            .menus
            .iter()
            .map(|m| m.shortcut.clone())
            .collect();
        let failures = self
            .with_hotkeys(|h| h.bind(&shortcuts))
            .unwrap_or_default();
        self.report_shortcut_failures(failures);
    }

    /// Replaces the general settings, persists them and notifies the pages.
    pub fn set_general_settings(&self, value: &Value, from: Option<Page>) {
        let new = GeneralSettings::from_value(value);
        let old = self.general.borrow().clone();
        if new == old {
            return;
        }
        *self.general.borrow_mut() = new.clone();
        self.schedule_save_general();

        if new.tray_icon_flavor != old.tray_icon_flavor {
            self.with_tray(|t| t.set_flavor(&new.tray_icon_flavor));
        }
        if new.auto_start != old.auto_start
            && let Err(e) = shell::set_auto_start(new.auto_start)
        {
            log!("failed to update auto-start setting: {e}");
        }
        if new.settings_window_color_scheme != old.settings_window_color_scheme
            || new.settings_window_flavor != old.settings_window_flavor
        {
            settings_window::on_dark_mode_changed(self, self.dark_mode.get());
        }
        if let Some(menu) = self.menu_window.borrow().clone() {
            if new.zoom_factor != old.zoom_factor {
                menu.set_zoom(new.zoom_factor);
            }
            if new.keep_input_focus != old.keep_input_focus {
                menu.set_keep_input_focus(new.keep_input_focus);
            }
        }

        let args = vec![new.to_value(), old.to_value()];
        self.post_event(Page::Menu, "common.general-settings-changed", args.clone());
        if from != Some(Page::Settings) {
            self.post_event(Page::Settings, "common.general-settings-changed", args);
        }
    }

    /// Replaces the menu settings (partial updates are merged), persists them and
    /// notifies the pages.
    pub fn set_menu_settings(&self, value: &Value, from: Option<Page>) -> Result<(), String> {
        let old = self.menus.borrow().clone();
        let mut merged = old.to_value();
        if let (Some(target), Some(source)) = (merged.as_object_mut(), value.as_object()) {
            for (key, v) in source {
                target.insert(key.clone(), v.clone());
            }
        }
        let new = MenuSettings::from_value(&merged)?;
        if new == old {
            return Ok(());
        }
        *self.menus.borrow_mut() = new.clone();
        self.schedule_save_menus();

        // While a menu is shown, its shortcut is inhibited; the bindings are updated when
        // the menu is hidden.
        let menu_visible = self
            .menu_window
            .borrow()
            .as_ref()
            .map(|m| m.visible.get())
            .unwrap_or(false);
        if !menu_visible {
            self.bind_shortcuts();
        }

        // Menu page receives the active menu tree on show-menu and does not listen to
        // menu-settings-changed; avoid waking the suspended menu WebView2 on every edit.
        if from != Some(Page::Settings) {
            let args = vec![new.to_value(), old.to_value()];
            self.post_event(Page::Settings, "common.menu-settings-changed", args);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------- //
    // WebView2 environment
    // ---------------------------------------------------------------------------------- //

    /// Calls `callback` once the shared WebView2 environment is available.
    pub fn with_environment(
        &self,
        callback: impl FnOnce(&App, &ICoreWebView2Environment) + 'static,
    ) {
        if let Some(environment) = self.environment.borrow().clone() {
            callback(self, &environment);
            return;
        }
        self.environment_waiters
            .borrow_mut()
            .push(Box::new(callback));
        if self.environment_creating.replace(true) {
            return;
        }

        let started = Instant::now();
        let result = webview::create_environment(
            &self.paths.webview_data(),
            &self.browser_args(0),
            move |result| {
                let Some(app) = app() else { return };
                app.environment_creating.set(false);
                match result {
                    Ok(environment) => {
                        log!("WebView2 environment ready after {:?}", started.elapsed());
                        *app.environment.borrow_mut() = Some(environment.clone());
                        let waiters = std::mem::take(&mut *app.environment_waiters.borrow_mut());
                        for waiter in waiters {
                            waiter(&app, &environment);
                        }
                    }
                    Err(e) => {
                        log!("WebView2 environment creation failed: {e}");
                        app.environment_waiters.borrow_mut().clear();
                        app.defer(|app| app.on_menu_environment_failed());
                    }
                }
            },
        );
        if let Err(e) = result {
            log!("CreateCoreWebView2EnvironmentWithOptions failed: {e}");
            self.environment_creating.set(false);
            self.environment_waiters.borrow_mut().clear();
            self.defer(|app| app.on_menu_environment_failed());
        }
    }

    /// The command line arguments of WebView2 browser processes. `port_offset` separates
    /// the remote debugging ports of the menu and the settings browser processes.
    pub fn browser_args(&self, port_offset: u16) -> String {
        let mut args = Vec::new();
        if !self.general.borrow().hardware_acceleration {
            args.push("--disable-gpu".to_string());
        }
        if let Some(port) = std::env::var("KANDOAIR_DEBUG_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
        {
            args.push(format!("--remote-debugging-port={}", port + port_offset));
        }
        args.join(" ")
    }

    /// Connects a web view to the message router and the navigation guard. `hwnd` is the
    /// window hosting the web view; process failures are only handled for that window.
    pub fn attach_page(&self, webview: &ICoreWebView2, page: Page, hwnd: HWND) {
        let _ = webview::on_message(webview, move |json, source, files| {
            if !webview::is_app_url(&source) {
                log!("ignoring message from {source}");
                return;
            }
            let Some(app) = app() else { return };
            match ipc::parse(&json, files) {
                Some(request) => app.handle_request(page, request),
                None => log!("invalid message: {json}"),
            }
        });
        let _ = webview::guard_navigation(webview, |url| {
            spawn_worker(move || {
                let _ = shell::shell_execute(&url, None, None);
            });
        });
        let _ = webview::on_process_failed(webview, move |kind| {
            log!("WebView2 process failed ({page:?}): {}", kind.0);
            // Recover from the message loop instead of inside the WebView2 event handler.
            if let Some(app) = app() {
                app.defer(move |app| app.on_process_failed(page, kind, hwnd));
            }
        });
    }

    fn on_process_failed(&self, page: Page, kind: COREWEBVIEW2_PROCESS_FAILED_KIND, hwnd: HWND) {
        let render_gone = kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
            || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE
            || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
        if !render_gone {
            return;
        }
        match page {
            Page::Menu => {
                // Ignore repeated or late reports for a menu window which is already gone.
                if self.menu().is_none_or(|m| m.hwnd != hwnd) {
                    return;
                }
                if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
                    *self.environment.borrow_mut() = None;
                }
                // Release any active hotkey inhibition and recreate the menu web view.
                let inhibition = {
                    let mut state = self.state.borrow_mut();
                    state.closing = false;
                    state.pending_hotkey = None;
                    state.pending_macro = None;
                    std::mem::take(&mut state.inhibition)
                };
                if inhibition > 0 {
                    let _ = self.with_hotkeys(|h| h.release(inhibition));
                }
                self.bind_shortcuts();
                self.discard_menu_window(hwnd);
                self.schedule_menu_retry();
            }
            Page::Settings => settings_window::discard_window(self, hwnd),
        }
    }

    // ---------------------------------------------------------------------------------- //
    // Messages to pages
    // ---------------------------------------------------------------------------------- //

    fn page_webview(&self, page: Page) -> Option<ICoreWebView2> {
        match page {
            Page::Menu => self
                .menu_window
                .borrow()
                .as_ref()
                .and_then(|m| m.webview.borrow().clone()),
            Page::Settings => self
                .settings_window
                .borrow()
                .as_ref()
                .and_then(|w| w.webview.borrow().clone()),
        }
    }

    pub fn post_event(&self, page: Page, channel: &str, args: Vec<Value>) {
        if page == Page::Menu && !self.menu_ready.get() {
            return;
        }
        if page == Page::Menu {
            self.resume_menu();
        }
        if let Some(webview) = self.page_webview(page) {
            webview::post_json(&webview, &ipc::event(channel, args));
        }
    }

    pub fn reply(&self, page: Page, id: u64, result: Result<Value, String>) {
        if let Some(webview) = self.page_webview(page) {
            webview::post_json(&webview, &ipc::reply(id, result));
        }
    }

    // ---------------------------------------------------------------------------------- //
    // Request routing
    // ---------------------------------------------------------------------------------- //

    fn handle_request(&self, page: Page, request: Request) {
        let id = request.id;
        let result = match self.handle_common(page, &request) {
            Some(result) => Some(result),
            None => match page {
                Page::Menu => self.handle_menu_request(&request),
                Page::Settings => settings_window::handle_request(self, &request),
            },
        };
        match (id, result) {
            (Some(id), Some(result)) => self.reply(page, id, result),
            (Some(id), None) => {
                // Deferred replies are sent later by the handler. Unknown channels fail.
                if !settings_window::is_deferred(&request.channel) {
                    log!("unknown channel {}", request.channel);
                    self.reply(
                        page,
                        id,
                        Err(format!("unknown channel {}", request.channel)),
                    );
                }
            }
            (None, _) => {}
        }
    }

    /// Channels of `COMMON_WINDOW_API`. Returns `None` for other channels.
    fn handle_common(&self, page: Page, request: &Request) -> Option<Result<Value, String>> {
        let result = match request.channel.as_str() {
            "common.log" => {
                log!("[{page:?}] {}", request.str_arg(0));
                Ok(Value::Null)
            }
            "common.get-locales" => Ok(self.i18n.borrow().renderer_payload()),
            "common.general-settings-get" => Ok(self.general.borrow().to_value()),
            "common.general-settings-set" => {
                self.set_general_settings(request.arg(0), Some(page));
                Ok(Value::Null)
            }
            "common.menu-settings-get" => Ok(self.menus.borrow().to_value()),
            "common.menu-settings-set" => {
                if let Err(e) = self.set_menu_settings(request.arg(0), Some(page)) {
                    log!("rejected menu settings: {e}");
                }
                Ok(Value::Null)
            }
            "common.get-icon-themes" => Ok(themes::icon_themes_info(&self.paths)),
            "common.get-menu-theme" => Ok(themes::current_menu_theme(
                &self.paths,
                &self.general.borrow(),
                self.dark_mode.get(),
            )),
            "common.get-current-menu-theme-colors" => Ok(themes::current_colors(
                &self.paths,
                &self.general.borrow(),
                self.dark_mode.get(),
            )),
            "common.get-sound-theme" => Ok(themes::current_sound_theme(
                &self.paths,
                &self.general.borrow(),
            )),
            "common.get-is-dark-mode" => Ok(json!(self.dark_mode.get())),
            _ => return None,
        };
        Some(result)
    }

    fn handle_menu_request(&self, request: &Request) -> Option<Result<Value, String>> {
        match request.channel.as_str() {
            "menu-window.ready" => {
                log!("menu page ready");
                self.menu_ready.set(true);
                self.menu_retries.set(0);
                // Trim and suspend the menu until it is shown for the first time.
                unsafe {
                    SetTimer(
                        Some(self.host.get()),
                        TIMER_MENU_IDLE,
                        MENU_IDLE_DELAY_MS,
                        None,
                    );
                }
                let pending = self.pending_show.borrow_mut().take();
                if let Some(pending) = pending {
                    self.show_menu(pending);
                }
            }
            "menu-window.shown" => {
                if let Some(started) = self.show_started.take() {
                    log!(
                        "menu shown: hotkey to first frame {:.1} ms",
                        started.elapsed().as_secs_f64() * 1000.0
                    );
                }
            }
            "menu-window.move-pointer" => self.move_pointer(request.arg(0)),
            "menu-window.finalize-interaction" => {
                let kind = request.str_arg(0).to_string();
                let path: Vec<usize> = request
                    .arg(1)
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as usize))
                            .collect()
                    })
                    .unwrap_or_default();
                self.on_interaction(&kind, &path);
            }
            "menu-window.show-settings" => self.show_settings(),
            _ => return None,
        }
        Some(Ok(Value::Null))
    }

    // ---------------------------------------------------------------------------------- //
    // Menu window
    // ---------------------------------------------------------------------------------- //

    fn ensure_menu_window(&self) {
        if self.menu_window.borrow().is_some() {
            return;
        }
        let keep_focus = self.general.borrow().keep_input_focus;
        let menu = match MenuWindow::create(Some(menu_wndproc), keep_focus) {
            Ok(m) => Rc::new(m),
            Err(e) => {
                log!("failed to create the menu window: {e}");
                return;
            }
        };
        *self.menu_window.borrow_mut() = Some(menu.clone());
        self.menu_ready.set(false);

        let started = Instant::now();
        let hwnd = menu.hwnd;
        self.with_environment(move |this, environment| {
            let menu_for_callback = menu.clone();
            let result = webview::create_controller(environment, menu.hwnd, move |controller| {
                let Some(app) = app() else { return };
                let menu = menu_for_callback;
                // The window may have been discarded in the meantime.
                if app.menu().is_none_or(|m| !Rc::ptr_eq(&m, &menu)) {
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
                        log!("menu controller creation failed: {e}");
                        app.defer(move |app| app.on_menu_web_view_failed(hwnd));
                        return;
                    }
                };
                let options = webview::ConfigureOptions {
                    app_folder: &app.paths.web_dir,
                    user_folder: &app.paths.config_dir,
                    transparent: true,
                    non_client_regions: false,
                };
                let webview = match webview::configure(&controller, &options) {
                    Ok(w) => w,
                    Err(e) => {
                        log!("menu web view configuration failed: {e}");
                        app.defer(move |app| {
                            unsafe {
                                let _ = controller.Close();
                            }
                            app.on_menu_web_view_failed(hwnd);
                        });
                        return;
                    }
                };
                app.attach_page(&webview, Page::Menu, hwnd);
                if let Some(environment) = app.environment.borrow().clone() {
                    app.attach_icons(&webview, environment);
                }
                unsafe {
                    let _ = controller.SetIsVisible(false);
                    let _ = controller.SetZoomFactor(app.general.borrow().zoom_factor);
                    let _ = webview.Navigate(&HSTRING::from(webview::app_url("menu/index.html")));
                }
                *menu.controller.borrow_mut() = Some(controller);
                *menu.webview.borrow_mut() = Some(webview);
                log!("menu web view created after {:?}", started.elapsed());
            });
            if let Err(e) = result {
                log!("CreateCoreWebView2Controller failed: {e}");
                this.defer(move |app| app.on_menu_web_view_failed(hwnd));
            }
        });
    }

    fn menu(&self) -> Option<Rc<MenuWindow>> {
        self.menu_window.borrow().clone()
    }

    /// Removes the menu window from the app. The borrow ends before the caller destroys
    /// the window, whose window procedure may access the app again.
    fn take_menu_window(&self) -> Option<Rc<MenuWindow>> {
        self.menu_window.borrow_mut().take()
    }

    /// Removes the settings window from the app (see [`App::take_menu_window`]).
    pub fn take_settings_window(&self) -> Option<Rc<SettingsWindow>> {
        self.settings_window.borrow_mut().take()
    }

    /// Destroys the menu window if `hwnd` is still the current one.
    fn discard_menu_window(&self, hwnd: HWND) {
        if self.menu().is_none_or(|m| m.hwnd != hwnd) {
            return;
        }
        let menu = self.take_menu_window();
        self.menu_ready.set(false);
        self.suspended.set(false);
        if let Some(menu) = menu {
            menu.destroy();
        }
    }

    /// The menu web view could not be created: start over with a fresh window.
    fn on_menu_web_view_failed(&self, hwnd: HWND) {
        if self.menu().is_none_or(|m| m.hwnd != hwnd) {
            return;
        }
        self.discard_menu_window(hwnd);
        self.schedule_menu_retry();
    }

    fn on_menu_environment_failed(&self) {
        if let Some(menu) = self.menu() {
            self.on_menu_web_view_failed(menu.hwnd);
        }
    }

    /// Recreates the menu web view after a second, up to [`MAX_MENU_RETRIES`] times in a
    /// row. After that, it is recreated when a menu is requested again.
    fn schedule_menu_retry(&self) {
        let attempt = self.menu_retries.get() + 1;
        self.menu_retries.set(attempt);
        if attempt <= MAX_MENU_RETRIES {
            unsafe {
                SetTimer(Some(self.host.get()), TIMER_RETRY_MENU, 1000, None);
            }
        } else {
            log!("menu web view failed {attempt} times in a row; waiting for the next request");
            self.pending_show.borrow_mut().take();
        }
    }

    fn resume_menu(&self) {
        if !self.suspended.replace(false) {
            return;
        }
        if let Some(menu) = self.menu() {
            menu.resume();
            if !menu.visible.get() {
                unsafe {
                    SetTimer(
                        Some(self.host.get()),
                        TIMER_MENU_IDLE,
                        MENU_IDLE_DELAY_MS,
                        None,
                    );
                }
            }
        }
    }

    fn suspend_menu_if_idle(&self) {
        let Some(menu) = self.menu() else { return };
        if menu.visible.get() || self.settings_window.borrow().is_some() || self.suspended.get() {
            return;
        }
        if menu.suspend() {
            self.suspended.set(true);
            shell::trim_working_set();
        }
    }

    /// Shows a menu. This is called for shortcuts, tray entries, previews and the
    /// command line (the counterpart of Kando's `MenuWindow.showMenu`).
    pub fn show_menu(&self, request: ShowRequest) {
        let Some(menu_window) = self.menu() else {
            self.ensure_menu_window();
            *self.pending_show.borrow_mut() = Some(request);
            return;
        };
        if !self.menu_ready.get() {
            *self.pending_show.borrow_mut() = Some(request);
            return;
        }

        // Start waking the suspended WebView2 renderer immediately while Rust evaluates
        // conditions and prepares the payload in parallel.
        self.resume_menu();

        let behavior = self.general.borrow().same_shortcut_behavior.clone();
        // A menu that is already fading out should not block or cycle a new trigger.
        let visible = menu_window.visible.get() && !self.state.borrow().closing;
        let mut refresh_pointer_only = false;

        if let ShowRequest::Trigger(trigger) = &request {
            if visible {
                let same = self
                    .state
                    .borrow()
                    .last_menu
                    .as_ref()
                    .is_some_and(|last| hotkey::same_shortcut(&last.shortcut, trigger));
                if same {
                    match behavior.as_str() {
                        "nothing" => return,
                        "close" => {
                            self.request_close(true);
                            return;
                        }
                        "re-open" => {
                            refresh_pointer_only = true;
                        }
                        _ => {
                            let mut state = self.state.borrow_mut();
                            *state.menu_indices.entry(trigger.clone()).or_insert(0) += 1;
                        }
                    }
                } else if behavior == "cycle-from-first" {
                    self.state
                        .borrow_mut()
                        .menu_indices
                        .insert(trigger.clone(), 0);
                }
            } else if behavior == "cycle-from-first" {
                self.state
                    .borrow_mut()
                    .menu_indices
                    .insert(trigger.clone(), 0);
            }
        }

        // Capture foreground window & pointer first so trigger conditions can match them.
        // For "re-open", keep the previously captured foreground app/title and refresh only
        // the pointer coordinates so conditional menus still match without flicker.
        let (captured_prev, captured_placeholders, captured_pointer) = if !visible {
            let mut previous = unsafe { GetForegroundWindow() };
            if previous == self.host.get() || previous == menu_window.hwnd {
                previous = HWND::default();
            }
            let (app_name, window_name) = shell::window_app_and_title(previous);
            let mut pointer = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut pointer);
            }
            let placeholders = Placeholders {
                app_name,
                window_name,
                pointer_x: pointer.x,
                pointer_y: pointer.y,
            };
            (Some(previous), placeholders, Some(pointer))
        } else if refresh_pointer_only {
            let mut pointer = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut pointer);
            }
            let mut placeholders = self.state.borrow().placeholders.clone();
            placeholders.pointer_x = pointer.x;
            placeholders.pointer_y = pointer.y;
            (None, placeholders, Some(pointer))
        } else {
            let state = self.state.borrow();
            (None, state.placeholders.clone(), state.opening_pointer)
        };

        let Some(menu) = self.choose_menu(&request, &behavior, &captured_placeholders) else {
            log!("no menu found for {request:?}");
            return;
        };

        let general = self.general.borrow().clone();

        // Release the shortcut while the menu is open so that key repeats reach the menu.
        if behavior == "nothing" && !menu.shortcut.is_empty() {
            let previous = std::mem::take(&mut self.state.borrow_mut().inhibition);
            let id = self
                .with_hotkeys(|h| {
                    if previous > 0 {
                        // Re-registration failures are reported when the menu closes.
                        let _ = h.release(previous);
                    }
                    h.inhibit(&menu.shortcut)
                })
                .unwrap_or(0);
            self.state.borrow_mut().inhibition = id;
        }

        if let Some(previous) = captured_prev {
            menu_window.previous_foreground.set(previous);
            let mut state = self.state.borrow_mut();
            state.placeholders = captured_placeholders;
            state.opening_pointer = captured_pointer;
        } else if refresh_pointer_only {
            let mut state = self.state.borrow_mut();
            state.placeholders = captured_placeholders;
            state.opening_pointer = captured_pointer;
        }

        let placement = Placement::at_pointer();
        // Position and fit the window BEFORE posting show-menu so monitor/DPI transitions
        // render with the right viewport bounds on the very first frame.
        menu_window.prepare_bounds(&placement);
        let zoom = general.zoom_factor;
        let (x, y) = placement.css_pointer(zoom);
        let (width, height) = placement.css_size(zoom);
        let options = json!({
            "mousePosition": { "x": x, "y": y },
            "windowSize": { "x": width, "y": height },
            "zoomFactor": zoom,
            "useFixedPosition": menu.use_fixed_position,
            "fixedMenuPosition": menu.fixed_menu_position,
            "anchoredMode": menu.anchored,
            "hoverMode": menu.hover_mode,
            "systemIconsChanged": false,
        });
        let info = {
            let state = self.state.borrow();
            json!({
                "appName": state.placeholders.app_name,
                "windowName": state.placeholders.window_name,
                "windowPosition": { "x": placement.work_area.left, "y": placement.work_area.top },
            })
        };
        let root = serde_json::to_value(&menu.root).unwrap_or_default();

        {
            let mut state = self.state.borrow_mut();
            state.last_menu = Some(menu);
            state.placement = Some(placement);
            state.restore_focus = true;
            state.pending_hotkey = None;
            state.pending_macro = None;
            state.closing = false;
        }

        self.post_event(
            Page::Menu,
            "menu-window.show-menu",
            vec![root, options, info],
        );
        unsafe {
            let _ = KillTimer(Some(self.host.get()), TIMER_MENU_HIDE);
            let _ = KillTimer(Some(self.host.get()), TIMER_MENU_IDLE);
        }
        menu_window.show(&placement, general.keep_input_focus);
    }

    fn choose_menu(
        &self,
        request: &ShowRequest,
        behavior: &str,
        context: &Placeholders,
    ) -> Option<Menu> {
        let menus = self.menus.borrow();
        let candidates: Vec<&Menu> = match request {
            ShowRequest::Index(index) => return menus.menus.get(*index).cloned(),
            ShowRequest::Name(name) => menus
                .menus
                .iter()
                .filter(|m| m.root.name() == name)
                .collect(),
            ShowRequest::Trigger(trigger) => {
                let matching: Vec<(&Menu, usize)> = menus
                    .menus
                    .iter()
                    .filter(|m| hotkey::same_shortcut(&m.shortcut, trigger))
                    .filter_map(|m| score_menu_conditions(m, context).map(|score| (m, score)))
                    .collect();
                let best_score = matching.iter().map(|(_, s)| *s).max()?;
                matching
                    .into_iter()
                    .filter(|(_, s)| *s == best_score)
                    .map(|(m, _)| m)
                    .collect()
            }
        };
        if candidates.is_empty() {
            return None;
        }
        let index = if behavior.starts_with("cycle")
            && let ShowRequest::Trigger(trigger) = request
        {
            self.state
                .borrow()
                .menu_indices
                .get(trigger)
                .copied()
                .unwrap_or(0)
                % candidates.len()
        } else {
            0
        };
        Some(candidates[index].clone())
    }

    /// Asks the renderer to fade out the menu. It reports back with a closeMenu
    /// interaction; a fallback timer hides the window if it does not.
    fn request_close(&self, restore_focus: bool) {
        let Some(menu) = self.menu() else { return };
        if !menu.visible.get() {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.restore_focus = restore_focus;
            state.closing = true;
        }
        self.post_event(
            Page::Menu,
            "menu-window.trigger-interaction",
            vec![json!("closeMenu")],
        );
        let fade = self.general.borrow().fade_out_duration as u32;
        unsafe {
            SetTimer(Some(self.host.get()), TIMER_MENU_HIDE, fade + 400, None);
        }
    }

    fn on_interaction(&self, kind: &str, path: &[usize]) {
        match kind {
            "closeMenu" => {
                self.state.borrow_mut().closing = true;
                let fade = self.general.borrow().fade_out_duration as u32;
                unsafe {
                    SetTimer(Some(self.host.get()), TIMER_MENU_HIDE, fade.max(1), None);
                }
            }
            "activateMenu" => {
                if path.is_empty() {
                    self.request_close(true);
                } else {
                    self.post_event(
                        Page::Menu,
                        "menu-window.trigger-interaction",
                        vec![json!("closeSubmenu")],
                    );
                }
            }
            "selectButton" => self.on_selection(path),
            _ => {}
        }
    }

    fn on_selection(&self, path: &[usize]) {
        let (action, opening) = {
            let state = self.state.borrow();
            let Some(menu) = state.last_menu.as_ref() else {
                return;
            };
            let Some(item) = menu.root.at_path(path) else {
                log!("no item at path {path:?}");
                return;
            };
            if !matches!(item, MenuItem::Button { .. }) {
                return;
            }
            (item.action().cloned(), state.opening_pointer)
        };

        if self
            .general
            .borrow()
            .return_pointer_to_menu_opening_position
            && let Some(point) = opening
        {
            unsafe {
                let _ = SetCursorPos(point.x, point.y);
            }
        }

        match action {
            None => self.request_close(true),
            Some(Action::OpenMenu { menu }) => {
                if !menu.trim().is_empty() {
                    self.show_menu(ShowRequest::Name(menu));
                } else {
                    self.request_close(true);
                }
            }
            Some(Action::SimulateHotkey { hotkey }) => match actions::parse_hotkey(&hotkey) {
                Ok(codes) => {
                    self.state.borrow_mut().pending_hotkey = Some(codes);
                    self.request_close(true);
                }
                Err(e) => {
                    self.notify_failure(&Failure::Other(e));
                    self.request_close(true);
                }
            },
            Some(Action::ExecuteMacro { r#macro }) => match actions::parse_macro(&r#macro) {
                Ok(keys) => {
                    self.state.borrow_mut().pending_macro = Some(keys);
                    self.request_close(true);
                }
                Err(e) => {
                    self.notify_failure(&Failure::Other(e));
                    self.request_close(true);
                }
            },
            Some(Action::SetClipboard { text }) => {
                let placeholders = self.state.borrow().placeholders.clone();
                spawn_worker(move || {
                    if let Err(failure) = actions::set_clipboard(&text, &placeholders) {
                        log!("clipboard action failed: {failure:?}");
                        post_worker_event(WorkerEvent::ActionFailed(failure));
                    }
                });
                self.request_close(true);
            }
            Some(Action::FocusWindow {
                app_name,
                window_name,
            }) => {
                // Run on the UI thread while KandoAir owns foreground rights so
                // SetForegroundWindow succeeds instead of merely flashing the taskbar.
                if let Err(failure) =
                    actions::focus_window(app_name.as_deref(), window_name.as_deref())
                {
                    log!("focus-window action failed: {failure:?}");
                    self.notify_failure(&failure);
                    self.request_close(true);
                } else {
                    self.request_close(false);
                }
            }
            Some(action) => {
                unsafe {
                    let _ = AllowSetForegroundWindow(ASFW_ANY);
                }
                let placeholders = self.state.borrow().placeholders.clone();
                spawn_worker(move || {
                    let result = match &action {
                        Action::LaunchApp { target } => actions::launch_app(target),
                        Action::OpenFile { path } => actions::open_file(path),
                        Action::OpenUri { uri } => actions::open_uri(uri, &placeholders),
                        Action::ExecuteCommand { command, detached } => {
                            actions::execute_command(command, *detached, &placeholders)
                        }
                        Action::SimulateHotkey { .. }
                        | Action::ExecuteMacro { .. }
                        | Action::SetClipboard { .. }
                        | Action::FocusWindow { .. }
                        | Action::OpenMenu { .. } => Ok(()),
                    };
                    if let Err(failure) = result {
                        log!("action failed: {failure:?}");
                        post_worker_event(WorkerEvent::ActionFailed(failure));
                    }
                });
                // The launched/focused window should keep the focus.
                self.request_close(false);
            }
        }
    }

    /// Hides the menu window, restores the focus and runs a pending hotkey/macro simulation.
    fn hide_menu_now(&self) {
        unsafe {
            let _ = KillTimer(Some(self.host.get()), TIMER_MENU_HIDE);
        }
        let Some(menu) = self.menu() else { return };
        if !menu.visible.get() {
            return;
        }
        let (restore, hotkey, macro_keys, inhibition) = {
            let mut state = self.state.borrow_mut();
            state.closing = false;
            (
                state.restore_focus
                    || state.pending_hotkey.is_some()
                    || state.pending_macro.is_some(),
                state.pending_hotkey.take(),
                state.pending_macro.take(),
                std::mem::take(&mut state.inhibition),
            )
        };
        menu.visible.set(false);
        menu.hide(restore);

        if inhibition > 0 {
            // Failures are reported by bind_shortcuts() below.
            let _ = self.with_hotkeys(|h| h.release(inhibition));
        }
        // Menu settings may have changed while the menu was open.
        self.bind_shortcuts();

        if let Some(codes) = hotkey {
            let inhibition = self.with_hotkeys(|h| h.inhibit("*")).unwrap_or(0);
            let target = menu.previous_foreground.get().0 as isize;
            spawn_worker(move || {
                let target = HWND(target as *mut _);
                let failure = actions::simulate_hotkey(target, &codes).err();
                post_worker_event(WorkerEvent::HotkeyDone {
                    inhibition,
                    failure,
                });
            });
        } else if let Some(keys) = macro_keys {
            let inhibition = self.with_hotkeys(|h| h.inhibit("*")).unwrap_or(0);
            let target = menu.previous_foreground.get().0 as isize;
            spawn_worker(move || {
                let target = HWND(target as *mut _);
                let failure = actions::simulate_macro(target, &keys).err();
                post_worker_event(WorkerEvent::HotkeyDone {
                    inhibition,
                    failure,
                });
            });
        }

        unsafe {
            SetTimer(
                Some(self.host.get()),
                TIMER_MENU_IDLE,
                MENU_IDLE_DELAY_MS,
                None,
            );
        }
    }

    fn move_pointer(&self, distance: &Value) {
        let dx = distance.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let dy = distance.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        let scale = self
            .state
            .borrow()
            .placement
            .map(|p| p.scale)
            .unwrap_or(1.0)
            * self.general.borrow().zoom_factor;
        let mut point = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut point);
            let _ = SetCursorPos(
                point.x + (dx * scale).round() as i32,
                point.y + (dy * scale).round() as i32,
            );
        }
    }

    fn on_menu_deactivated(&self) {
        let Some(menu) = self.menu() else { return };
        let closing = self.state.borrow().closing;
        if menu.visible.get() && !closing && self.general.borrow().hide_on_focus_out {
            self.request_close(false);
        }
    }

    // ---------------------------------------------------------------------------------- //
    // Settings window
    // ---------------------------------------------------------------------------------- //

    pub fn show_settings(&self) {
        // If a menu is still fading out, hiding it must not give the focus back to the
        // previous window: the settings window should stay in front.
        self.state.borrow_mut().restore_focus = false;
        settings_window::show(self);
    }

    // ---------------------------------------------------------------------------------- //
    // Tray, command line and system events
    // ---------------------------------------------------------------------------------- //

    fn show_tray_menu(&self) {
        let mut entries = Vec::new();
        {
            let i18n = self.i18n.borrow();
            let menus = self.menus.borrow();
            for (index, menu) in menus.menus.iter().enumerate() {
                let shortcut = if menu.shortcut.is_empty() {
                    i18n.t(s::NOT_BOUND)
                } else {
                    hotkey::display(&menu.shortcut)
                };
                // Escape '&' so Win32 AppendMenuW does not turn it into an accelerator underline.
                let safe_name = menu.root.name().replace('&', "&&");
                entries.push(MenuEntry::Item {
                    id: TRAY_MENU_BASE + index as u32,
                    label: format!("{safe_name} ({shortcut})"),
                    checked: false,
                });
            }
            if !menus.menus.is_empty() {
                entries.push(MenuEntry::Separator);
            }
            entries.push(MenuEntry::Item {
                id: TRAY_SETTINGS,
                label: i18n.t(s::SHOW_SETTINGS),
                checked: false,
            });
            let paused = self.pause_inhibition.get() > 0;
            entries.push(MenuEntry::Item {
                id: TRAY_PAUSE,
                label: i18n.t(if paused {
                    s::UN_INHIBIT_SHORTCUTS
                } else {
                    s::INHIBIT_SHORTCUTS
                }),
                checked: false,
            });
            entries.push(MenuEntry::Separator);
            entries.push(MenuEntry::Item {
                id: TRAY_QUIT,
                label: i18n.t(s::QUIT),
                checked: false,
            });
        }

        // No borrow of the tray may be held here: the menu runs a modal message loop which
        // dispatches timers and notifications that modify the tray.
        let command = if self.tray.borrow().is_some() {
            tray::show_menu(self.host.get(), &entries)
        } else {
            0
        };
        match command {
            0 => {}
            TRAY_SETTINGS => self.show_settings(),
            TRAY_PAUSE => {
                let current = self.pause_inhibition.get();
                if current > 0 {
                    let failures = self
                        .with_hotkeys(|h| h.release(current))
                        .unwrap_or_default();
                    self.pause_inhibition.set(0);
                    self.report_shortcut_failures(failures);
                } else {
                    let id = self.with_hotkeys(|h| h.inhibit("*")).unwrap_or(0);
                    self.pause_inhibition.set(id);
                }
            }
            TRAY_QUIT => self.quit(),
            id if id >= TRAY_MENU_BASE => {
                self.show_menu(ShowRequest::Index((id - TRAY_MENU_BASE) as usize))
            }
            _ => {}
        }
    }

    pub fn handle_options(&self, options: &Options, from_second_instance: bool) {
        if let Some(name) = &options.menu {
            self.show_menu(ShowRequest::Name(name.clone()));
        } else if options.settings || from_second_instance {
            self.show_settings();
        }
    }

    fn on_worker_events(&self) {
        let events = std::mem::take(&mut *WORKER_EVENTS.lock().unwrap_or_else(|e| e.into_inner()));
        for event in events {
            match event {
                WorkerEvent::ActionFailed(failure) => {
                    self.notify_failure(&failure);
                    if let Some(menu) = self.menu() {
                        let previous = menu.previous_foreground.get();
                        if !menu.visible.get() && unsafe { IsWindow(Some(previous)) }.as_bool() {
                            unsafe {
                                let _ = SetForegroundWindow(previous);
                            }
                        }
                    }
                }
                WorkerEvent::HotkeyDone {
                    inhibition,
                    failure,
                } => {
                    let failures = self
                        .with_hotkeys(|h| h.release(inhibition))
                        .unwrap_or_default();
                    self.report_shortcut_failures(failures);
                    if let Some(failure) = failure {
                        self.notify_failure(&failure);
                    }
                }
                WorkerEvent::Reply { page, id, result } => self.reply(page, id, result),
                WorkerEvent::Icon { id, bytes } => crate::icons::complete(id, bytes),
            }
        }
    }

    fn on_dark_mode_check(&self) {
        let dark = shell::dark_mode();
        if dark != self.dark_mode.replace(dark) {
            log!("dark mode changed: {dark}");
            self.post_event(Page::Menu, "common.dark-mode-changed", vec![json!(dark)]);
            self.post_event(
                Page::Settings,
                "common.dark-mode-changed",
                vec![json!(dark)],
            );
            settings_window::on_dark_mode_changed(self, dark);
        }
    }

    pub fn quit(&self) {
        log!("quitting");
        self.flush_settings();
        if let Some(window) = self.take_settings_window() {
            window.save_state(self);
            window.destroy();
        }
        if let Some(menu) = self.take_menu_window() {
            menu.destroy();
        }
        self.with_hotkeys(|h| h.clear());
        self.with_tray(|t| t.remove());
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn on_timer(&self, id: usize) {
        unsafe {
            let _ = KillTimer(Some(self.host.get()), id);
        }
        match id {
            TIMER_MENU_HIDE => self.hide_menu_now(),
            TIMER_MENU_IDLE => self.suspend_menu_if_idle(),
            TIMER_SAVE_GENERAL => self.save_general_now(),
            TIMER_SAVE_MENUS => self.save_menus_now(),
            TIMER_TRAY_CLEANUP => {
                self.with_tray(|t| t.remove_temporary());
            }
            TIMER_RETRY_MENU => self.ensure_menu_window(),
            _ => {}
        }
    }

    /// Runs `task` from the message loop (outside of WebView2 event handlers). Used for
    /// modal dialogs.
    pub fn defer(&self, task: impl FnOnce(&App) + 'static) {
        self.deferred.borrow_mut().push(Box::new(task));
        unsafe {
            let _ = PostMessageW(Some(self.host.get()), WM_APP_DEFER, WPARAM(0), LPARAM(0));
        }
    }

    fn run_deferred(&self) {
        let tasks = std::mem::take(&mut *self.deferred.borrow_mut());
        for task in tasks {
            task(self);
        }
    }

    /// Serves generated app icons to a web view using a bounded 3-thread worker pool.
    pub fn attach_icons(&self, webview: &ICoreWebView2, environment: ICoreWebView2Environment) {
        let result = crate::icons::register(
            webview,
            environment,
            self.paths.icon_cache(),
            enqueue_icon_job,
        );
        if let Err(e) = result {
            log!("failed to register the icon handler: {e}");
        }
    }

    /// The index of the most recently shown menu (used to preselect it in the settings).
    pub fn current_menu_index(&self) -> usize {
        let state = self.state.borrow();
        let Some(last) = state.last_menu.as_ref() else {
            return 0;
        };
        self.menus
            .borrow()
            .menus
            .iter()
            .position(|m| m.root.name() == last.root.name())
            .unwrap_or(0)
    }

    pub fn open_dev_tools(&self, page: Page) {
        if page == Page::Menu {
            self.resume_menu();
        }
        if let Some(webview) = self.page_webview(page) {
            unsafe {
                let _ = webview.OpenDevToolsWindow();
            }
        }
    }

    /// Called after the settings window was destroyed.
    pub fn on_settings_closed(&self) {
        log!("settings window closed");
        unsafe {
            SetTimer(
                Some(self.host.get()),
                TIMER_MENU_IDLE,
                MENU_IDLE_DELAY_MS,
                None,
            );
        }
    }

    pub fn mark_show_started(&self) {
        self.show_started.set(Some(Instant::now()));
    }
}

extern "system" fn host_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(app) = app() else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match msg {
        WM_HOTKEY => {
            let shortcut = app
                .hotkeys
                .borrow()
                .as_ref()
                .and_then(|h| h.shortcut_for_id(wparam.0 as i32));
            if let Some(shortcut) = shortcut {
                log!("hotkey triggered: {shortcut}");
                app.mark_show_started();
                app.show_menu(ShowRequest::Trigger(shortcut));
            }
            LRESULT(0)
        }
        tray::WM_TRAY => {
            match tray::event_of(lparam) {
                WM_CONTEXTMENU | WM_RBUTTONUP => app.show_tray_menu(),
                // NIN_SELECT and NIN_KEYSELECT.
                0x0400 | 0x0401 => app.show_settings(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            app.on_timer(wparam.0);
            LRESULT(0)
        }
        WM_APP_WORKER => {
            app.on_worker_events();
            LRESULT(0)
        }
        WM_APP_DEFER => {
            app.run_deferred();
            LRESULT(0)
        }
        WM_COPYDATA => {
            let data = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
            if data.dwData == COPYDATA_ARGS && !data.lpData.is_null() {
                let bytes = unsafe {
                    std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize)
                };
                let args: Vec<String> = serde_json::from_slice(bytes).unwrap_or_default();
                log!("second instance: {args:?}");
                let options = Options::parse(&args);
                app.handle_options(&options, true);
            }
            LRESULT(1)
        }
        WM_SETTINGCHANGE => {
            app.on_dark_mode_check();
            LRESULT(0)
        }
        WM_QUERYENDSESSION => LRESULT(1),
        // Lets scripts and installers quit gracefully. `taskkill` without /f does not send
        // it, because the host window is hidden.
        WM_CLOSE => {
            app.quit();
            LRESULT(0)
        }
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                app.flush_settings();
            }
            LRESULT(0)
        }
        _ if msg == app.taskbar_created && msg != 0 => {
            app.with_tray(|t| t.readd());
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

extern "system" fn menu_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(app) = app() else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match msg {
        WM_SIZE => {
            if let Some(menu) = app.menu() {
                menu.fit();
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE {
                app.on_menu_deactivated();
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => {
            if app.general.borrow().keep_input_focus {
                LRESULT(MA_NOACTIVATE as isize)
            } else {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
        WM_CLOSE => {
            app.request_close(true);
            LRESULT(0)
        }
        // We position the window ourselves; ignore the suggested rectangle.
        WM_DPICHANGED => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Evaluates a menu's trigger conditions against the active window & pointer.
/// Returns `None` if any configured condition fails, or `Some(score)` with the count of
/// matched conditions (higher score = more specific menu wins).
fn score_menu_conditions(menu: &Menu, context: &Placeholders) -> Option<usize> {
    let Some(conditions) = menu.conditions.as_ref() else {
        return Some(0);
    };
    let mut score = 0usize;
    if let Some(app_pattern) = conditions.app_name.as_deref().filter(|s| !s.trim().is_empty()) {
        if !shell::matches_condition(app_pattern, &context.app_name) {
            return None;
        }
        score += 1;
    }
    if let Some(win_pattern) = conditions
        .window_name
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        if !shell::matches_condition(win_pattern, &context.window_name) {
            return None;
        }
        score += 1;
    }
    if let Some(area) = conditions.screen_area.as_ref().filter(|a| !a.is_empty()) {
        if let Some(x_min) = area.x_min
            && context.pointer_x < x_min
        {
            return None;
        }
        if let Some(x_max) = area.x_max
            && context.pointer_x > x_max
        {
            return None;
        }
        if let Some(y_min) = area.y_min
            && context.pointer_y < y_min
        {
            return None;
        }
        if let Some(y_max) = area.y_max
            && context.pointer_y > y_max
        {
            return None;
        }
        score += 1;
    }
    Some(score)
}

/// Suspends a web view (used by the menu window while hidden).
pub fn try_suspend(webview: &ICoreWebView2) -> bool {
    match webview.cast::<ICoreWebView2_3>() {
        Ok(webview3) => {
            let handler = TrySuspendCompletedHandler::create(Box::new(|result, suspended| {
                log!("menu web view suspended: {} ({result:?})", suspended);
                Ok(())
            }));
            unsafe { webview3.TrySuspend(&handler).is_ok() }
        }
        Err(_) => false,
    }
}
