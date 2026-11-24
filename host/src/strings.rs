// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The i18n keys used by the host. A unit test checks that all of them exist in the
//! English and Simplified Chinese translation files.

pub const SHOW_SETTINGS: &str = "main.show-settings";
pub const INHIBIT_SHORTCUTS: &str = "main.inhibit-shortcuts";
pub const UN_INHIBIT_SHORTCUTS: &str = "main.un-inhibit-shortcuts";
pub const QUIT: &str = "main.quit";
pub const NOT_BOUND: &str = "settings.not-bound";
pub const SHORTCUT_CONFLICT_TITLE: &str = "main.shortcut-conflict-title";
pub const SHORTCUT_CONFLICT_MESSAGE: &str = "main.shortcut-conflict-message";
pub const SHORTCUT_IME_CONFLICT_MESSAGE: &str = "main.shortcut-ime-conflict-message";
pub const FIRST_START_TITLE: &str = "main.first-start-title";
pub const FIRST_START_MESSAGE: &str = "main.first-start-message";
pub const SETTINGS_WINDOW_TITLE: &str = "main.settings-window-title";
pub const ACTION_FAILED_TITLE: &str = "main.action-failed-title";
pub const PATH_NOT_FOUND: &str = "main.path-not-found";
pub const COMMAND_FAILED: &str = "main.command-failed";
pub const TARGET_ELEVATED: &str = "main.target-elevated";
pub const WEBVIEW2_MISSING_TITLE: &str = "main.webview2-missing-title";
pub const WEBVIEW2_MISSING_MESSAGE: &str = "main.webview2-missing-message";
pub const CANCEL: &str = "settings.cancel";
pub const RESTORE_BACKUP: &str = "settings.restore-backup";
pub const RESTORE_GENERAL_WARNING_TITLE: &str = "settings.restore-general-settings-warning-title";
pub const RESTORE_GENERAL_WARNING_MESSAGE: &str =
    "settings.restore-general-settings-warning-message";
pub const RESTORE_GENERAL_ERROR_TITLE: &str = "settings.restore-general-settings-error-title";
pub const RESTORE_GENERAL_SUCCESS_TITLE: &str = "settings.restore-general-settings-success-title";
pub const RESTORE_GENERAL_SUCCESS_MESSAGE: &str =
    "settings.restore-general-settings-success-message";
pub const RESTORE_MENUS_WARNING_TITLE: &str = "settings.restore-menu-settings-warning-title";
pub const RESTORE_MENUS_WARNING_MESSAGE: &str = "settings.restore-menu-settings-warning-message";
pub const RESTORE_MENUS_ERROR_TITLE: &str = "settings.restore-menu-settings-error-title";
pub const RESTORE_MENUS_SUCCESS_TITLE: &str = "settings.restore-menu-settings-success-title";
pub const RESTORE_MENUS_SUCCESS_MESSAGE: &str = "settings.restore-menu-settings-success-message";
pub const IMPORT_MENU_ERROR_TITLE: &str = "settings.import-menu-error-title";
pub const IMPORT_MENU_ERROR_MESSAGE: &str = "settings.import-menu-error-message";
pub const SAVE_PRESET_ERROR_TITLE: &str = "settings.menu-themes-dialog.save-preset-error-title";
pub const SAVE_PRESET_ERROR_MESSAGE: &str = "settings.menu-themes-dialog.save-preset-error-message";

#[cfg_attr(not(test), allow(dead_code))]
pub const ALL: &[&str] = &[
    SHOW_SETTINGS,
    INHIBIT_SHORTCUTS,
    UN_INHIBIT_SHORTCUTS,
    QUIT,
    NOT_BOUND,
    SHORTCUT_CONFLICT_TITLE,
    SHORTCUT_CONFLICT_MESSAGE,
    SHORTCUT_IME_CONFLICT_MESSAGE,
    FIRST_START_TITLE,
    FIRST_START_MESSAGE,
    SETTINGS_WINDOW_TITLE,
    ACTION_FAILED_TITLE,
    PATH_NOT_FOUND,
    COMMAND_FAILED,
    TARGET_ELEVATED,
    WEBVIEW2_MISSING_TITLE,
    WEBVIEW2_MISSING_MESSAGE,
    CANCEL,
    RESTORE_BACKUP,
    RESTORE_GENERAL_WARNING_TITLE,
    RESTORE_GENERAL_WARNING_MESSAGE,
    RESTORE_GENERAL_ERROR_TITLE,
    RESTORE_GENERAL_SUCCESS_TITLE,
    RESTORE_GENERAL_SUCCESS_MESSAGE,
    RESTORE_MENUS_WARNING_TITLE,
    RESTORE_MENUS_WARNING_MESSAGE,
    RESTORE_MENUS_ERROR_TITLE,
    RESTORE_MENUS_SUCCESS_TITLE,
    RESTORE_MENUS_SUCCESS_MESSAGE,
    IMPORT_MENU_ERROR_TITLE,
    IMPORT_MENU_ERROR_MESSAGE,
    SAVE_PRESET_ERROR_TITLE,
    SAVE_PRESET_ERROR_MESSAGE,
];
