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

import { WindowWithAPIs, MENU_WINDOW_API } from './menu-window-api';
import { COMMON_WINDOW_API } from '../common/common-window-api';
declare const window: WindowWithAPIs;

// In KandoAir, the APIs are provided by the WebView2 bridge instead of a preload script.
Object.assign(window, { commonAPI: COMMON_WINDOW_API, menuAPI: MENU_WINDOW_API });

import { Menu } from './menu';
import { SettingsButton } from './settings-button';
import { MenuTheme } from './menu-theme';
import { SoundTheme } from './sound-theme';
import { IconThemeRegistry } from '../common/icon-themes/icon-theme-registry';
import KeyMapper from '../common/key-mapper';
import { CenterText } from './center-text';
import { MenuInteractionType } from '../common';

/**
 * This file is the main entry point for Kando's menu renderer process. It is responsible
 * for drawing the menu and handling user input. It is created when Kando starts and stays
 * hidden until the user requests a menu. So the code below is executed only once.
 */

// Wire up the menu and the settings button -----------------------------------------------

Promise.all([
  window.commonAPI.getMenuTheme(),
  window.commonAPI.getCurrentMenuThemeColors(),
  window.commonAPI.getSoundTheme(),
  window.commonAPI.generalSettings.get(),
]).then(async ([themeDescription, colors, soundThemeDescription, settings]) => {
  // First, we create a new menu theme and load the description we got from the main
  // process.
  const menuTheme = new MenuTheme();
  menuTheme.loadDescription(themeDescription);
  menuTheme.setColors(colors);

  const soundTheme = new SoundTheme();
  soundTheme.loadDescription(soundThemeDescription);
  soundTheme.setVolume(settings.soundVolume);

  const reloadSoundTheme = async () => {
    const description = await window.commonAPI.getSoundTheme();
    soundTheme.loadDescription(description);
  };

  // We also create a new center text object. This will be used to display the name of the
  // currently selected menu item in the center of the menu.
  const centerText = new CenterText(document.getElementById('kando-menu'));
  centerText.setDiameter(menuTheme.centerTextWrapWidth);
  centerText.setEnabled(menuTheme.drawCenterText);

  // When the color scheme or the accent colors change, we reload the menu theme.
  const reloadMenuTheme = async () => {
    Promise.all([
      window.commonAPI.getMenuTheme(),
      window.commonAPI.getCurrentMenuThemeColors(),
    ]).then(([themeDescription, colors]) => {
      menuTheme.loadDescription(themeDescription);
      menuTheme.setColors(colors);
      centerText.setDiameter(menuTheme.centerTextWrapWidth);
      centerText.setEnabled(menuTheme.drawCenterText);
      const [root, menuOptions] = menu.getCurrentRequest();
      if (root) {
        menu.show(root, menuOptions);
      }
    });
  };

  window.commonAPI.darkModeChanged(reloadMenuTheme);
  window.menuAPI.onReloadMenuTheme(reloadMenuTheme);
  window.menuAPI.onReloadSoundTheme(reloadSoundTheme);

  // Initialize keyboard layout mapping so quick-select keys work reliably with IMEs.
  KeyMapper.init();

  // Reload the icon themes if requested.
  window.menuAPI.onReloadIconThemes(async () => {
    await IconThemeRegistry.getInstance().init();
  });

  // Initialize the icon theme registry.
  await IconThemeRegistry.getInstance().init();

  // We create the settings button and the menu.
  const settingsButton = new SettingsButton(
    document.getElementById('settings-button') as HTMLButtonElement,
    settings
  );

  const showSettings = () => {
    menu.triggerInteraction(MenuInteractionType.eCloseMenu);
    window.menuAPI.showSettings();
  };

  settingsButton.on('click', () => {
    showSettings();
  });

  const menu = new Menu(
    document.getElementById('kando-menu'),
    menuTheme,
    centerText,
    settings
  );

  // Show the menu when the host requests it. Once the first frame of the menu has been
  // rendered, we report back so that the host can measure the latency.
  window.menuAPI.onShowMenu(async (root, menuOptions) => {
    menu.show(root, menuOptions);
    settingsButton.show();
    requestAnimationFrame(() => requestAnimationFrame(() => window.menuAPI.menuShown()));
  });

  // Sometimes, the host may request to close the menu or a submenu.
  window.menuAPI.onTriggerInteraction((type) => {
    menu.triggerInteraction(type);
  });

  let currentSettings = settings;

  // Report actionable menu interactions to the host process and play interaction sounds.
  // Skip sending hover/open/submenu-nav events over IPC because the host ignores them.
  menu.on('interaction', (type, path, time, source) => {
    soundTheme.playSound(type);
    if (
      type === MenuInteractionType.eCloseMenu ||
      type === MenuInteractionType.eActivateMenu ||
      type === MenuInteractionType.eSelectButton
    ) {
      window.menuAPI.finalizeInteraction(type, path, time, source);
    }

    if (type === MenuInteractionType.eCloseMenu) {
      settingsButton.hide();
      soundTheme.scheduleSuspend();
    }
  });

  // Reload the menu or sound theme only when theme-related settings actually changed.
  window.commonAPI.generalSettings.onChange((newSettings, oldSettings) => {
    currentSettings = newSettings;
    const themeChanged =
      !oldSettings ||
      newSettings.menuTheme !== oldSettings.menuTheme ||
      newSettings.darkMenuTheme !== oldSettings.darkMenuTheme ||
      newSettings.enableDarkModeForMenuThemes !==
        oldSettings.enableDarkModeForMenuThemes ||
      JSON.stringify(newSettings.menuThemeColors) !==
        JSON.stringify(oldSettings.menuThemeColors) ||
      JSON.stringify(newSettings.darkMenuThemeColors) !==
        JSON.stringify(oldSettings.darkMenuThemeColors);

    if (themeChanged) {
      reloadMenuTheme();
    }

    if (!oldSettings || newSettings.soundTheme !== oldSettings.soundTheme) {
      reloadSoundTheme();
    }
    soundTheme.setVolume(newSettings.soundVolume);

    menu.updateSettings(newSettings);
    settingsButton.updateSettings(newSettings);
  });

  // This is used to warp the mouse pointer to a different position.
  menu.on('move-pointer', (dist) => {
    window.menuAPI.movePointer(dist);
  });

  // Hide the menu when the user presses escape. Also, open the settings when the user
  // presses ctrl + comma.
  document.body.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape') {
      menu.triggerInteraction(MenuInteractionType.eCloseMenu);
    }

    if (ev.ctrlKey && ev.key === ',') {
      if (currentSettings.useDefaultOsShowSettingsHotkey) {
        showSettings();
      }
    }
  });

  // This is helpful during development as it shows us when the renderer process has
  // finished reloading.
  window.commonAPI.log("Successfully loaded Kando's Menu process.");

  // Notify the host process that the menu window is ready.
  window.menuAPI.menuWindowReady();
});
