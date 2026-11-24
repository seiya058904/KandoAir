// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The two settings files of KandoAir:
//!
//! - `config.json` contains the general settings (Kando's `GeneralSettings`).
//! - `menus.json` contains the configured menus (Kando's `MenuSettings`, v2 format).
//!
//! Both files use Kando's JSON layout so that menus can be exchanged with Kando 3.x. When
//! loading foreign data, menus are normalized to KandoAir's feature set: every button runs
//! at most one of the five supported actions, hover/open workflows are dropped, and the
//! center-click behavior of the root and of submenus is fixed (close / go back).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::log;

/// The version which is written into the settings files.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The action types supported by KandoAir. Must match the TypeScript registry.
#[cfg_attr(not(test), allow(dead_code))]
pub const ACTION_TYPES: [&str; 9] = [
    "launch-app",
    "open-file",
    "open-uri",
    "simulate-hotkey",
    "execute-command",
    "set-clipboard",
    "execute-macro",
    "focus-window",
    "open-menu",
];

// ------------------------------------------------------------------------------------ //
// General settings
// ------------------------------------------------------------------------------------ //

/// Mirrors `GENERAL_SETTINGS_SCHEMA_V1` minus the settings of removed features.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    pub version: String,
    pub locale: String,
    pub auto_start: bool,
    pub show_introduction_dialog: bool,
    pub menu_theme: String,
    pub dark_menu_theme: String,
    pub menu_theme_colors: Map<String, Value>,
    pub dark_menu_theme_colors: Map<String, Value>,
    pub enable_dark_mode_for_menu_themes: bool,
    pub sound_theme: String,
    pub sound_volume: f64,
    pub enable_selection_wedges: bool,
    pub underline_quick_select_key: bool,
    pub draw_quick_select_key: bool,
    pub fade_in_duration: f64,
    pub fade_out_duration: f64,
    pub enable_menu_animations: bool,
    pub enable_pointer_reactive_effects: bool,
    pub settings_window_color_scheme: String,
    pub settings_window_flavor: String,
    pub tray_icon_flavor: String,
    pub hardware_acceleration: bool,
    pub lazy_initialization: bool,
    pub zoom_factor: f64,
    pub hide_settings_button: bool,
    pub settings_button_position: String,
    pub max_selection_radius: f64,
    pub center_dead_zone: f64,
    pub min_parent_distance: f64,
    pub drag_threshold: f64,
    pub keep_input_focus: bool,
    pub hide_on_focus_out: bool,
    pub enable_marking_mode: bool,
    pub enable_turbo_mode: bool,
    pub warp_mouse: bool,
    pub return_pointer_to_menu_opening_position: bool,
    pub hover_mode_needs_confirmation: bool,
    pub trigger_center_click_on_key_release: bool,
    pub gesture_min_stroke_length: f64,
    pub gesture_min_stroke_angle: f64,
    pub gesture_jitter_threshold: f64,
    pub gesture_pause_timeout: f64,
    pub fixed_stroke_length: f64,
    pub rmb_selects_parent: bool,
    pub same_shortcut_behavior: String,
    pub use_default_os_show_settings_hotkey: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            version: VERSION.to_string(),
            locale: "auto".into(),
            auto_start: false,
            show_introduction_dialog: true,
            menu_theme: "default".into(),
            dark_menu_theme: "default".into(),
            menu_theme_colors: Map::new(),
            dark_menu_theme_colors: Map::new(),
            enable_dark_mode_for_menu_themes: false,
            sound_theme: "none".into(),
            sound_volume: 0.5,
            enable_selection_wedges: false,
            underline_quick_select_key: true,
            draw_quick_select_key: false,
            fade_in_duration: 75.0,
            fade_out_duration: 100.0,
            enable_menu_animations: true,
            enable_pointer_reactive_effects: true,
            settings_window_color_scheme: "system".into(),
            settings_window_flavor: "auto".into(),
            tray_icon_flavor: "color".into(),
            hardware_acceleration: true,
            lazy_initialization: false,
            zoom_factor: 1.0,
            hide_settings_button: false,
            settings_button_position: "bottom-right".into(),
            max_selection_radius: 0.0,
            center_dead_zone: 50.0,
            min_parent_distance: 150.0,
            drag_threshold: 15.0,
            keep_input_focus: false,
            hide_on_focus_out: true,
            enable_marking_mode: true,
            enable_turbo_mode: true,
            warp_mouse: true,
            return_pointer_to_menu_opening_position: false,
            hover_mode_needs_confirmation: false,
            trigger_center_click_on_key_release: false,
            gesture_min_stroke_length: 150.0,
            gesture_min_stroke_angle: 20.0,
            gesture_jitter_threshold: 10.0,
            gesture_pause_timeout: 100.0,
            fixed_stroke_length: 0.0,
            rmb_selects_parent: false,
            same_shortcut_behavior: "nothing".into(),
            use_default_os_show_settings_hotkey: true,
        }
    }
}

fn one_of(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_string();
    }
}

fn at_least(value: &mut f64, min: f64, fallback: f64) {
    if !value.is_finite() || *value < min {
        *value = fallback;
    }
}

impl GeneralSettings {
    /// Parses general settings leniently: unknown keys are dropped, invalid values fall
    /// back to their defaults individually instead of discarding the whole file.
    pub fn from_value(value: &Value) -> GeneralSettings {
        let defaults = serde_json::to_value(GeneralSettings::default()).unwrap_or_default();
        let mut merged = defaults.as_object().cloned().unwrap_or_default();
        if let Some(object) = value.as_object() {
            for (key, default) in defaults.as_object().into_iter().flatten() {
                if let Some(v) = object.get(key) {
                    let same_kind = matches!(
                        (default, v),
                        (Value::Bool(_), Value::Bool(_))
                            | (Value::Number(_), Value::Number(_))
                            | (Value::String(_), Value::String(_))
                            | (Value::Object(_), Value::Object(_))
                    );
                    if same_kind {
                        merged.insert(key.clone(), v.clone());
                    }
                }
            }
        }
        let mut settings: GeneralSettings =
            serde_json::from_value(Value::Object(merged)).unwrap_or_default();
        settings.normalize();
        settings
    }

    /// Clamps values to the ranges of Kando's zod schema and fixes removed features.
    pub fn normalize(&mut self) {
        let d = GeneralSettings::default();
        self.version = VERSION.to_string();
        if !crate::themes::valid_theme_id(&self.menu_theme) {
            self.menu_theme = crate::themes::DEFAULT_THEME.into();
        }
        if !crate::themes::valid_theme_id(&self.dark_menu_theme) {
            self.dark_menu_theme = crate::themes::DEFAULT_THEME.into();
        }
        if self.sound_theme != "none" && !crate::themes::valid_theme_id(&self.sound_theme) {
            self.sound_theme = "none".into();
        }
        if !self.sound_volume.is_finite() {
            self.sound_volume = d.sound_volume;
        } else {
            self.sound_volume = self.sound_volume.clamp(0.0, 1.0);
        }
        one_of(
            &mut self.settings_window_color_scheme,
            &["light", "dark", "system"],
            "system",
        );
        one_of(
            &mut self.settings_window_flavor,
            &[
                "auto",
                "sakura-light",
                "sakura-dark",
                "sakura-system",
                "transparent-light",
                "transparent-dark",
                "transparent-system",
            ],
            "auto",
        );
        one_of(
            &mut self.tray_icon_flavor,
            &["light", "dark", "color", "black", "white", "none"],
            "color",
        );
        one_of(
            &mut self.settings_button_position,
            &["top-left", "top-right", "bottom-left", "bottom-right"],
            "bottom-right",
        );
        one_of(
            &mut self.same_shortcut_behavior,
            &[
                "cycle-from-first",
                "cycle-from-recent",
                "re-open",
                "close",
                "nothing",
            ],
            "nothing",
        );
        at_least(&mut self.fade_in_duration, 0.0, d.fade_in_duration);
        at_least(&mut self.fade_out_duration, 0.0, d.fade_out_duration);
        at_least(&mut self.zoom_factor, 0.5, d.zoom_factor);
        at_least(&mut self.max_selection_radius, 0.0, d.max_selection_radius);
        at_least(&mut self.center_dead_zone, 0.0, d.center_dead_zone);
        at_least(&mut self.min_parent_distance, 0.0, d.min_parent_distance);
        at_least(&mut self.drag_threshold, 0.0, d.drag_threshold);
        at_least(
            &mut self.gesture_min_stroke_length,
            0.0,
            d.gesture_min_stroke_length,
        );
        at_least(
            &mut self.gesture_min_stroke_angle,
            0.0,
            d.gesture_min_stroke_angle,
        );
        at_least(
            &mut self.gesture_jitter_threshold,
            0.0,
            d.gesture_jitter_threshold,
        );
        at_least(
            &mut self.gesture_pause_timeout,
            0.0,
            d.gesture_pause_timeout,
        );
        at_least(&mut self.fixed_stroke_length, 0.0, d.fixed_stroke_length);
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ------------------------------------------------------------------------------------ //
// Menu settings
// ------------------------------------------------------------------------------------ //

/// One recorded key event of an `execute-macro` action.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MacroEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
}

/// A single-step action. Mirrors the action schemas in `menu-settings-v2.ts`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    LaunchApp {
        target: String,
    },
    OpenFile {
        path: String,
    },
    OpenUri {
        uri: String,
    },
    SimulateHotkey {
        hotkey: String,
    },
    ExecuteCommand {
        command: String,
        #[serde(default = "default_true")]
        detached: bool,
    },
    SetClipboard {
        text: String,
    },
    ExecuteMacro {
        #[serde(default)]
        r#macro: Vec<MacroEvent>,
    },
    FocusWindow {
        #[serde(rename = "appName", skip_serializing_if = "Option::is_none")]
        app_name: Option<String>,
        #[serde(rename = "windowName", skip_serializing_if = "Option::is_none")]
        window_name: Option<String>,
    },
    OpenMenu {
        menu: String,
    },
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_select_key: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MenuItem {
    Root {
        name: String,
        icon: String,
        #[serde(rename = "iconTheme")]
        icon_theme: String,
        #[serde(rename = "activateWorkflow", skip_serializing_if = "Option::is_none")]
        activate_workflow: Option<Workflow>,
        children: Vec<MenuItem>,
    },
    Button {
        name: String,
        icon: String,
        #[serde(rename = "iconTheme")]
        icon_theme: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        angle: Option<f64>,
        #[serde(rename = "selectWorkflow", skip_serializing_if = "Option::is_none")]
        select_workflow: Option<Workflow>,
    },
    Submenu {
        name: String,
        icon: String,
        #[serde(rename = "iconTheme")]
        icon_theme: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        angle: Option<f64>,
        #[serde(rename = "openWorkflow", skip_serializing_if = "Option::is_none")]
        open_workflow: Option<Workflow>,
        #[serde(rename = "activateWorkflow", skip_serializing_if = "Option::is_none")]
        activate_workflow: Option<Workflow>,
        children: Vec<MenuItem>,
    },
}

impl MenuItem {
    pub fn name(&self) -> &str {
        match self {
            MenuItem::Root { name, .. }
            | MenuItem::Button { name, .. }
            | MenuItem::Submenu { name, .. } => name,
        }
    }

    pub fn children(&self) -> &[MenuItem] {
        match self {
            MenuItem::Root { children, .. } | MenuItem::Submenu { children, .. } => children,
            MenuItem::Button { .. } => &[],
        }
    }

    /// Returns the item at the given child-index path.
    pub fn at_path(&self, path: &[usize]) -> Option<&MenuItem> {
        let mut item = self;
        for &index in path {
            item = item.children().get(index)?;
        }
        Some(item)
    }

    /// The single action of a button, if any.
    pub fn action(&self) -> Option<&Action> {
        match self {
            MenuItem::Button {
                select_workflow: Some(workflow),
                ..
            } => workflow.actions.first(),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenArea {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_max: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_max: Option<i32>,
}

impl ScreenArea {
    pub fn is_empty(&self) -> bool {
        self.x_min.is_none()
            && self.x_max.is_none()
            && self.y_min.is_none()
            && self.y_max.is_none()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MenuConditions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_area: Option<ScreenArea>,
}

impl MenuConditions {
    pub fn is_empty(&self) -> bool {
        self.app_name.as_deref().is_none_or(|s| s.trim().is_empty())
            && self
                .window_name
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
            && self.screen_area.as_ref().is_none_or(ScreenArea::is_empty)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub root: MenuItem,
    pub shortcut: String,
    pub use_fixed_position: bool,
    pub fixed_menu_position: Vec2,
    pub anchored: bool,
    pub hover_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<MenuConditions>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MenuCollection {
    pub name: String,
    pub icon: String,
    pub icon_theme: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MenuSettings {
    pub version: String,
    pub menus: Vec<Menu>,
    pub collections: Vec<MenuCollection>,
}

impl Default for MenuSettings {
    fn default() -> Self {
        MenuSettings {
            version: VERSION.to_string(),
            menus: Vec::new(),
            collections: vec![MenuCollection {
                name: "Favorites".into(),
                icon: "favorite".into(),
                icon_theme: "material-symbols-rounded".into(),
                tags: vec!["favs".into()],
            }],
        }
    }
}

fn string_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn strings_of(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn bool_of(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn opt_string_of(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn opt_i32_of(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64)))
        .map(|n| n as i32)
}

/// Converts one Kando workflow action into a KandoAir action. Control actions (like
/// close-menu) and unsupported action types yield `None`.
pub fn action_from_value(value: &Value) -> Option<Action> {
    let kind = value.get("type")?.as_str()?;
    match kind {
        "launch-app" => Some(Action::LaunchApp {
            target: string_of(value, "target"),
        }),
        "open-file" => Some(Action::OpenFile {
            path: string_of(value, "path"),
        }),
        "open-uri" => Some(Action::OpenUri {
            uri: string_of(value, "uri"),
        }),
        "simulate-hotkey" => Some(Action::SimulateHotkey {
            hotkey: string_of(value, "hotkey"),
        }),
        "execute-command" => Some(Action::ExecuteCommand {
            command: string_of(value, "command"),
            detached: bool_of(value, "detached", true),
        }),
        "set-clipboard" => Some(Action::SetClipboard {
            text: string_of(value, "text"),
        }),
        "execute-macro" => {
            let events = value
                .get("macro")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| {
                            let event_type = e.get("type")?.as_str()?;
                            if event_type != "keyDown" && event_type != "keyUp" {
                                return None;
                            }
                            let key = e.get("key")?.as_str()?.to_string();
                            let delay = e.get("delay").and_then(Value::as_f64);
                            Some(MacroEvent {
                                event_type: event_type.to_string(),
                                key,
                                delay,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Action::ExecuteMacro { r#macro: events })
        }
        "focus-window" => Some(Action::FocusWindow {
            app_name: opt_string_of(value, "appName"),
            window_name: opt_string_of(value, "windowName"),
        }),
        "open-menu" => Some(Action::OpenMenu {
            menu: string_of(value, "menu"),
        }),
        _ => None,
    }
}

fn quick_key_of(workflow: Option<&Value>) -> Option<String> {
    workflow
        .and_then(|w| w.get("quickSelectKey"))
        .and_then(Value::as_str)
        .filter(|k| !k.is_empty())
        .map(str::to_string)
}

/// Converts a Kando menu item (any version 2 item) into a normalized KandoAir item.
fn item_from_value(value: &Value, is_root: bool) -> Option<MenuItem> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let name = string_of(value, "name");
    let icon = string_of(value, "icon");
    let icon_theme = string_of(value, "iconTheme");
    let angle = value.get("angle").and_then(Value::as_f64);
    let children = || -> Vec<MenuItem> {
        value
            .get("children")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|c| item_from_value(c, false)).collect())
            .unwrap_or_default()
    };

    if is_root {
        return Some(MenuItem::Root {
            name,
            icon,
            icon_theme,
            activate_workflow: Some(Workflow {
                quick_select_key: quick_key_of(value.get("activateWorkflow"))
                    .or_else(|| Some("Backspace".into())),
                actions: Vec::new(),
            }),
            children: children(),
        });
    }

    match kind {
        "button" => {
            let workflow = value.get("selectWorkflow");
            let action = workflow
                .and_then(|w| w.get("actions"))
                .and_then(Value::as_array)
                .and_then(|actions| actions.iter().find_map(action_from_value));
            Some(MenuItem::Button {
                name,
                icon,
                icon_theme,
                angle,
                select_workflow: Some(Workflow {
                    quick_select_key: quick_key_of(workflow),
                    actions: action.into_iter().collect(),
                }),
            })
        }
        "submenu" => Some(MenuItem::Submenu {
            name,
            icon,
            icon_theme,
            angle,
            open_workflow: Some(Workflow {
                quick_select_key: quick_key_of(value.get("openWorkflow")),
                actions: Vec::new(),
            }),
            activate_workflow: Some(Workflow {
                quick_select_key: quick_key_of(value.get("activateWorkflow"))
                    .or_else(|| Some("Backspace".into())),
                actions: Vec::new(),
            }),
            children: children(),
        }),
        _ => None,
    }
}

fn conditions_from_value(value: Option<&Value>) -> Option<MenuConditions> {
    let value = value?;
    let screen_area = value.get("screenArea").and_then(|area| {
        let parsed = ScreenArea {
            x_min: opt_i32_of(area, "xMin"),
            x_max: opt_i32_of(area, "xMax"),
            y_min: opt_i32_of(area, "yMin"),
            y_max: opt_i32_of(area, "yMax"),
        };
        (!parsed.is_empty()).then_some(parsed)
    });
    let conditions = MenuConditions {
        app_name: opt_string_of(value, "appName"),
        window_name: opt_string_of(value, "windowName"),
        screen_area,
    };
    (!conditions.is_empty()).then_some(conditions)
}

/// Converts a Kando 3.x menu (or a KandoAir menu) into a normalized KandoAir menu.
pub fn menu_from_value(value: &Value) -> Option<Menu> {
    let root = item_from_value(value.get("root")?, true)?;
    let position = value.get("fixedMenuPosition");
    Some(Menu {
        root,
        shortcut: string_of(value, "shortcut"),
        use_fixed_position: bool_of(value, "useFixedPosition", false),
        fixed_menu_position: Vec2 {
            x: position
                .and_then(|p| p.get("x"))
                .and_then(Value::as_f64)
                .unwrap_or(0.5),
            y: position
                .and_then(|p| p.get("y"))
                .and_then(Value::as_f64)
                .unwrap_or(0.5),
        },
        anchored: bool_of(value, "anchored", false),
        hover_mode: bool_of(value, "hoverMode", false),
        conditions: conditions_from_value(value.get("conditions")),
        tags: strings_of(value, "tags"),
    })
}

impl MenuSettings {
    /// Parses menu settings in Kando's v2 format (Kando 3.x) or KandoAir's format.
    /// Returns an error for files which are not in the v2 format.
    pub fn from_value(value: &Value) -> Result<MenuSettings, String> {
        let menus = value
            .get("menus")
            .and_then(Value::as_array)
            .ok_or("The file does not contain a list of menus.")?;

        // Pre-3.0 files store items with a "type" like "command" or "hotkey" and have no
        // "button" items. We do not migrate those.
        let looks_like_v1 = menus.iter().any(|m| {
            m.get("root")
                .and_then(|r| r.get("type"))
                .and_then(Value::as_str)
                == Some("submenu")
        });
        if looks_like_v1 {
            return Err("Menus from Kando versions before 3.0 are not supported.".into());
        }

        let mut settings = MenuSettings {
            version: VERSION.to_string(),
            menus: menus.iter().filter_map(menu_from_value).collect(),
            collections: Vec::new(),
        };

        match value.get("collections").and_then(Value::as_array) {
            Some(collections) => {
                settings.collections = collections
                    .iter()
                    .map(|c| MenuCollection {
                        name: string_of(c, "name"),
                        icon: string_of(c, "icon"),
                        icon_theme: string_of(c, "iconTheme"),
                        tags: strings_of(c, "tags"),
                    })
                    .collect();
            }
            None => settings.collections = MenuSettings::default().collections,
        }

        Ok(settings)
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ------------------------------------------------------------------------------------ //
// Files
// ------------------------------------------------------------------------------------ //

/// Reads a JSON file. Returns `None` if the file does not exist.
pub fn read_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let text = text.trim_start_matches('\u{feff}');
            serde_json::from_str(text)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Writes a JSON file atomically: the content is written to a temporary file which then
/// replaces the original.
pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&temp, path).map_err(|e| e.to_string())
}

/// If the given file was written by another version, a copy is stored in the backups
/// directory before it gets rewritten in the current format.
pub fn backup_if_version_differs(path: &Path, value: &Value, backup_dir: &Path) {
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if version == VERSION {
        return;
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("settings");
    let target = backup_dir.join(format!("{name}-{version}.json"));
    if target.exists() {
        return;
    }
    let _ = fs::create_dir_all(backup_dir);
    match fs::copy(path, &target) {
        Ok(_) => log!("backed up {} to {}", path.display(), target.display()),
        Err(e) => log!("failed to back up {}: {e}", path.display()),
    }
}

/// The settings file paths in the config directory.
pub struct Files {
    pub general: PathBuf,
    pub menus: PathBuf,
    pub window_state: PathBuf,
    pub backups: PathBuf,
}

impl Files {
    pub fn new(config_dir: &Path) -> Files {
        Files {
            general: config_dir.join("config.json"),
            menus: config_dir.join("menus.json"),
            window_state: config_dir.join("settings-window-state.json"),
            backups: config_dir.join("backups"),
        }
    }
}

/// Loads the general settings. Missing or broken files result in defaults; broken files
/// are kept as a backup.
pub fn load_general(files: &Files) -> GeneralSettings {
    match read_json(&files.general) {
        Ok(Some(value)) => {
            backup_if_version_differs(&files.general, &value, &files.backups);
            GeneralSettings::from_value(&value)
        }
        Ok(None) => GeneralSettings::default(),
        Err(e) => {
            log!("failed to read general settings: {e}");
            backup_broken(&files.general, &files.backups);
            GeneralSettings::default()
        }
    }
}

/// Loads the menu settings. Missing files result in empty settings (the caller adds the
/// example menu); broken files are kept as a backup.
pub fn load_menus(files: &Files) -> MenuSettings {
    match read_json(&files.menus) {
        Ok(Some(value)) => {
            backup_if_version_differs(&files.menus, &value, &files.backups);
            MenuSettings::from_value(&value).unwrap_or_else(|e| {
                log!("failed to parse menu settings: {e}");
                backup_broken(&files.menus, &files.backups);
                MenuSettings::default()
            })
        }
        Ok(None) => MenuSettings::default(),
        Err(e) => {
            log!("failed to read menu settings: {e}");
            backup_broken(&files.menus, &files.backups);
            MenuSettings::default()
        }
    }
}

fn backup_broken(path: &Path, backup_dir: &Path) {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("settings");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = fs::create_dir_all(backup_dir);
    let _ = fs::copy(path, backup_dir.join(format!("{name}-broken-{stamp}.json")));
}

/// Builds the menu which is created when no menus exist. Item names are i18n keys which
/// are translated with the given function.
pub fn example_menu(translate: &dyn Fn(&str) -> String) -> Option<Menu> {
    let value: Value = serde_json::from_str(include_str!("../../assets/example-menu.json")).ok()?;
    let mut menu = menu_from_value(&value)?;
    fn translate_item(item: &mut MenuItem, translate: &dyn Fn(&str) -> String) {
        match item {
            MenuItem::Root { name, children, .. } | MenuItem::Submenu { name, children, .. } => {
                *name = translate(name);
                for child in children {
                    translate_item(child, translate);
                }
            }
            MenuItem::Button { name, .. } => *name = translate(name),
        }
    }
    translate_item(&mut menu.root, translate);
    Some(menu)
}

/// Exported menu files (`settings-window.export-menu`) use this layout.
pub fn exported_menu_value(menu: &Menu) -> Value {
    json!({ "version": VERSION, "menu": menu })
}

/// Parses an exported menu file (Kando 3.x or KandoAir).
pub fn menu_from_exported(value: &Value) -> Result<Menu, String> {
    let menu = value
        .get("menu")
        .ok_or("The file does not contain a menu.")?;
    if menu
        .get("root")
        .and_then(|r| r.get("type"))
        .and_then(Value::as_str)
        == Some("submenu")
    {
        return Err("Menus from Kando versions before 3.0 are not supported.".into());
    }
    menu_from_value(menu).ok_or_else(|| "The menu could not be read.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_settings_fall_back_per_field() {
        let value = json!({
            "fadeOutDuration": -5,
            "zoomFactor": 2,
            "trayIconFlavor": "rainbow",
            "enableAchievements": true,
            "menuTheme": "neon-lights",
            "darkMenuTheme": "../evil",
            "locale": 3
        });
        let s = GeneralSettings::from_value(&value);
        assert_eq!(s.fade_out_duration, 100.0);
        assert_eq!(s.zoom_factor, 2.0);
        assert_eq!(s.tray_icon_flavor, "color");
        assert_eq!(s.menu_theme, "neon-lights");
        assert_eq!(s.dark_menu_theme, "default");
        assert_eq!(s.locale, "auto");
        assert!(s.to_value().get("enableAchievements").is_none());
    }

    #[test]
    fn kando_workflows_are_normalized_to_single_actions() {
        let value = json!({
            "version": "3.0.1",
            "menus": [{
                "root": {
                    "type": "root", "name": "Example", "icon": "a", "iconTheme": "b",
                    "activateWorkflow": { "quickSelectKey": "Backspace", "actions": [{ "type": "close-menu" }] },
                    "children": [
                        { "type": "button", "name": "Paste", "icon": "c", "iconTheme": "d",
                          "selectWorkflow": { "quickSelectKey": "V", "actions": [
                            { "type": "inhibit-shortcuts" }, { "type": "close-menu" },
                            { "type": "simulate-hotkey", "hotkey": "ControlLeft+KeyV" } ] },
                          "hoverWorkflow": { "actions": [{ "type": "delay", "duration": 1 }] } },
                        { "type": "button", "name": "Clip", "icon": "c", "iconTheme": "d",
                          "selectWorkflow": { "actions": [{ "type": "set-clipboard", "text": "x" }] } },
                        { "type": "submenu", "name": "Apps", "icon": "e", "iconTheme": "f",
                          "openWorkflow": { "quickSelectKey": "A", "actions": [{ "type": "focus-window" }] },
                          "children": [
                            { "type": "button", "name": "Web", "icon": "g", "iconTheme": "h",
                              "selectWorkflow": { "actions": [
                                { "type": "execute-command", "command": "start \"\" x", "detached": false },
                                { "type": "close-menu" } ] } } ] }
                    ]
                },
                "shortcut": "Control+Space",
                "shortcutID": "",
                "conditions": { "appName": "firefox" },
                "tags": ["favs"]
            }]
        });
        let settings = MenuSettings::from_value(&value).unwrap();
        let menu = &settings.menus[0];
        assert_eq!(menu.shortcut, "Control+Space");
        assert_eq!(
            menu.root.at_path(&[0]).unwrap().action(),
            Some(&Action::SimulateHotkey {
                hotkey: "ControlLeft+KeyV".into()
            })
        );
        assert_eq!(
            menu.root.at_path(&[1]).unwrap().action(),
            Some(&Action::SetClipboard { text: "x".into() })
        );
        assert_eq!(
            menu.root.at_path(&[2, 0]).unwrap().action(),
            Some(&Action::ExecuteCommand {
                command: "start \"\" x".into(),
                detached: false
            })
        );
        assert_eq!(
            menu.conditions.as_ref().and_then(|c| c.app_name.as_deref()),
            Some("firefox")
        );
        let out = settings.to_value();
        let text = out.to_string();
        assert!(!text.contains("hoverWorkflow"));
        assert!(text.contains("\"conditions\""));
        assert!(!text.contains("close-menu"));
        assert!(text.contains("\"quickSelectKey\":\"A\""));
        assert_eq!(settings.collections.len(), 1);

        // Normalized output must survive a second round trip unchanged.
        assert_eq!(MenuSettings::from_value(&out).unwrap(), settings);
    }

    #[test]
    fn pre_v3_menus_are_rejected() {
        let value = json!({ "menus": [{ "root": { "type": "submenu", "name": "x", "icon": "", "iconTheme": "", "children": [] } }] });
        assert!(MenuSettings::from_value(&value).is_err());
    }

    fn repo_file(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        std::fs::read_to_string(root.join(path)).unwrap()
    }

    /// The Rust settings must have exactly the keys of the TypeScript schema.
    #[test]
    fn general_settings_match_typescript_schema() {
        let ts = repo_file("src/common/settings-schemata/general-settings-v1.ts");
        let mut ts_keys: Vec<String> = ts
            .lines()
            .filter_map(|l| l.strip_prefix("  "))
            .filter_map(|l| l.split_once(": z").map(|(k, _)| k.to_string()))
            .filter(|k| k.chars().all(|c| c.is_ascii_alphanumeric()))
            .collect();
        let value = GeneralSettings::default().to_value();
        let mut rust_keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        ts_keys.sort();
        rust_keys.sort();
        assert_eq!(rust_keys, ts_keys);
    }

    /// The action whitelist must match the TypeScript schema and registry.
    #[test]
    fn action_types_match_typescript() {
        let schema = repo_file("src/common/settings-schemata/menu-settings-v2.ts");
        // Only the action schemas, which are defined before the union of all actions.
        let schema = schema
            .split("export const WORKFLOW_ACTION_SCHEMA_V2")
            .next()
            .unwrap();
        let mut schema_types: Vec<String> = schema
            .split("type: z.literal('")
            .skip(1)
            .filter_map(|rest| rest.split_once('\'').map(|(t, _)| t.to_string()))
            .collect();
        schema_types.sort();
        let mut rust_types: Vec<String> = ACTION_TYPES.iter().map(|t| t.to_string()).collect();
        rust_types.sort();
        assert_eq!(schema_types, rust_types);

        let registry = repo_file("src/common/action-type-registry.ts");
        let list = registry.split("export const ACTION_TYPES").nth(1).unwrap();
        let list = &list[..list.find("];").unwrap()];
        let registry_types: Vec<&str> = list.split('\'').skip(1).step_by(2).collect();
        assert_eq!(registry_types, ACTION_TYPES);
    }

    #[test]
    fn action_types_cover_all_variants() {
        for kind in ACTION_TYPES {
            let mut value = json!({ "type": kind });
            for key in ["target", "path", "uri", "hotkey", "command", "text", "menu"] {
                value[key] = json!("x");
            }
            value["macro"] = json!([{ "type": "keyDown", "key": "KeyA", "delay": 10 }]);
            value["appName"] = json!("notepad");
            let action = action_from_value(&value).expect(kind);
            assert_eq!(serde_json::to_value(&action).unwrap()["type"], kind);
        }
    }
}
