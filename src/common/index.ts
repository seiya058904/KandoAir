//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

export * from './settings-schemata';
export * from './typed-event-emitter';
export * from './action-type-registry';
export * from './shortcut';

import { Menu } from './settings-schemata';

/**
 * A simple 2D vector.
 *
 * You can find some vector math in the `src/renderer/math` directory.
 */
export type Vec2 = {
  x: number;
  y: number;
};

/** This type describes some information about the currently used backend. */
export type BackendInfo = {
  /**
   * The name of the backend. This is shown in the user interface so that users can see
   * which backend is currently active.
   */
  readonly name: string;

  /**
   * There are some backends which do not support custom shortcuts. In this case, the user
   * will not be able to change the shortcuts in the settings. Instead, the user will set
   * a shortcut ID and then assign a shortcut in the operating system.
   */
  readonly supportsShortcuts: boolean;

  /**
   * This hint is shown in the settings next to the shortcut-id input field if
   * supportsShortcuts is false. It should very briefly explain how to change the
   * shortcuts in the operating system. If supportsShortcuts is true, this is not
   * required.
   */
  readonly shortcutHint?: string;

  /**
   * This text will be shown in the settings window as a banner at the bottom of the
   * window. It can be used to show some warning to the user, for instance if the backend
   * is not fully supported or if some features are not available. If this is not set, no
   * banner will be shown.
   */
  readonly settingsWindowBanner?: string;
};

/** This type describes some information about the current version of Kando. */
export type VersionInfo = {
  readonly kandoVersion: string;
  readonly chromeVersion: string;
};

/**
 * This type is used to transfer information required from the window manager when opening
 * the pie menu. It contains the name of the currently focused app / window, the current
 * pointer position, and the screen area where a maximized window can be placed. That is
 * the screen resolution minus the taskbar and other panels.
 */
export type WMInfo = {
  readonly windowName: string;
  readonly appName: string;
  readonly pointerX: number;
  readonly pointerY: number;
  readonly workArea: { x: number; y: number; width: number; height: number };
};

/**
 * Options of the file picker. This is a subset of Electron's OpenDialogOptions which is
 * supported by the KandoAir host.
 */
export type OpenDialogOptions = {
  readonly title?: string;
  readonly defaultPath?: string;
  readonly filters?: { name: string; extensions: string[] }[];
  readonly properties?: Array<'openFile' | 'openDirectory'>;
};

/** The persisted widths of the resizable sidebars in the settings window. */
export type SettingsWindowSidebarWidths = {
  readonly left?: number;
  readonly right?: number;
};

/** This type describes a icon theme consisting of a collection of icon files. */
export type FileIconThemeDescription = {
  /**
   * The ID of the theme. This is used to identify the theme in the settings file. It is
   * also the directory name of the icon theme.
   */
  readonly name: string;

  /**
   * The absolute path to the directory where the theme is stored, including the name as
   * the last part of the path.
   */
  readonly directory: string;

  /**
   * A list of all available icons in this theme. These are the filenames of the icons
   * relative to the theme directory. In case of nested directories, the filenames can
   * actually be paths.
   */
  readonly icons: string[];
};

/**
 * This type is used to pass information about all available icon themes to the renderer
 * process.
 */
export type IconThemesInfo = {
  /** The absolute path to the directory where the user may store custom icon themes. */
  readonly userIconDirectory: string;

  /** All available file icon themes. */
  readonly fileIconThemes: FileIconThemeDescription[];
};

/**
 * This type is used to describe an element of a key sequence. It contains the DOM name of
 * the key, a boolean indicating whether the key is pressed or released and a delay in
 * milliseconds.
 */
export type KeyStroke = {
  name: string;
  down: boolean;
  delay: number;
};

/**
 * This type is used to describe a sequence of key strokes. It is used to simulate
 * keyboard shortcuts.
 */
export type KeySequence = Array<KeyStroke>;

/** Enum for the different things a user can do in a menu. */
export enum MenuInteractionType {
  eOpenMenu = 'openMenu',
  eOpenSubmenu = 'openSubmenu',
  eCloseMenu = 'closeMenu',
  eCloseSubmenu = 'closeSubmenu',
  eHoverParent = 'hoverParent',
  eHoverCenter = 'hoverCenter',
  eHoverButton = 'hoverButton',
  eHoverSubmenu = 'hoverSubmenu',
  eSelectButton = 'selectButton',
  eActivateMenu = 'activateMenu',
}

/**
 * There are different reasons why a menu should be shown. This type is used to describe
 * the request to show a menu. A menu can be shown because a shortcut was pressed (in this
 * case `trigger` will be the shortcut or the shortcut ID), because a menu was requested
 * by name, or a custom menu is requested (in this case `menu` contains the menu
 * structure).
 */
export type ShowMenuRequest = {
  readonly trigger?: string;
  readonly name?: string;
  readonly menu?: Menu;
};

/**
 * This type is used to describe the additional information that is passed to the Menu's
 * `show()` method from the main to the renderer process.
 */
export type ShowMenuOptions = {
  /**
   * The position of the mouse cursor when the menu was opened. Relative to the top left
   * corner of the window.
   */
  readonly mousePosition: Vec2;

  /**
   * The size of the window. Usually, this is the same as window.innerWidth and
   * window.innerHeight. However, when the window was just resized, this can be different.
   * Therefore, we need to pass it from the main process.
   */
  readonly windowSize: Vec2;

  /**
   * The scale factor of the menu. This is required to compute the correct position of the
   * menu.
   */
  readonly zoomFactor: number;

  /**
   * If true, the menu will open at a fixed position determined by
   * {@link fixedMenuPosition}.
   */
  useFixedPosition: boolean;

  /**
   * Position at which the menu will open if {@link useFixedPosition} is true. Value should
   * be between 0-1 for both x and y.
   */
  readonly fixedMenuPosition: Vec2;

  /**
   * If this is set, the menu will be "anchored". This means that any submenus will be
   * opened at the same position as the parent menu.
   */
  readonly anchoredMode: boolean;

  /**
   * If this is set, the menu will be in "hover mode". This means that the menu items can
   * be selected by only hovering over them.
   */
  readonly hoverMode: boolean;

  /**
   * If this is set, the system-icon theme has changed since the last time the menu was
   * opened. This is used to determine if the menu needs to be reloaded.
   */
  readonly systemIconsChanged: boolean;
};

/**
 * The description of a menu theme. These are the properties which can be defined in the
 * JSON file of a menu theme.
 */
export type MenuThemeDescription = {
  /**
   * The ID of the theme. This is used to identify the theme in the settings file. It is
   * also the directory name of the theme and is set by Kando when loading the theme.json
   * file. So the path to the theme.json file is this.directory/this.id/theme.json.
   */
  id: string;

  /**
   * The absolute path to the directory where the theme is stored. This is set by Kando
   * when loading the theme.json file.
   */
  directory: string;

  /** A human readable name of the theme. */
  readonly name: string;

  /** The author of the theme. */
  readonly author: string;

  /** The version of the theme. Should be a semantic version string like "1.0.0". */
  readonly themeVersion: string;

  /** The version of the Kando theme engine this theme is compatible with. */
  readonly engineVersion: number;

  /** The license of the theme. For instance "CC-BY-4.0". */
  readonly license: string;

  /**
   * The maximum radius in pixels of a menu when using this theme. This is used to move
   * the menu away from the screen edges when it's opened too close to them. Default is
   * 150px.
   */
  readonly maxMenuRadius: number;

  /** The width of the text wrap in the center of the menu in pixels. Default is 90px. */
  readonly centerTextWrapWidth: number;

  /**
   * If this is true, children of a menu item will be drawn below the parent. Otherwise
   * they will be drawn above. Default is true.
   */
  readonly drawChildrenBelow: boolean;

  /**
   * If this is set to true, the center text of the menu will be drawn. This is the text
   * that is displayed in the center of the menu when it is opened. Default is true.
   */
  readonly drawCenterText: boolean;

  /**
   * If this is set to true, a full-screen div will be drawn below the menu with the CSS
   * class "selection-wedges". If any menu item is hovered, it will also receive the class
   * "hovered" and the "--start-angle" and "--end-angle" CSS properties will indicate
   * where the selected child is. Default is false.
   */
  readonly drawSelectionWedges: boolean;

  /**
   * If this is set to true, a full-screen div will be drawn below the menu with the CSS
   * class "wedge-separators". It will contain a div for each separator line between
   * adjacent wedges. They will have the "separator" class. Default is false.
   */
  readonly drawWedgeSeparators: boolean;

  /**
   * These colors will be available as var(--name) in the CSS file and can be adjusted by
   * the user in the settings. The map assigns a default CSS color to each name.
   */
  readonly colors: Record<string, string>;

  /**
   * The layers which are drawn on top of each other for each menu item. Each layer will
   * be a html div element with a class defined in the theme file. Also, each layer can
   * have a `content` property which can be used to make the layer contain the item's icon
   * or name.
   */
  readonly layers: {
    readonly class: string;
    readonly content: 'none' | 'name' | 'icon' | 'quick-select-key';
  }[];

  /**
   * True if the theme.json(5) file could not be read or parsed. A broken theme is
   * excluded from the list of available themes in the settings.
   */
  readonly loadFailed?: boolean;
};

/** The description of a single sound in a sound theme. */
export type SoundDescription = {
  readonly file: string;
  readonly volume?: number;
  readonly minPitch?: number;
  readonly maxPitch?: number;
};

/** The description of a sound theme. */
export type SoundThemeDescription = {
  id: string;
  directory: string;
  readonly name: string;
  readonly author: string;
  readonly themeVersion: string;
  readonly engineVersion: number;
  readonly license: string;
  readonly sounds: Partial<Record<MenuInteractionType, SoundDescription>>;
  readonly loadFailed?: boolean;
};

/**
 * This type is used to describe an installed application. When the settings window is
 * opened, it will query the host process for a list of all installed applications.
 */
export type AppDescription = {
  /**
   * Some unique identifier for the application. What that is depends on the backend.
   * Could be for instance the UWP app ID. If the backend is not able to provide a unique
   * ID, it may fall back to using the application command.
   */
  readonly id: string;

  /** The name of the application. */
  readonly name: string;

  /** The command to launch the application. */
  readonly command: string;

  /** The icon used for the application. */
  readonly icon: string;

  /** The icon theme used for the above icon. */
  readonly iconTheme: string;
};

/**
 * This type is used to describe an open window. It is used when listing the open windows,
 * for instance for the focus-window action.
 */
export type WindowDescription = {
  /** The title of the window. */
  readonly windowName: string;

  /** The application the window belongs to. */
  readonly appName: string;
};

/**
 * For achievement tracking, it is important to know the source of a selection. This enum
 * is used to indicate whether a selection was made using the mouse, a gesture (marking
 * mode or turbo mode).
 */
export enum SelectionSource {
  eUnknown = 'unknown',
  eClick = 'click',
  eKeyboard = 'keyboard',
  eGesture = 'gesture',
}
