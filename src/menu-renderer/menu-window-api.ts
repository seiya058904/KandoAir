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
  Vec2,
  ShowMenuOptions,
  SelectionSource,
  MenuInteractionType,
  RootMenuItem,
} from '../common';

/**
 * These functions are available in the menu window's renderer process. They are available
 * on the window object as `window.menuAPI.<method>`. In addition, the common window API
 * functions are also be available as `window.commonAPI.<method>`.
 */
export const MENU_WINDOW_API = {
  /**
   * This will be called by the renderer process when it is fully loaded. This is used to
   * notify the host process that the renderer process is ready to receive messages.
   */
  menuWindowReady: () => {
    hostBridge.send('menu-window.ready');
  },

  /**
   * This is called once the first frame of a newly shown menu has been rendered. The host
   * uses this to measure the latency between the shortcut and the visible menu.
   */
  menuShown: () => {
    hostBridge.send('menu-window.shown');
  },

  /**
   * This can be used to warp the mouse pointer to a different position.
   *
   * @param dist The distance to move the mouse pointer.
   */
  movePointer: (dist: Vec2) => {
    hostBridge.send('menu-window.move-pointer', dist);
  },

  /**
   * This will be triggered by the host process when a new menu should be shown.
   *
   * @param callback This callback will be called when a new menu should be shown.
   */
  onShowMenu: (func: (root: RootMenuItem, menuOptions: ShowMenuOptions) => void) => {
    hostBridge.on('menu-window.show-menu', (root, menuOptions) =>
      func(root, menuOptions)
    );
  },

  /**
   * The host can request the renderer process to initiate several menu interactions, such
   * as closing the menu or closing the current submenu. The renderer process will act
   * accordingly and call the finalizeInteraction callback afterwards.
   *
   * @param callback This callback will be called when the host process requests a menu
   *   interaction.
   */
  onTriggerInteraction: (
    func: (
      type: MenuInteractionType.eCloseMenu | MenuInteractionType.eCloseSubmenu
    ) => void
  ) => {
    hostBridge.on('menu-window.trigger-interaction', (type) => func(type));
  },

  /**
   * Menu interactions can be triggered by the user in the renderer process, for example
   * by clicking a menu item or hovering over a submenu. They can also be triggered by the
   * host process using the onTriggerInteraction function. In both cases, the renderer
   * process will call this finalizeInteraction function to notify the host process about
   * the interaction. The host process will then execute the corresponding action.
   *
   * @param type The type of the interaction, e.g. opening a menu, hovering a button, etc.
   * @param path The path of the selected menu item.
   * @param time The time it took to select the item in milliseconds.
   * @param source The source used to make the selection.
   */
  finalizeInteraction: (
    type: MenuInteractionType,
    path: number[],
    time: number,
    source: SelectionSource
  ) => {
    hostBridge.send('menu-window.finalize-interaction', type, path, time, source);
  },

  /** This will be called by the render process to show the settings window. */
  showSettings: () => {
    hostBridge.send('menu-window.show-settings');
  },

  /**
   * This will be called by the host process when the icon themes should be reloaded.
   *
   * @param callback This callback will be called when the icon themes should be reloaded.
   */
  onReloadIconThemes: (func: () => void) => {
    hostBridge.on('menu-window.reload-icon-themes', func);
  },

  /**
   * This will be called by the host process when the menu theme should be reloaded.
   *
   * @param callback This callback will be called when the menu theme should be reloaded.
   */
  onReloadMenuTheme: (func: () => void) => {
    hostBridge.on('menu-window.reload-menu-theme', func);
  },

  /**
   * This will be called by the host process when the sound theme should be reloaded.
   *
   * @param callback This callback will be called when the sound theme should be reloaded.
   */
  onReloadSoundTheme: (func: () => void) => {
    hostBridge.on('menu-window.reload-sound-theme', func);
  },
};

/**
 * This type extends the global window object with the `api` objects. These `api` objects
 * contain all functions that can be called by the renderer process.
 */
export type WindowWithAPIs = {
  readonly commonAPI: typeof COMMON_WINDOW_API;
  readonly menuAPI: typeof MENU_WINDOW_API;
} & Window;
