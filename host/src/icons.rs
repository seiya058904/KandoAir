// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Serves generated and local images to the pages:
//!
//! - `https://sys.kandoair.example/app-icon/<app name>`: icons of installed apps. Cached
//!   icons are answered immediately; others are rendered on a worker thread while the
//!   request is deferred.
//! - `https://sys.kandoair.example/file?path=<path>`: local image files, which Kando
//!   allows as `file://` icons (pages on https cannot load `file://` URLs).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{WebResourceRequestedEventHandler, take_pwstr};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{HSTRING, PWSTR};

use crate::apps;
use crate::log;
use crate::webview::SYS_HOST;

struct Pending {
    args: ICoreWebView2WebResourceRequestedEventArgs,
    deferral: ICoreWebView2Deferral,
    environment: ICoreWebView2Environment,
}

thread_local! {
    static PENDING: RefCell<HashMap<u64, Pending>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

fn respond(
    environment: &ICoreWebView2Environment,
    args: &ICoreWebView2WebResourceRequestedEventArgs,
    bytes: Option<&[u8]>,
) {
    respond_typed(environment, args, bytes, "image/png");
}

fn respond_typed(
    environment: &ICoreWebView2Environment,
    args: &ICoreWebView2WebResourceRequestedEventArgs,
    bytes: Option<&[u8]>,
    content_type: &str,
) {
    unsafe {
        let result = match bytes {
            Some(bytes) => {
                let stream = SHCreateMemStream(Some(bytes));
                environment.CreateWebResourceResponse(
                    stream.as_ref(),
                    200,
                    &HSTRING::from("OK"),
                    &HSTRING::from(format!(
                        "Content-Type: {content_type}\r\nCache-Control: max-age=86400"
                    )),
                )
            }
            None => environment.CreateWebResourceResponse(
                None,
                404,
                &HSTRING::from("Not Found"),
                &HSTRING::new(),
            ),
        };
        match result {
            Ok(response) => {
                let _ = args.SetResponse(&response);
            }
            Err(e) => log!("CreateWebResourceResponse failed: {e}"),
        }
    }
}

/// The content type of a local image file, or `None` if it is not an image.
fn image_type(path: &str) -> Option<&'static str> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return None,
    })
}

/// The path of a local image URL, if the URL is a local image URL.
pub fn file_path_of(url: &str) -> Option<String> {
    let prefix = format!("https://{SYS_HOST}/file?path=");
    let rest = url.strip_prefix(&prefix)?;
    let rest = rest.split(['&', '#']).next().unwrap_or(rest);
    let path = apps::decode_url_segment(&rest.replace('+', " "));
    (!path.is_empty()).then_some(path)
}

/// The app name of an icon URL, if the URL is an app icon URL.
pub fn app_name_of(url: &str) -> Option<String> {
    let prefix = format!("https://{SYS_HOST}/app-icon/");
    let rest = url.strip_prefix(&prefix)?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let name = apps::decode_url_segment(rest);
    (!name.is_empty()).then_some(name)
}

/// Answers app icon requests of the given web view. `spawn` runs the rendering on a
/// worker thread and must eventually call [`complete`] on the UI thread.
pub fn register(
    webview: &ICoreWebView2,
    environment: ICoreWebView2Environment,
    cache_dir: PathBuf,
    spawn: impl Fn(u64, String, PathBuf) + 'static,
) -> windows::core::Result<()> {
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        unsafe {
            let request = args.Request()?;
            let mut uri = PWSTR::null();
            request.Uri(&mut uri)?;
            let uri = take_pwstr(uri);
            if let Some(path) = file_path_of(&uri) {
                let content_type = image_type(&path);
                let bytes = content_type.and_then(|_| std::fs::read(&path).ok());
                respond_typed(
                    &environment,
                    &args,
                    bytes.as_deref(),
                    content_type.unwrap_or("image/png"),
                );
                return Ok(());
            }
            let Some(name) = app_name_of(&uri) else {
                respond(&environment, &args, None);
                return Ok(());
            };
            if let Ok(bytes) = std::fs::read(apps::cache_file(&cache_dir, &name)) {
                respond(&environment, &args, Some(&bytes));
                return Ok(());
            }
            let deferral = args.GetDeferral()?;
            let id = NEXT_ID.with(|n| {
                let id = n.get();
                n.set(id + 1);
                id
            });
            PENDING.with(|p| {
                p.borrow_mut().insert(
                    id,
                    Pending {
                        args: args.clone(),
                        deferral,
                        environment: environment.clone(),
                    },
                )
            });
            spawn(id, name, cache_dir.clone());
        }
        Ok(())
    }));

    unsafe {
        webview.AddWebResourceRequestedFilter(
            &HSTRING::from(format!("https://{SYS_HOST}/*")),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        )?;
        let mut token = 0i64;
        webview.add_WebResourceRequested(&handler, &mut token)
    }
}

/// Completes a deferred icon request.
pub fn complete(id: u64, bytes: Option<Vec<u8>>) {
    let pending = PENDING.with(|p| p.borrow_mut().remove(&id));
    if let Some(pending) = pending {
        respond(&pending.environment, &pending.args, bytes.as_deref());
        unsafe {
            let _ = pending.deferral.Complete();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_urls_are_parsed() {
        assert_eq!(
            app_name_of("https://sys.kandoair.example/app-icon/Visual%20Studio%20Code?reload=2"),
            Some("Visual Studio Code".into())
        );
        assert_eq!(app_name_of("https://sys.kandoair.example/other/x"), None);
        assert_eq!(app_name_of("https://sys.kandoair.example/app-icon/"), None);
        assert_eq!(
            file_path_of("https://sys.kandoair.example/file?path=C%3A%2FIcons%2Fmy%20icon.png"),
            Some("C:/Icons/my icon.png".into())
        );
        assert_eq!(image_type("C:/a/b.SVG"), Some("image/svg+xml"));
        assert_eq!(image_type("C:/a/b.exe"), None);
    }
}
