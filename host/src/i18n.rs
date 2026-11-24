// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Localization. The host uses the same translation files as the renderers
//! (`resources/web/locales/<lng>/translation.json`) and resolves languages like Kando's
//! i18next setup: the requested language, its language-only code, and the fallbacks of
//! Kando's `fallbackLng` table.

use std::path::Path;

use serde_json::{Map, Value, json};
use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
use windows::core::PWSTR;

use crate::log;

/// Kando's fallback table from `src/main/index.ts`.
const FALLBACKS: &[(&str, &[&str])] = &[
    ("en-US", &["en"]),
    ("en-GB", &["en"]),
    ("de-DE", &["de", "en"]),
    ("de-CH", &["de", "en"]),
    ("de-AT", &["de", "en"]),
    ("zh-CN", &["zh-Hans", "en"]),
    ("zh-TW", &["zh-Hant", "en"]),
    ("zh", &["zh-Hans", "en"]),
    ("pt", &["pt-BR", "en"]),
];

pub struct I18n {
    /// The requested language, e.g. "zh-CN".
    pub current: String,
    /// Loaded translations in resolution order.
    data: Vec<(String, Value)>,
}

/// The first preferred UI language of the user, e.g. "zh-CN".
pub fn system_language() -> String {
    unsafe {
        let mut count = 0u32;
        let mut size = 0u32;
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut size).is_ok()
            && size > 0
        {
            let mut buffer = vec![0u16; size as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut size,
            )
            .is_ok()
            {
                let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                let first = String::from_utf16_lossy(&buffer[..end]);
                if !first.is_empty() {
                    return first;
                }
            }
        }
    }
    "en".into()
}

/// Returns the fallback codes for a language like i18next's `getFallbackCodes`.
fn fallback_codes(code: &str) -> Vec<String> {
    let language = code.split('-').next().unwrap_or(code);
    let found = FALLBACKS
        .iter()
        .find(|(k, _)| *k == code)
        .or_else(|| FALLBACKS.iter().find(|(k, _)| *k == language));
    match found {
        Some((_, list)) => list.iter().map(|s| s.to_string()).collect(),
        None => vec!["en".into()],
    }
}

/// The resolution order for a language: the code, its language part and the fallbacks.
pub fn resolution_order(code: &str) -> Vec<String> {
    let mut order = vec![code.to_string()];
    if let Some(language) = code.split('-').next()
        && language != code
    {
        order.push(language.to_string());
    }
    for fallback in fallback_codes(code) {
        if !order.contains(&fallback) {
            order.push(fallback);
        }
    }
    order
}

fn lookup<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    let mut node = value;
    for part in key.split('.') {
        node = node.get(part)?;
    }
    node.as_str()
}

impl I18n {
    /// Loads the translations for the given locale setting ("auto" or a language code).
    pub fn load(locales_dir: &Path, setting: &str) -> I18n {
        let current = if setting == "auto" || setting.is_empty() {
            system_language()
        } else {
            setting.to_string()
        };

        let mut data = Vec::new();
        for code in resolution_order(&current) {
            let file = locales_dir.join(&code).join("translation.json");
            if let Ok(text) = std::fs::read_to_string(&file) {
                match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
                    Ok(value) => data.push((code, value)),
                    Err(e) => log!("failed to parse {}: {e}", file.display()),
                }
            }
        }
        log!(
            "language {current}, loaded: {:?}",
            data.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>()
        );
        I18n { current, data }
    }

    /// Translates a dotted key. Falls back to the key itself.
    pub fn t(&self, key: &str) -> String {
        self.data
            .iter()
            .find_map(|(_, value)| lookup(value, key))
            .unwrap_or(key)
            .to_string()
    }

    /// Translates a key and replaces `{{name}}` placeholders.
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.t(key);
        for (name, value) in args {
            text = text.replace(&format!("{{{{{name}}}}}"), value);
        }
        text
    }

    /// The payload of `common.get-locales` for the renderers.
    pub fn renderer_payload(&self) -> Value {
        let mut data = Map::new();
        for (code, value) in &self.data {
            data.insert(code.clone(), json!({ "translation": value }));
        }
        let mut fallback = Map::new();
        for (code, list) in FALLBACKS {
            fallback.insert(code.to_string(), json!(list));
        }
        fallback.insert("default".into(), json!(["en"]));
        json!({ "current": self.current, "data": data, "fallbackLng": fallback })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_follows_kando_fallbacks() {
        assert_eq!(resolution_order("zh-CN"), ["zh-CN", "zh", "zh-Hans", "en"]);
        assert_eq!(resolution_order("zh-SG"), ["zh-SG", "zh", "zh-Hans", "en"]);
        assert_eq!(resolution_order("de-AT"), ["de-AT", "de", "en"]);
        assert_eq!(resolution_order("fr"), ["fr", "en"]);
        assert_eq!(resolution_order("pt"), ["pt", "pt-BR", "en"]);
    }

    /// Every key the host looks up must exist in English and Chinese.
    #[test]
    fn host_keys_exist() {
        let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("../locales");
        for code in ["en", "zh-Hans", "zh-Hant"] {
            let text =
                std::fs::read_to_string(locales.join(code).join("translation.json")).unwrap();
            let value: Value = serde_json::from_str(&text).unwrap();
            for key in crate::strings::ALL {
                assert!(lookup(&value, key).is_some(), "{code}: missing {key}");
            }
        }
    }
}
