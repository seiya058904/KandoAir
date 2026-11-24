// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Menu theme and icon theme information for the renderers. Bundled menu themes live in
//! `resources/web/assets/menu-themes`; user themes live in `%APPDATA%\KandoAir\menu-themes`
//! and override bundled themes with the same ID. Icon themes are the bundled `kando` theme
//! plus any folders in the user's `icon-themes` directory.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::config::GeneralSettings;
use crate::log;
use crate::paths::Paths;
use crate::webview::{APP_HOST, USER_HOST};

pub const DEFAULT_THEME: &str = "default";

/// Validates a menu theme directory name (single safe path segment).
pub fn valid_theme_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains('\0')
        && !id.chars().any(char::is_control)
}

/// Converts a subset of JSON5 (comments, single-quoted strings, unquoted object keys,
/// trailing commas) to strict JSON so user `theme.json5` files can be read without an
/// extra crate.
fn json5_to_json(input: &str) -> String {
    let chars: Vec<char> = input.trim_start_matches('\u{feff}').chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < len {
        let c = chars[i];
        // Line comment.
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Block comment.
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(len);
            continue;
        }
        // Double-quoted string: copy verbatim.
        if c == '"' {
            out.push('"');
            i += 1;
            while i < len {
                let sc = chars[i];
                out.push(sc);
                i += 1;
                if sc == '\\' && i < len {
                    out.push(chars[i]);
                    i += 1;
                } else if sc == '"' {
                    break;
                }
            }
            continue;
        }
        // Single-quoted string: convert to double-quoted JSON string.
        if c == '\'' {
            out.push('"');
            i += 1;
            while i < len {
                let sc = chars[i];
                i += 1;
                if sc == '\\' && i < len {
                    let next = chars[i];
                    i += 1;
                    if next == '\'' {
                        out.push('\'');
                    } else {
                        out.push('\\');
                        out.push(next);
                    }
                } else if sc == '\'' {
                    break;
                } else if sc == '"' {
                    out.push_str("\\\"");
                } else {
                    out.push(sc);
                }
            }
            out.push('"');
            continue;
        }
        // Potential unquoted object key (identifier followed by optional whitespace and ':').
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < len
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '$'
                    || chars[i] == '-')
            {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            let mut look = i;
            while look < len && chars[look].is_whitespace() {
                look += 1;
            }
            if look < len && chars[look] == ':' {
                out.push('"');
                out.push_str(&ident);
                out.push('"');
            } else {
                out.push_str(&ident);
            }
            continue;
        }
        // JSON5 leading '+' on numbers (e.g. +5 or +.5) -> omit '+'.
        if c == '+'
            && i + 1 < len
            && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '.')
        {
            i += 1;
            continue;
        }
        // JSON5 leading decimal point (e.g. .5 -> 0.5).
        if c == '.'
            && i + 1 < len
            && chars[i + 1].is_ascii_digit()
            && !out.chars().last().is_some_and(|p| p.is_ascii_digit())
        {
            out.push('0');
            out.push('.');
            i += 1;
            continue;
        }
        // JSON5 trailing decimal point (e.g. 5. -> 5.0).
        if c == '.'
            && out.chars().last().is_some_and(|p| p.is_ascii_digit())
            && (i + 1 >= len || !chars[i + 1].is_ascii_digit())
        {
            out.push('.');
            out.push('0');
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }

    // Remove trailing commas before '}' or ']'.
    let cleaned: Vec<char> = out.chars().collect();
    let mut final_out = String::with_capacity(out.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut j = 0;
    while j < cleaned.len() {
        let c = cleaned[j];
        if in_string {
            final_out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            j += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            final_out.push(c);
            j += 1;
            continue;
        }
        if c == ',' {
            let mut k = j + 1;
            while k < cleaned.len() && cleaned[k].is_whitespace() {
                k += 1;
            }
            if k < cleaned.len() && (cleaned[k] == '}' || cleaned[k] == ']') {
                j += 1;
                continue;
            }
        }
        final_out.push(c);
        j += 1;
    }
    final_out
}

fn parse_theme_file(dir: &Path) -> Option<Map<String, Value>> {
    let json_path = dir.join("theme.json");
    if let Ok(text) = std::fs::read_to_string(&json_path) {
        if let Ok(Value::Object(object)) =
            serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
        {
            return Some(object);
        }
        log!("failed to parse {}", json_path.display());
    }

    let json5_path = dir.join("theme.json5");
    if let Ok(text) = std::fs::read_to_string(&json5_path) {
        let converted = json5_to_json(&text);
        if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&converted) {
            return Some(object);
        }
        log!("failed to parse {}", json5_path.display());
    }
    None
}

fn build_theme_description(id: &str, mut description: Map<String, Value>, base_url: &str) -> Value {
    let defaults = [
        ("name", json!(id)),
        ("author", json!("")),
        ("license", json!("")),
        ("themeVersion", json!("1.0")),
        ("engineVersion", json!(1)),
        ("maxMenuRadius", json!(160)),
        ("centerTextWrapWidth", json!(95)),
        ("drawChildrenBelow", json!(true)),
        ("drawCenterText", json!(true)),
        ("drawSelectionWedges", json!(false)),
        ("drawWedgeSeparators", json!(false)),
        ("colors", json!({})),
        (
            "layers",
            json!([{ "class": "icon-layer", "content": "icon" }]),
        ),
    ];
    for (key, value) in defaults {
        description.entry(key).or_insert(value);
    }
    description.insert("id".into(), json!(id));
    description.insert("directory".into(), json!(base_url));
    Value::Object(description)
}

fn load_themes(paths: &Paths) -> Vec<(String, Value)> {
    let mut themes: Vec<(String, Value)> = Vec::new();
    let bundled_url = format!("https://{APP_HOST}/assets/menu-themes");
    let user_url = format!("https://{USER_HOST}/menu-themes");

    let user_dir = paths.user_menu_themes();
    for id in subdirectories(&user_dir) {
        if !valid_theme_id(&id) {
            continue;
        }
        let dir = user_dir.join(&id);
        if !dir.join("theme.css").is_file() {
            continue;
        }
        if let Some(description) = parse_theme_file(&dir) {
            themes.push((id.clone(), build_theme_description(&id, description, &user_url)));
        }
    }

    let bundled_dir = paths.menu_themes();
    for id in subdirectories(&bundled_dir) {
        if !valid_theme_id(&id) || themes.iter().any(|(existing, _)| existing == &id) {
            continue;
        }
        let dir = bundled_dir.join(&id);
        if let Some(description) = parse_theme_file(&dir) {
            themes.push((
                id.clone(),
                build_theme_description(&id, description, &bundled_url),
            ));
        }
    }

    // Ensure "default" is always present even if assets were not found.
    if !themes.iter().any(|(id, _)| id == DEFAULT_THEME) {
        let mut fallback = Map::new();
        fallback.insert("name".into(), json!("Default"));
        themes.push((
            DEFAULT_THEME.into(),
            build_theme_description(DEFAULT_THEME, fallback, &bundled_url),
        ));
    }

    // Keep "default" first, followed by other themes sorted by display name.
    themes.sort_by(|(id_a, val_a), (id_b, val_b)| {
        let a_default = id_a == DEFAULT_THEME;
        let b_default = id_b == DEFAULT_THEME;
        b_default.cmp(&a_default).then_with(|| {
            let name_a = val_a
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id_a)
                .to_ascii_lowercase();
            let name_b = val_b
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id_b)
                .to_ascii_lowercase();
            name_a.cmp(&name_b)
        })
    });

    themes
}

/// All available menu themes (bundled plus user themes in `%APPDATA%\KandoAir\menu-themes`).
pub fn all_menu_themes(paths: &Paths) -> Value {
    Value::Array(
        load_themes(paths)
            .into_iter()
            .map(|(_, value)| value)
            .collect(),
    )
}

/// The theme ID configured for the current color scheme.
pub fn active_theme_id(settings: &GeneralSettings, dark_mode: bool) -> &str {
    let id = if settings.enable_dark_mode_for_menu_themes && dark_mode {
        settings.dark_menu_theme.as_str()
    } else {
        settings.menu_theme.as_str()
    };
    if valid_theme_id(id) {
        id
    } else {
        DEFAULT_THEME
    }
}

/// Loads a single menu theme by ID without scanning all theme directories.
fn load_menu_theme_by_id(paths: &Paths, id: &str) -> Option<Value> {
    if !valid_theme_id(id) {
        return None;
    }
    let user_dir = paths.user_menu_themes().join(id);
    if user_dir.join("theme.css").is_file()
        && let Some(description) = parse_theme_file(&user_dir)
    {
        let user_url = format!("https://{USER_HOST}/menu-themes");
        return Some(build_theme_description(id, description, &user_url));
    }
    let bundled_dir = paths.menu_themes().join(id);
    if let Some(description) = parse_theme_file(&bundled_dir) {
        let bundled_url = format!("https://{APP_HOST}/assets/menu-themes");
        return Some(build_theme_description(id, description, &bundled_url));
    }
    None
}

/// The description of the currently active menu theme (`MenuThemeDescription`).
pub fn current_menu_theme(paths: &Paths, settings: &GeneralSettings, dark_mode: bool) -> Value {
    let wanted = active_theme_id(settings, dark_mode);
    load_menu_theme_by_id(paths, wanted)
        .or_else(|| load_menu_theme_by_id(paths, DEFAULT_THEME))
        .unwrap_or_else(|| {
            let bundled_url = format!("https://{APP_HOST}/assets/menu-themes");
            let mut fallback = Map::new();
            fallback.insert("name".into(), json!("Default"));
            build_theme_description(DEFAULT_THEME, fallback, &bundled_url)
        })
}

/// The accent color overrides for the active theme and current system color scheme.
pub fn current_colors(paths: &Paths, settings: &GeneralSettings, dark_mode: bool) -> Value {
    let wanted = active_theme_id(settings, dark_mode);
    let resolved_id = if load_menu_theme_by_id(paths, wanted).is_some() {
        wanted
    } else {
        DEFAULT_THEME
    };
    let overrides = if settings.enable_dark_mode_for_menu_themes && dark_mode {
        &settings.dark_menu_theme_colors
    } else {
        &settings.menu_theme_colors
    };
    overrides
        .get(resolved_id)
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn build_sound_theme_description(
    id: &str,
    mut description: Map<String, Value>,
    base_url: &str,
) -> Value {
    let defaults = [
        ("name", json!(id)),
        ("author", json!("")),
        ("license", json!("")),
        ("themeVersion", json!("1.0")),
        ("engineVersion", json!(2)),
        ("sounds", json!({})),
    ];
    for (key, value) in defaults {
        description.entry(key).or_insert(value);
    }
    description.insert("id".into(), json!(id));
    description.insert("directory".into(), json!(base_url));
    Value::Object(description)
}

fn load_sound_theme_by_id(paths: &Paths, id: &str) -> Option<Value> {
    if id == "none" || !valid_theme_id(id) {
        return None;
    }
    let user_dir = paths.user_sound_themes().join(id);
    if let Some(description) = parse_theme_file(&user_dir) {
        let user_url = format!("https://{USER_HOST}/sound-themes");
        return Some(build_sound_theme_description(id, description, &user_url));
    }
    let bundled_dir = paths.sound_themes().join(id);
    if let Some(description) = parse_theme_file(&bundled_dir) {
        let bundled_url = format!("https://{APP_HOST}/assets/sound-themes");
        return Some(build_sound_theme_description(id, description, &bundled_url));
    }
    None
}

/// All available sound themes (bundled plus user sound themes in `%APPDATA%\KandoAir\sound-themes`).
pub fn all_sound_themes(paths: &Paths) -> Value {
    let mut themes: Vec<(String, Value)> = Vec::new();
    let user_url = format!("https://{USER_HOST}/sound-themes");
    let bundled_url = format!("https://{APP_HOST}/assets/sound-themes");

    let user_dir = paths.user_sound_themes();
    for id in subdirectories(&user_dir) {
        if !valid_theme_id(&id) {
            continue;
        }
        if let Some(description) = parse_theme_file(&user_dir.join(&id)) {
            themes.push((
                id.clone(),
                build_sound_theme_description(&id, description, &user_url),
            ));
        }
    }

    let bundled_dir = paths.sound_themes();
    for id in subdirectories(&bundled_dir) {
        if !valid_theme_id(&id) || themes.iter().any(|(existing, _)| existing == &id) {
            continue;
        }
        if let Some(description) = parse_theme_file(&bundled_dir.join(&id)) {
            themes.push((
                id.clone(),
                build_sound_theme_description(&id, description, &bundled_url),
            ));
        }
    }

    themes.sort_by(|(id_a, val_a), (id_b, val_b)| {
        let name_a = val_a
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id_a)
            .to_ascii_lowercase();
        let name_b = val_b
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id_b)
            .to_ascii_lowercase();
        name_a.cmp(&name_b)
    });

    Value::Array(themes.into_iter().map(|(_, v)| v).collect())
}

/// The active sound theme description, or a disabled (`id: "none"`) placeholder.
pub fn current_sound_theme(paths: &Paths, settings: &GeneralSettings) -> Value {
    load_sound_theme_by_id(paths, &settings.sound_theme).unwrap_or_else(|| {
        json!({
            "id": "none",
            "directory": "",
            "name": "None",
            "author": "",
            "themeVersion": "1.0",
            "engineVersion": 2,
            "license": "",
            "sounds": {},
        })
    })
}

fn read_presets(directory: &Path, presets: &mut Vec<(String, Value)>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            != Some(true)
        {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some(colors) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
            .and_then(|v| v.get("colors").cloned())
            .filter(|c| {
                c.as_object()
                    .map(|o| o.values().all(Value::is_string))
                    .unwrap_or(false)
            })
        else {
            log!("skipping invalid preset {}", path.display());
            continue;
        };
        presets.retain(|(n, _)| n != &name);
        presets.push((name, colors));
    }
}

/// Built-in presets first; user presets with the same name replace them.
pub fn presets(paths: &Paths, theme_id: &str) -> Value {
    let theme_id = if valid_theme_id(theme_id) {
        theme_id
    } else {
        DEFAULT_THEME
    };
    let mut presets = Vec::new();
    read_presets(
        &paths.menu_themes().join(theme_id).join("presets"),
        &mut presets,
    );
    read_presets(&paths.user_presets(theme_id), &mut presets);
    Value::Array(
        presets
            .into_iter()
            .map(|(name, colors)| json!({ "name": name, "colors": colors }))
            .collect(),
    )
}

/// Validates a preset name like Kando does.
pub fn valid_preset_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ' ')
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "avif",
];

fn list_images(root: &Path, directory: &Path, icons: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            list_images(root, &path, icons);
            continue;
        }
        let is_image = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false);
        if is_image && let Ok(relative) = path.strip_prefix(root) {
            icons.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn subdirectories(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

/// The `IconThemesInfo` for the renderers. Theme directories are given as URLs.
pub fn icon_themes_info(paths: &Paths) -> Value {
    let mut themes = Vec::new();
    let user_dir = paths.user_icon_themes();
    let _ = std::fs::create_dir_all(&user_dir);

    let mut seen = Vec::new();
    for name in subdirectories(&user_dir) {
        let directory = user_dir.join(&name);
        let mut icons = Vec::new();
        list_images(&directory, &directory, &mut icons);
        icons.sort();
        themes.push(json!({
            "name": name,
            "directory": format!("https://{USER_HOST}/icon-themes/{}", encode_path_segment(&name)),
            "icons": icons,
        }));
        seen.push(name);
    }
    for name in subdirectories(&paths.bundled_icon_themes()) {
        if seen.contains(&name) {
            continue;
        }
        let directory = paths.bundled_icon_themes().join(&name);
        let mut icons = Vec::new();
        list_images(&directory, &directory, &mut icons);
        icons.sort();
        themes.push(json!({
            "name": name,
            "directory": format!("https://{APP_HOST}/assets/icon-themes/{}", encode_path_segment(&name)),
            "icons": icons,
        }));
    }

    json!({
        "userIconDirectory": user_dir.to_string_lossy(),
        "fileIconThemes": themes,
    })
}

/// Percent-encodes a single URL path segment.
pub fn encode_path_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_are_validated() {
        assert!(valid_preset_name("Dark Night"));
        assert!(valid_preset_name("my-preset_2"));
        assert!(!valid_preset_name("../evil"));
        assert!(!valid_preset_name(" "));
    }

    #[test]
    fn theme_ids_are_validated() {
        assert!(valid_theme_id("default"));
        assert!(valid_theme_id("neon-lights"));
        assert!(valid_theme_id("purity"));
        assert!(!valid_theme_id(""));
        assert!(!valid_theme_id(".."));
        assert!(!valid_theme_id("../evil"));
        assert!(!valid_theme_id(r"a\b"));
    }

    #[test]
    fn path_segments_are_encoded() {
        assert_eq!(encode_path_segment("my icons"), "my%20icons");
        assert_eq!(encode_path_segment("图标"), "%E5%9B%BE%E6%A0%87");
    }

    #[test]
    fn bundled_theme_json5_files_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("assets")
            .join("menu-themes");
        for id in [
            "default",
            "clean-circle",
            "neon-lights",
            "purity",
            "rainbow-labels",
        ] {
            let parsed = parse_theme_file(&root.join(id))
                .unwrap_or_else(|| panic!("failed to parse theme {id}"));
            assert!(
                parsed.get("name").and_then(Value::as_str).is_some(),
                "missing name in {id}"
            );
        }
        let sound_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("assets")
            .join("sound-themes")
            .join("simple-clicks");
        let sound = parse_theme_file(&sound_root).expect("failed to parse simple-clicks");
        assert!(sound.get("sounds").and_then(Value::as_object).is_some());
    }
}
