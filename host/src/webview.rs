// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Thin helpers around the WebView2 COM API: creating the shared environment, creating a
//! controller for a host window, applying the common settings, and exchanging JSON
//! messages with the page.

use std::path::Path;

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{HSTRING, Interface, PCWSTR, PWSTR};

use crate::log;

/// The virtual host which serves the bundled frontend.
pub const APP_HOST: &str = "app.kandoair.example";
/// The virtual host which serves the config directory (user icon themes).
pub const USER_HOST: &str = "user.kandoair.example";
/// The virtual host whose requests are answered by the host (generated app icons).
pub const SYS_HOST: &str = "sys.kandoair.example";

/// Creates the WebView2 environment. The `done` callback is invoked on the UI thread.
pub fn create_environment(
    user_data_folder: &Path,
    browser_args: &str,
    done: impl FnOnce(windows::core::Result<ICoreWebView2Environment>) + 'static,
) -> windows::core::Result<()> {
    let options = CoreWebView2EnvironmentOptions::default();
    unsafe {
        options.set_additional_browser_arguments(browser_args.to_string());
    }
    let options: ICoreWebView2EnvironmentOptions = options.into();

    let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |result, environment| {
            done(result.and_then(|_| environment.ok_or_else(|| E_POINTER.into())));
            Ok(())
        },
    ));

    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            &HSTRING::from(user_data_folder.as_os_str()),
            &options,
            &handler,
        )
    }
}

/// Creates a controller hosted in the given window. The `done` callback is invoked on the
/// UI thread.
pub fn create_controller(
    environment: &ICoreWebView2Environment,
    hwnd: HWND,
    done: impl FnOnce(windows::core::Result<ICoreWebView2Controller>) + 'static,
) -> windows::core::Result<()> {
    let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
        move |result, controller| {
            done(result.and_then(|_| controller.ok_or_else(|| E_POINTER.into())));
            Ok(())
        },
    ));

    unsafe { environment.CreateCoreWebView2Controller(hwnd, &handler) }
}

/// Options for [`configure`].
pub struct ConfigureOptions<'a> {
    /// Folder which is served at `https://app.kandoair.example/`.
    pub app_folder: &'a Path,
    /// Folder which is served at `https://user.kandoair.example/`.
    pub user_folder: &'a Path,
    /// Whether the page background should be transparent.
    pub transparent: bool,
    /// Whether `app-region: drag` areas should move the window.
    pub non_client_regions: bool,
}

/// Applies the settings which are shared by all KandoAir web views. Browser features
/// which make no sense for our local pages are disabled.
pub fn configure(
    controller: &ICoreWebView2Controller,
    options: &ConfigureOptions,
) -> windows::core::Result<ICoreWebView2> {
    unsafe {
        let webview = controller.CoreWebView2()?;
        let settings = webview.Settings()?;
        settings.SetAreDefaultContextMenusEnabled(false)?;
        settings.SetAreDefaultScriptDialogsEnabled(true)?;
        settings.SetIsStatusBarEnabled(false)?;
        settings.SetIsZoomControlEnabled(false)?;
        settings.SetIsBuiltInErrorPageEnabled(false)?;
        settings.SetAreDevToolsEnabled(true)?;

        if let Ok(s) = settings.cast::<ICoreWebView2Settings3>() {
            s.SetAreBrowserAcceleratorKeysEnabled(false)?;
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings4>() {
            s.SetIsGeneralAutofillEnabled(false)?;
            s.SetIsPasswordAutosaveEnabled(false)?;
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings5>() {
            s.SetIsPinchZoomEnabled(false)?;
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings6>() {
            s.SetIsSwipeNavigationEnabled(false)?;
        }
        if options.non_client_regions
            && let Ok(s) = settings.cast::<ICoreWebView2Settings9>()
        {
            s.SetIsNonClientRegionSupportEnabled(true)?;
        }

        if options.transparent {
            let controller2 = controller.cast::<ICoreWebView2Controller2>()?;
            controller2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 0,
                R: 0,
                G: 0,
                B: 0,
            })?;
        }

        let webview3 = webview.cast::<ICoreWebView2_3>()?;
        webview3.SetVirtualHostNameToFolderMapping(
            &HSTRING::from(APP_HOST),
            &HSTRING::from(options.app_folder.as_os_str()),
            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
        )?;
        webview3.SetVirtualHostNameToFolderMapping(
            &HSTRING::from(USER_HOST),
            &HSTRING::from(options.user_folder.as_os_str()),
            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
        )?;

        Ok(webview)
    }
}

/// Resizes the controller to fill the client area of its host window.
pub fn fit_to_window(controller: &ICoreWebView2Controller, hwnd: HWND) {
    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let _ = controller.SetBounds(rect);
        }
    }
}

/// Registers a handler for JSON messages sent with `window.chrome.webview.postMessage()`
/// or `postMessageWithAdditionalObjects()`. The handler receives the raw JSON text, the
/// URL of the sending page and the paths of any files passed along.
pub fn on_message(
    webview: &ICoreWebView2,
    mut handler: impl FnMut(String, String, Vec<String>) + 'static,
) -> windows::core::Result<()> {
    let callback = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let mut json = PWSTR::null();
            args.WebMessageAsJson(&mut json)?;
            let json = take_pwstr(json);

            let mut source = PWSTR::null();
            args.Source(&mut source)?;
            let source = take_pwstr(source);

            let mut files = Vec::new();
            if let Ok(args2) = args.cast::<ICoreWebView2WebMessageReceivedEventArgs2>()
                && let Ok(objects) = args2.AdditionalObjects()
            {
                let mut count = 0u32;
                let _ = objects.Count(&mut count);
                for index in 0..count {
                    let Ok(object) = objects.GetValueAtIndex(index) else {
                        continue;
                    };
                    if let Ok(file) = object.cast::<ICoreWebView2File>() {
                        let mut path = PWSTR::null();
                        if file.Path(&mut path).is_ok() {
                            files.push(take_pwstr(path));
                        }
                    }
                }
            }

            handler(json, source, files);
        }
        Ok(())
    }));

    let mut token = 0i64;
    unsafe { webview.add_WebMessageReceived(&callback, &mut token) }
}

/// Keeps the web view on the bundled pages: other navigations are cancelled and handed
/// to `open_external` (which opens them in the default browser), as are new windows.
pub fn guard_navigation(
    webview: &ICoreWebView2,
    open_external: impl Fn(String) + Clone + 'static,
) -> windows::core::Result<()> {
    let external = open_external.clone();
    let navigation = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let mut uri = PWSTR::null();
            args.Uri(&mut uri)?;
            let uri = take_pwstr(uri);
            if !is_app_url(&uri) {
                args.SetCancel(true)?;
                if uri.starts_with("http://")
                    || uri.starts_with("https://")
                    || uri.starts_with("mailto:")
                {
                    external(uri);
                }
            }
        }
        Ok(())
    }));

    let new_window = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let mut uri = PWSTR::null();
            args.Uri(&mut uri)?;
            let uri = take_pwstr(uri);
            args.SetHandled(true)?;
            if uri.starts_with("http://")
                || uri.starts_with("https://")
                || uri.starts_with("mailto:")
            {
                open_external(uri);
            }
        }
        Ok(())
    }));

    let mut token = 0i64;
    unsafe {
        webview.add_NavigationStarting(&navigation, &mut token)?;
        webview.add_NewWindowRequested(&new_window, &mut token)
    }
}

/// Calls `handler` with a description whenever a WebView2 process fails.
pub fn on_process_failed(
    webview: &ICoreWebView2,
    mut handler: impl FnMut(COREWEBVIEW2_PROCESS_FAILED_KIND) + 'static,
) -> windows::core::Result<()> {
    let callback = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_UNKNOWN_PROCESS_EXITED;
            unsafe {
                let _ = args.ProcessFailedKind(&mut kind);
            }
            handler(kind);
        }
        Ok(())
    }));
    let mut token = 0i64;
    unsafe { webview.add_ProcessFailed(&callback, &mut token) }
}

/// Posts a JSON message to the page. Errors are logged and otherwise ignored.
pub fn post_json(webview: &ICoreWebView2, json: &str) {
    unsafe {
        if let Err(e) = webview.PostWebMessageAsJson(&HSTRING::from(json)) {
            log!("PostWebMessageAsJson failed: {e}");
        }
    }
}

/// Returns true if the given URL belongs to the bundled frontend.
pub fn is_app_url(url: &str) -> bool {
    url.starts_with(&format!("https://{APP_HOST}/")) || url == "about:blank"
}

/// Returns the URL of a page of the bundled frontend.
pub fn app_url(page: &str) -> String {
    format!("https://{APP_HOST}/{page}")
}
