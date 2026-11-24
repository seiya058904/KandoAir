//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import './index.scss';

import { WindowWithAPIs, SETTINGS_WINDOW_API } from './settings-window-api';
import { COMMON_WINDOW_API } from '../common/common-window-api';
declare const window: WindowWithAPIs;

// In KandoAir, the APIs are provided by the WebView2 bridge instead of a preload script.
Object.assign(window, { commonAPI: COMMON_WINDOW_API, settingsAPI: SETTINGS_WINDOW_API });

import React from 'react';
import { createRoot } from 'react-dom/client';
import i18next from 'i18next';

import App from './components/App';
import {
  useAppState,
  useGeneralSettings,
  useMenuSettings,
  getSelectedChild,
} from './state';
import { IconThemeRegistry } from '../common/icon-themes/icon-theme-registry';
import { ActionTypeRegistry } from '../common/action-type-registry';
import KeyMapper from '../common/key-mapper';
import { attachIconPickerData } from './utils/icon-picker-data';

/**
 * This file is the main entry point for Kando's settings renderer process. It is
 * responsible for drawing the settings window. The window is created when the the user
 * wants to open the settings and destroyed when the user closes it. So the code below is
 * executed whenever the user opens the settings.
 */

// Wire up the the settings --------------------------------------------------------------

// We need some information from the host process before we can start.
Promise.all([
  window.commonAPI.getLocales(),
  window.commonAPI.generalSettings.get(),
  window.commonAPI.menuSettings.get(),
  window.commonAPI.getIsDarkMode(),
  window.settingsAPI.getBackendInfo(),
  window.settingsAPI.getVersionInfo(),
  window.settingsAPI.getAllMenuThemes(),
  window.settingsAPI.getAllSoundThemes(),
  window.settingsAPI.getCurrentMenu(),
]).then(
  async ([
    locales,
    generalSettings,
    menuSettings,
    darkMode,
    backendInfo,
    versionInfo,
    menuThemes,
    soundThemes,
    selectedMenu,
  ]) => {
    // Initialize i18next with the current locale and the english fallback locale.
    await i18next.init({
      lng: locales.current,
      fallbackLng: locales.fallbackLng,
    });

    Object.keys(locales.data).forEach((key) => {
      i18next.addResourceBundle(
        key,
        'translation',
        locales.data[key].translation,
        true,
        true
      );
    });

    // This will be called whenever the icons should be reloaded.
    window.settingsAPI.onReloadIconThemes(async () => {
      await IconThemeRegistry.getInstance().init();
      attachIconPickerData();
    });

    // Initialize the keyboard layout map so shortcut recording works with IMEs.
    KeyMapper.init();

    // Initialize the icon theme registry and attach searchable icon lists.
    await IconThemeRegistry.getInstance().init();
    attachIconPickerData();

    // Validate State --------------------------------------------------------------------

    // This function is called whenever the app state or the menu settings change. It
    // makes sure that the current state is valid. For instance, if a menu is deleted, the
    // selected menu might be invalid.
    const validateState = () => {
      const { menus, collections } = useMenuSettings.getState();
      const {
        selectedMenu,
        selectMenu,
        selectedCollection,
        selectCollection,
        selectedChildPath,
        selectParent,
      } = useAppState.getState();

      // Make sure that the selected menu is valid. This could for instance happen if the
      // currently selected menu is deleted by an external event (e.g. by editing the
      // settings file) or by re-doing a previously undone deletion :).
      if (selectedMenu >= menus.length) {
        selectMenu(menus.length - 1);
        return;
      }

      // Also make sure that the selected collection is valid.
      if (selectedCollection < -1) {
        selectCollection(-1);
        return;
      }
      if (selectedCollection >= collections.length) {
        selectCollection(collections.length - 1);
        return;
      }

      // Make sure that the selected child path is valid.
      if (selectedMenu >= 0) {
        const selectedItem = getSelectedChild(menus, selectedMenu, selectedChildPath);
        if (!selectedItem) {
          selectParent();
          return;
        }
      }
    };

    useAppState.subscribe(validateState);
    useMenuSettings.subscribe(validateState);

    // Initialize the action type registry -----------------------------------------------

    ActionTypeRegistry.init(backendInfo);

    // Initialize the global state objects -----------------------------------------------

    // Make sure to not record the initial state of the menu settings in the undo history.
    useGeneralSettings.setState(generalSettings);
    useMenuSettings.temporal.getState().pause();
    useMenuSettings.setState(menuSettings);
    useMenuSettings.temporal.getState().resume();
    useAppState.setState({
      backendInfo,
      versionInfo,
      darkMode,
      menuThemes,
      soundThemes,
      selectedMenu,
    });

    // Update the state whenever the settings change in the host process, suppressing
    // echo-back to the host inside the Zustand subscribers.
    let applyingFromHost = false;
    window.commonAPI.generalSettings.onChange((newSettings) => {
      applyingFromHost = true;
      try {
        useGeneralSettings.setState(newSettings);
      } finally {
        applyingFromHost = false;
      }
    });

    window.commonAPI.menuSettings.onChange((newSettings) => {
      applyingFromHost = true;
      try {
        useMenuSettings.setState(newSettings);
      } finally {
        applyingFromHost = false;
      }
    });

    window.commonAPI.darkModeChanged((darkMode) => {
      useAppState.setState({ darkMode });
    });

    window.settingsAPI.onMenuThemesChanged((menuThemes) => {
      useAppState.setState({ menuThemes });
    });

    window.settingsAPI.onSoundThemesChanged((soundThemes) => {
      useAppState.setState({ soundThemes });
    });

    // Send the settings to the host process whenever they change.
    useGeneralSettings.subscribe((newSettings) => {
      if (!applyingFromHost) {
        window.commonAPI.generalSettings.set(newSettings);
      }
    });

    useMenuSettings.subscribe((newSettings) => {
      if (!applyingFromHost) {
        window.commonAPI.menuSettings.set({
          menus: newSettings.menus,
          collections: newSettings.collections,
        });
      }
    });

    // Make sure to never open files or links by accident when dragging them over the
    // settings window.
    window.addEventListener('dragover', (e) => e.preventDefault(), false);
    window.addEventListener('drop', (e) => e.preventDefault(), false);

    // Create the settings dialog app object.
    const root = createRoot(document.body);
    root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>
    );

    // The installed applications are only needed by the app picker and the icon picker.
    // The host enumerates them on demand, so we load them in the background.
    window.settingsAPI
      .getInstalledApps()
      .then((installedApps) => {
        useAppState.setState({ installedApps });
        IconThemeRegistry.getInstance().setSystemIconNames(
          installedApps.map((app) => app.name)
        );
      })
      .catch((error) => console.error('Failed to load the installed apps:', error));
  }
);
