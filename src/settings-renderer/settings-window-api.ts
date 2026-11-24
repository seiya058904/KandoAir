//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { hostBridge } from '../common/host-bridge';
import { COMMON_WINDOW_API } from '../common/common-window-api';
import {
  BackendInfo,
  MenuThemeDescription,
  SoundThemeDescription,
  VersionInfo,
  WMInfo,
  AppDescription,
  OpenDialogOptions,
  SettingsWindowSidebarWidths,
} from '../common';

/**
 * These functions are available in the settings window's renderer process. They are
 * available on the window object as `window.settingsAPI.<method>`. In addition, the
 * common window API functions are also be available as `window.commonAPI.<method>`.
 */
export const SETTINGS_WINDOW_API = {
  /**
   * This will be called by the renderer process when it is fully loaded. This is used to
   * notify the host process that the renderer process is ready to receive messages.
   */
  settingsWindowReady: () => {
    hostBridge.send('settings-window.ready');
  },

  /** This will return some information about the backend. */
  getBackendInfo: (): Promise<BackendInfo> => {
    return hostBridge.invoke('settings-window.get-backend-info');
  },

  /** This will return the version of KandoAir and of the WebView2 runtime. */
  getVersionInfo: (): Promise<VersionInfo> => {
    return hostBridge.invoke('settings-window.get-version');
  },

  /** This will return the pointer position and the work area of its monitor. */
  getWMInfo: (): Promise<WMInfo> => {
    return hostBridge.invoke('settings-window.get-wm-info');
  },

  /** This will return the persisted widths of the settings window sidebars. */
  getSidebarWidths: (): Promise<SettingsWindowSidebarWidths> => {
    return hostBridge.invoke('settings-window.get-sidebar-widths');
  },

  /**
   * This will persist the width of one settings window sidebar.
   *
   * @param position The sidebar whose width changed.
   * @param width The new sidebar width in pixels.
   */
  setSidebarWidth: (position: 'left' | 'right', width: number) => {
    hostBridge.send('settings-window.set-sidebar-width', position, width);
  },

  /** This will return the index of the currently selected menu. */
  getCurrentMenu: function (): Promise<number> {
    return hostBridge.invoke('settings-window.get-current-menu');
  },

  /** This will return the path to the config directory. */
  getConfigDirectory: (): Promise<string> => {
    return hostBridge.invoke('settings-window.get-config-directory');
  },

  /** This will return the path to the directory of user menu-theme presets. */
  getMenuThemesDirectory: (): Promise<string> => {
    return hostBridge.invoke('settings-window.get-menu-themes-directory');
  },

  /** This will return the descriptions of all available menu themes. */
  getAllMenuThemes: (): Promise<Array<MenuThemeDescription>> => {
    return hostBridge.invoke('settings-window.get-all-menu-themes');
  },

  /** This will return the descriptions of all available sound themes. */
  getAllSoundThemes: (): Promise<Array<SoundThemeDescription>> => {
    return hostBridge.invoke('settings-window.get-all-sound-themes');
  },

  /** This will return the path to the user sound themes directory. */
  getSoundThemesDirectory: (): Promise<string> => {
    return hostBridge.invoke('settings-window.get-sound-themes-directory');
  },

  /**
   * This will return all presets for a given menu theme.
   *
   * @param themeId The ID of the theme.
   */
  getMenuThemePresets: (
    themeId: string
  ): Promise<Array<{ name: string; colors: Record<string, string> }>> => {
    return hostBridge.invoke('settings-window.get-menu-theme-presets', themeId);
  },

  /** This will return all installed applications. They are enumerated on demand. */
  getInstalledApps: (): Promise<Array<AppDescription>> => {
    return hostBridge.invoke('settings-window.get-installed-apps');
  },

  /**
   * This will show the menu with the given index.
   *
   * @param index The index of the menu to show.
   */
  openMenu: (index: number) => {
    hostBridge.send('settings-window.open-menu', index);
  },

  /**
   * This will show the web developer tools of the given window.
   *
   * @param forWindow The window for which the developer tools should be shown.
   */
  showDevTools: (forWindow: 'menu-window' | 'settings-window') => {
    hostBridge.send('settings-window.show-dev-tools', forWindow);
  },

  /** This will reload the icon themes in all windows. */
  reloadIconThemes: () => {
    hostBridge.send('settings-window.reload-icon-themes');
  },

  /** This will reload the menu theme in the menu window and refresh the theme list. */
  reloadMenuTheme: () => {
    hostBridge.send('settings-window.reload-menu-theme');
  },

  /** This will reload the sound theme in the menu window and refresh the list. */
  reloadSoundTheme: () => {
    hostBridge.send('settings-window.reload-sound-theme');
  },

  /**
   * This will be called by the host when the list of available menu themes is reloaded.
   *
   * @param callback This callback receives the updated menu theme list.
   * @returns A function to unsubscribe.
   */
  onMenuThemesChanged: (
    callback: (themes: Array<MenuThemeDescription>) => void
  ): (() => void) => {
    return hostBridge.on('settings-window.menu-themes-changed', callback);
  },

  /**
   * This will be called by the host when the list of available sound themes is reloaded.
   *
   * @param callback This callback receives the updated sound theme list.
   * @returns A function to unsubscribe.
   */
  onSoundThemesChanged: (
    callback: (themes: Array<SoundThemeDescription>) => void
  ): (() => void) => {
    return hostBridge.on('settings-window.sound-themes-changed', callback);
  },

  /**
   * This will be called by the host process when the icon themes should be reloaded.
   *
   * @param callback This callback will be called when the icon themes should be reloaded.
   */
  onReloadIconThemes: (func: () => void) => {
    hostBridge.on('settings-window.reload-icon-themes', func);
  },

  /**
   * This will show an open-file dialog.
   *
   * @param config The configuration of the dialog.
   * @returns The selected path or an empty string if the dialog was cancelled.
   */
  openFilePicker: (config: OpenDialogOptions): Promise<string> => {
    return hostBridge.invoke('settings-window.open-file-picker', config);
  },

  /**
   * This will open a file picker to let the user select a location to save the backup of
   * the config.json file.
   */
  backupGeneralSettings: () => {
    hostBridge.send('settings-window.backup-general-settings');
  },

  /**
   * This will open a file picker to let the user select a location to save the backup of
   * the menus.json file.
   */
  backupMenuSettings: () => {
    hostBridge.send('settings-window.backup-menu-settings');
  },

  /**
   * This will open a file picker to let the user select a previously backed up
   * config.json to restore the general settings from.
   */
  restoreGeneralSettings: () => {
    hostBridge.send('settings-window.restore-general-settings');
  },

  /**
   * This will open a file picker to let the user select a previously backed up menus.json
   * to restore the menu settings from.
   */
  restoreMenuSettings: () => {
    hostBridge.send('settings-window.restore-menu-settings');
  },

  /**
   * This will export the menu with the given index to a JSON file.
   *
   * @param menuIndex The index of the menu to export.
   * @returns True if the menu was exported.
   */
  exportMenu: (menuIndex: number): Promise<boolean> => {
    return hostBridge.invoke('settings-window.export-menu', menuIndex);
  },

  /**
   * This will import a menu from a JSON file.
   *
   * @returns True if a menu was imported.
   */
  importMenu: (): Promise<boolean> => {
    return hostBridge.invoke('settings-window.import-menu');
  },

  /**
   * This will export the current menu theme colors as a preset with the given name.
   *
   * @param themeId The ID of the theme.
   * @param colors The colors of the preset.
   * @param presetName The name of the preset.
   */
  exportMenuThemePreset: (
    themeId: string,
    colors: Record<string, string>,
    presetName: string
  ): Promise<void> => {
    return hostBridge.invoke(
      'settings-window.export-menu-theme-preset',
      themeId,
      colors,
      presetName
    );
  },

  /**
   * This will open the directory containing the presets for the given menu theme.
   *
   * @param themeId The ID of the theme.
   */
  openMenuThemePresetsDirectory: (themeId: string) => {
    hostBridge.send('settings-window.open-menu-theme-presets-directory', themeId);
  },

  /**
   * This will be called by the host when menu theme presets have been changed.
   *
   * @param callback This callback will be called when presets changed.
   * @returns A function to unsubscribe.
   */
  onPresetsChanged: (callback: () => void): (() => void) => {
    return hostBridge.on('settings-window.presets-changed', callback);
  },

  /**
   * This will open a file or directory in the file manager.
   *
   * @param path The path to open.
   */
  openPath: (path: string) => {
    hostBridge.send('settings-window.open-path', path);
  },

  /** These control the settings window, which has no system title bar. */
  minimizeWindow: () => {
    hostBridge.send('settings-window.minimize');
  },

  toggleMaximizeWindow: () => {
    hostBridge.send('settings-window.toggle-maximize');
  },

  closeWindow: () => {
    hostBridge.send('settings-window.close');
  },

  isWindowMaximized: (): Promise<boolean> => {
    return hostBridge.invoke('settings-window.is-maximized');
  },

  /**
   * This will be called by the host when the settings window is maximized or restored.
   *
   * @param callback This callback receives the new maximized state.
   * @returns A function to unsubscribe.
   */
  onMaximizedChanged: (callback: (maximized: boolean) => void): (() => void) => {
    return hostBridge.on('settings-window.maximized-changed', callback);
  },
};

/**
 * This type extends the global window object with the `api` objects. These `api` objects
 * contain all functions that can be called by the renderer process.
 */
export type WindowWithAPIs = {
  readonly commonAPI: typeof COMMON_WINDOW_API;
  readonly settingsAPI: typeof SETTINGS_WINDOW_API;
} & Window;
