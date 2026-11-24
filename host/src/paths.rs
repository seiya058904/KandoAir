// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Resolves the directories used by KandoAir.
//!
//! - The config directory holds `config.json`, `menus.json`, user icon themes and theme
//!   presets. It is `%APPDATA%\KandoAir` unless a `portableMode.json` next to the
//!   executable points somewhere else (same format as Kando's portable mode).
//! - The local directory holds caches (WebView2 profile, logs, icon cache). It is
//!   `%LOCALAPPDATA%\KandoAir`, or a `local` subdirectory of the config directory in
//!   portable mode.
//! - The web directory holds the bundled frontend (`resources\web` next to the
//!   executable). It can be overridden with the `KANDOAIR_WEB_DIR` environment variable.

use std::path::{Path, PathBuf};

use crate::log;

pub struct Paths {
    pub web_dir: PathBuf,
    pub config_dir: PathBuf,
    pub local_dir: PathBuf,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn portable_config_dir(exe_dir: &Path) -> Option<PathBuf> {
    let file = exe_dir.join("portableMode.json");
    let text = std::fs::read_to_string(&file).ok()?;
    let value: serde_json::Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let dir = value.get("configDirectory")?.as_str()?;
    if dir.is_empty() {
        return None;
    }
    Some(exe_dir.join(dir))
}

impl Paths {
    pub fn resolve(config_override: Option<PathBuf>) -> Paths {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));

        let web_dir =
            env_dir("KANDOAIR_WEB_DIR").unwrap_or_else(|| exe_dir.join("resources").join("web"));

        let portable = config_override
            .map(|p| if p.is_absolute() { p } else { exe_dir.join(p) })
            .or_else(|| portable_config_dir(&exe_dir));

        let (config_dir, local_dir) = match portable {
            Some(dir) => {
                let local = dir.join("local");
                (dir, local)
            }
            None => (
                env_dir("APPDATA")
                    .unwrap_or_else(|| exe_dir.clone())
                    .join("KandoAir"),
                env_dir("LOCALAPPDATA")
                    .unwrap_or_else(|| exe_dir.clone())
                    .join("KandoAir"),
            ),
        };

        for dir in [&config_dir, &local_dir] {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log!("failed to create {}: {e}", dir.display());
            }
        }

        Paths {
            web_dir,
            config_dir,
            local_dir,
        }
    }

    pub fn log_file(&self) -> PathBuf {
        self.local_dir.join("logs").join("kandoair.log")
    }

    pub fn webview_data(&self) -> PathBuf {
        self.local_dir.join("WebView2")
    }

    /// The settings window uses its own WebView2 browser processes, which exit together
    /// with the window. They need a separate profile folder.
    pub fn settings_webview_data(&self) -> PathBuf {
        self.local_dir.join("WebView2-settings")
    }

    pub fn icon_cache(&self) -> PathBuf {
        self.local_dir.join("icon-cache")
    }

    pub fn locales_dir(&self) -> PathBuf {
        self.web_dir.join("locales")
    }

    pub fn user_icon_themes(&self) -> PathBuf {
        self.config_dir.join("icon-themes")
    }

    pub fn bundled_icon_themes(&self) -> PathBuf {
        self.web_dir.join("assets").join("icon-themes")
    }

    pub fn menu_themes(&self) -> PathBuf {
        self.web_dir.join("assets").join("menu-themes")
    }

    pub fn user_menu_themes(&self) -> PathBuf {
        self.config_dir.join("menu-themes")
    }

    pub fn user_presets(&self, theme: &str) -> PathBuf {
        self.user_menu_themes().join(theme).join("presets")
    }

    pub fn sound_themes(&self) -> PathBuf {
        self.web_dir.join("assets").join("sound-themes")
    }

    pub fn user_sound_themes(&self) -> PathBuf {
        self.config_dir.join("sound-themes")
    }
}
