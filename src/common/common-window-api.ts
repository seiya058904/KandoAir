//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { hostBridge } from './host-bridge';
import {
  MenuItem,
  GeneralSettings,
  MenuThemeDescription,
  SoundThemeDescription,
  MenuSettings,
  IconThemesInfo,
} from '.';

/**
 * This basic set of functions will be available in both renderer processes. It will be
 * available in the menu window's renderer process as well as in the settings window's
 * renderer process. Both processes will extend this interface with additional functions.
 * In KandoAir, the functions talk to the native host via the WebView2 bridge.
 */
export const COMMON_WINDOW_API = {
  /**
   * This will print the given message to the log of the host process.
   *
   * @param message The message to print.
   */
  log: (message: string) => {
    hostBridge.send('common.log', message);
  },

  /**
   * This will return the current locale and all localization strings loaded by the host
   * for the current and all potential fallback locales.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  getLocales: (): Promise<{ current: string; data: any; fallbackLng: any }> => {
    return hostBridge.invoke('common.get-locales');
  },

  /**
   * The generalSettings object can be used to read and write the general settings. The
   * settings are persisted in the host process. When a setting is changed, the host
   * process will notify the renderer process.
   */
  generalSettings: {
    get: function (): Promise<GeneralSettings> {
      return hostBridge.invoke('common.general-settings-get');
    },
    set: function (settings: GeneralSettings): void {
      hostBridge.send('common.general-settings-set', settings);
    },
    onChange: function (
      callback: (newSettings: GeneralSettings, oldSettings: GeneralSettings) => void
    ): () => void {
      return hostBridge.on('common.general-settings-changed', callback);
    },
  },

  /**
   * The menuSettings object can be used to read and write the configured menus. They are
   * persisted in the host process. When a menu is changed, the host process will notify
   * the renderer process.
   */
  menuSettings: {
    get: function (): Promise<MenuSettings> {
      return hostBridge.invoke('common.menu-settings-get');
    },
    set: function (data: Partial<MenuSettings>): void {
      hostBridge.send('common.menu-settings-set', data);
    },
    onChange: function (
      callback: (newSettings: MenuSettings, oldSettings: MenuSettings) => void
    ): () => void {
      return hostBridge.on('common.menu-settings-changed', callback);
    },
  },

  /** This will return a IconThemesInfo describing all available user icon themes. */
  getIconThemes: (): Promise<IconThemesInfo> => {
    return hostBridge.invoke('common.get-icon-themes');
  },

  /** This will return the descriptions of the currently used menu theme. */
  getMenuTheme: (): Promise<MenuThemeDescription> => {
    return hostBridge.invoke('common.get-menu-theme');
  },

  /** This will return the description of the currently used sound theme. */
  getSoundTheme: (): Promise<SoundThemeDescription> => {
    return hostBridge.invoke('common.get-sound-theme');
  },

  /**
   * This will return the accent colors of the currently used menu theme. This may depend
   * on the current system theme (light or dark).
   */
  getCurrentMenuThemeColors: (): Promise<Record<string, string>> => {
    return hostBridge.invoke('common.get-current-menu-theme-colors');
  },

  /** This will return whether the dark mode is currently enabled for the system. */
  getIsDarkMode: (): Promise<boolean> => {
    return hostBridge.invoke('common.get-is-dark-mode');
  },

  /**
   * This will be called by the host process when the dark mode is toggled for the system.
   * It returns a function to disconnect the listener.
   */
  darkModeChanged: (callback: (darkMode: boolean) => void): (() => void) => {
    return hostBridge.on('common.dark-mode-changed', callback);
  },

  /**
   * This method creates a new menu item for a given file. Executables, shortcuts and apps
   * dragged from the start menu become launch-app items, other files and folders become
   * open-file items. It may happen that no item could be created.
   *
   * @param file The file for which a menu item should be created.
   * @returns A new menu item for the given file.
   */
  createItemForDroppedFile(file: File): Promise<MenuItem | null> {
    return hostBridge.invokeWithFiles(
      'common.create-menu-item-for-file',
      [file],
      file.name
    );
  },
};

/**
 * This type extends the global window object with the `commonAPI` object. This will be
 * available in both renderer processes. In the menu window's renderer process and in the
 * settings window's renderer process are some additional functions available. See
 * `menu/menu-window-api.ts` and `settings/settings-window-api.ts` for more information.
 */
export type WindowWithAPIs = {
  readonly commonAPI: typeof COMMON_WINDOW_API;
} & Window;
