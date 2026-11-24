// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Installed applications (the shell's AppsFolder, i.e. everything in the start menu
//! including Store apps) and their icons.
//!
//! Unlike Kando, which renders the icons of all installed apps at startup and keeps them
//! in memory, KandoAir enumerates apps only when the settings need them and renders
//! icons on demand. Rendered icons are cached as PNG files on disk.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_ItemNameDisplay};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, STATFLAG_NONAME, STATSTG,
    STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::*;
use windows::core::Interface;

use crate::log;

#[derive(Clone, Debug)]
pub struct AppInfo {
    pub name: String,
    /// The AppUserModelID. `shell:AppsFolder\<id>` launches the app.
    pub id: String,
}

impl AppInfo {
    pub fn launch_target(&self) -> String {
        format!("shell:AppsFolder\\{}", self.id)
    }
}

static APPS: Mutex<Option<Vec<AppInfo>>> = Mutex::new(None);

fn enumerate() -> Vec<AppInfo> {
    let mut apps = Vec::new();
    unsafe {
        let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        else {
            return apps;
        };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            return apps;
        };
        loop {
            let mut item = [None];
            let mut fetched = 0u32;
            if items.Next(&mut item, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = item[0].take() else { break };
            let Ok(item2) = item.cast::<IShellItem2>() else {
                continue;
            };
            let name = item2.GetString(&PKEY_ItemNameDisplay);
            let id = item2.GetString(&PKEY_AppUserModel_ID);
            if let (Ok(name), Ok(id)) = (name, id) {
                let app = AppInfo {
                    name: name.to_string().unwrap_or_default(),
                    id: id.to_string().unwrap_or_default(),
                };
                CoTaskMemFree(Some(name.0 as *const _));
                CoTaskMemFree(Some(id.0 as *const _));
                if !app.name.is_empty() && !app.id.is_empty() {
                    apps.push(app);
                }
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Returns the installed apps, enumerating them if necessary. Must be called on a thread
/// with COM initialized; enumeration takes a few hundred milliseconds.
pub fn installed() -> Vec<AppInfo> {
    let mut cache = APPS.lock().unwrap_or_else(|e| e.into_inner());
    if cache.is_none() {
        let started = std::time::Instant::now();
        let apps = enumerate();
        log!("enumerated {} apps in {:?}", apps.len(), started.elapsed());
        *cache = Some(apps);
    }
    cache.clone().unwrap_or_default()
}

/// Forgets the enumerated apps (called when the settings window closes).
pub fn forget() {
    *APPS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Finds an installed app by display name, AppUserModelID or launch target without
/// cloning the full app list.
pub fn find(key: &str) -> Option<AppInfo> {
    let mut cache = APPS.lock().unwrap_or_else(|e| e.into_inner());
    if cache.is_none() {
        let started = std::time::Instant::now();
        let apps = enumerate();
        log!("enumerated {} apps in {:?}", apps.len(), started.elapsed());
        *cache = Some(apps);
    }
    cache
        .as_ref()?
        .iter()
        .find(|a| a.name == key || a.id == key || a.launch_target() == key)
        .cloned()
}

/// FNV-1a, used for icon cache file names.
fn hash(text: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn cache_file(cache_dir: &Path, name: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.png", hash(name)))
}

/// Renders the icon of an app (by AppUserModelID) as PNG.
pub fn render_icon(app_id: &str, size: i32) -> Option<Vec<u8>> {
    unsafe {
        let path = windows::core::HSTRING::from(format!("shell:AppsFolder\\{app_id}"));
        let item: IShellItemImageFactory = SHCreateItemFromParsingName(&path, None).ok()?;
        let bitmap = item
            .GetImage(
                SIZE { cx: size, cy: size },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )
            .ok()?;
        let result = bitmap_to_png(bitmap);
        let _ = DeleteObject(bitmap.into());
        result
    }
}

unsafe fn bitmap_to_png(bitmap: HBITMAP) -> Option<Vec<u8>> {
    unsafe {
        let mut info = BITMAP::default();
        if GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut info as *mut _ as *mut _),
        ) == 0
        {
            return None;
        }
        let (width, height) = (info.bmWidth, info.bmHeight.abs());
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        if lines == 0 {
            return None;
        }
        // The shell returns premultiplied alpha; PNG stores straight alpha.
        for px in pixels.as_chunks_mut::<4>().0 {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        encode_png(width as u32, height as u32, &pixels)
    }
}

/// Encodes 32-bit BGRA pixels as PNG using WIC.
fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let stream = SHCreateMemStream(None)?;
        let encoder = factory
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
            .ok()?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache).ok()?;
        let mut frame = None;
        let mut properties = None;
        encoder.CreateNewFrame(&mut frame, &mut properties).ok()?;
        let frame = frame?;
        frame.Initialize(properties.as_ref()).ok()?;
        frame.SetSize(width, height).ok()?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format).ok()?;
        frame.WritePixels(height, width * 4, pixels).ok()?;
        frame.Commit().ok()?;
        encoder.Commit().ok()?;

        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME).ok()?;
        stream.Seek(0, STREAM_SEEK_SET, None).ok()?;
        let mut bytes = vec![0u8; stat.cbSize as usize];
        let mut read = 0u32;
        stream
            .Read(
                bytes.as_mut_ptr() as *mut _,
                bytes.len() as u32,
                Some(&mut read),
            )
            .ok()
            .ok()?;
        bytes.truncate(read as usize);
        Some(bytes)
    }
}

/// Returns the PNG icon of an app by display name, from the disk cache or by rendering it.
pub fn icon_for_name(cache_dir: &Path, name: &str) -> Option<Vec<u8>> {
    let file = cache_file(cache_dir, name);
    if let Ok(bytes) = std::fs::read(&file) {
        return Some(bytes);
    }
    let app = find(name)?;
    let bytes = render_icon(&app.id, 128)?;
    let _ = std::fs::create_dir_all(cache_dir);
    if let Err(e) = std::fs::write(&file, &bytes) {
        log!("failed to cache icon {}: {e}", file.display());
    }
    Some(bytes)
}

/// Percent-decodes a URL path segment.
pub fn decode_url_segment(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&segment[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_segments_are_decoded() {
        assert_eq!(
            decode_url_segment("Visual%20Studio%20Code"),
            "Visual Studio Code"
        );
        assert_eq!(decode_url_segment("%E8%AE%B0%E4%BA%8B%E6%9C%AC"), "记事本");
        assert_eq!(decode_url_segment("100%"), "100%");
    }

    #[test]
    fn cache_names_are_stable() {
        assert_eq!(
            cache_file(Path::new("c"), "Paint"),
            cache_file(Path::new("c"), "Paint")
        );
        assert_ne!(
            cache_file(Path::new("c"), "Paint"),
            cache_file(Path::new("c"), "Notepad")
        );
    }
}
