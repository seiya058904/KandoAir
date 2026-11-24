//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import i18next from 'i18next';

import type { WindowWithAPIs } from './common-window-api';
import { BackendInfo } from '../common';
import {
  ChildMenuItem,
  WorkflowActionType,
  SubmenuMenuItem,
  ButtonMenuItem,
  WorkflowAction,
} from '.';

// This file is imported by both the menu renderer and the settings renderer. This
// function is used to check if the common API to call host methods from the renderer is
// available in the current window.
function hasCommonAPI(currentWindow: unknown): currentWindow is WindowWithAPIs {
  return (
    typeof currentWindow === 'object' &&
    currentWindow !== null &&
    'commonAPI' in currentWindow
  );
}

/**
 * Meta information about an action type. This includes the translated name, a description
 * and an icon.
 */
export type WorkflowActionTypeMeta = {
  /** The translated name of the action type. */
  name: string;

  /** The icon representing the action type. */
  icon: string;

  /** The theme of the icon. */
  iconTheme: string;

  /** The translated description of the action type. */
  description: string;

  /** A function that creates an action of this type with default parameters. */
  createAction: () => WorkflowAction;
};

/** The nine action types of KandoAir, in the order in which they are offered. */
export const ACTION_TYPES: WorkflowActionType[] = [
  'launch-app',
  'open-file',
  'open-uri',
  'simulate-hotkey',
  'execute-command',
  'set-clipboard',
  'execute-macro',
  'focus-window',
  'open-menu',
];

/**
 * This singleton class is responsible for managing the action types of menu items. It
 * provides metadata for each action type, such as the name, description and icon.
 *
 * It can also be used to create new menu items for some dropped data types. For example,
 * if a file is dropped on the menu, we can create a new menu item with the "open file"
 * action and the corresponding file path as parameter.
 *
 * In KandoAir, every button runs exactly one action and the menu closes afterwards.
 */
export class ActionTypeRegistry {
  /** The singleton instance of this class. */
  private static instance?: ActionTypeRegistry;

  /** The metadata for each action type. */
  private metadata?: Record<WorkflowActionType, WorkflowActionTypeMeta>;

  /**
   * The constructor is private to enforce the singleton pattern. Use `getInstance` to get
   * the instance of this class.
   */
  private constructor() {}

  /**
   * Initializes the metadata. This has to be called after i18next has been initialized.
   *
   * @param backend Information about the backend (unused, kept for API compatibility).
   */
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  public static init(backend?: BackendInfo) {
    this.getInstance().metadata = {
      ['launch-app']: {
        name: i18next.t('menu-actions.launch-app.name'),
        icon: 'rocket_launch',
        iconTheme: 'material-symbols-rounded',
        description: i18next.t('menu-actions.launch-app.description'),
        createAction: () => ({ type: 'launch-app', target: '' }),
      },
      ['open-file']: {
        name: i18next.t('menu-actions.open-file.name'),
        icon: 'file-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.open-file.description'),
        createAction: () => ({ type: 'open-file', path: '' }),
      },
      ['open-uri']: {
        name: i18next.t('menu-actions.open-uri.name'),
        icon: 'uri-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.open-uri.description'),
        createAction: () => ({ type: 'open-uri', uri: '' }),
      },
      ['simulate-hotkey']: {
        name: i18next.t('menu-actions.simulate-hotkey.name'),
        icon: 'hotkey-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.simulate-hotkey.description-auto-close'),
        createAction: () => ({ type: 'simulate-hotkey', hotkey: '' }),
      },
      ['execute-command']: {
        name: i18next.t('menu-actions.execute-command.name'),
        icon: 'command-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.execute-command.description'),
        createAction: () => ({
          type: 'execute-command',
          command: '',
          detached: true,
        }),
      },
      ['set-clipboard']: {
        name: i18next.t('menu-actions.set-clipboard.name'),
        icon: 'clipboard-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.set-clipboard.description'),
        createAction: () => ({
          type: 'set-clipboard',
          text: '',
        }),
      },
      ['execute-macro']: {
        name: i18next.t('menu-actions.execute-macro.name'),
        icon: 'macro-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.execute-macro.description'),
        createAction: () => ({
          type: 'execute-macro',
          macro: [],
        }),
      },
      ['focus-window']: {
        name: i18next.t('menu-actions.focus-window.name'),
        icon: 'focus-window-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.focus-window.description'),
        createAction: () => ({
          type: 'focus-window',
          appName: '',
          windowName: '',
        }),
      },
      ['open-menu']: {
        name: i18next.t('menu-actions.open-menu.name'),
        icon: 'redirect-item.svg',
        iconTheme: 'kando',
        description: i18next.t('menu-actions.open-menu.description'),
        createAction: () => ({
          type: 'open-menu',
          menu: '',
        }),
      },
    };
  }

  /**
   * Use this method to get the singleton instance of this class.
   *
   * @returns The singleton instance of this class.
   */
  public static getInstance(): ActionTypeRegistry {
    if (!ActionTypeRegistry.instance) {
      ActionTypeRegistry.instance = new ActionTypeRegistry();
    }
    return ActionTypeRegistry.instance;
  }

  /**
   * This method returns the meta information for all action types.
   *
   * @returns A map of all action types to their meta information.
   */
  public getAllMetadata(): Record<WorkflowActionType, WorkflowActionTypeMeta> {
    return this.metadata!;
  }

  /**
   * This method returns the meta information for a given action type.
   *
   * @param type The action type for which the meta information should be returned.
   * @returns The meta information for the given action type.
   */
  public getMetadata(type: WorkflowActionType): WorkflowActionTypeMeta {
    return this.metadata![type];
  }

  /**
   * This is used during drag-and-drop operations: When some data is dragged into the
   * settings menu, we try to create a corresponding action. Usually, the drag source
   * offers the data in a variety of formats. Given a list of formats, this method returns
   * true if one of the formats is supported.
   *
   * @param transfer The transferred data.
   * @returns True if the drag source offers a supported data type.
   */
  public hasSupportedDataType(transfer: DataTransfer): boolean {
    const supportedTypes = [
      'kando/new-item-type', // This is used for new items dragged from the item type list.
      'Files',
      'text/uri-list',
      'text/plain',
    ];

    for (const type of supportedTypes) {
      if (transfer.types.includes(type)) {
        return true;
      }
    }

    return false;
  }

  /**
   * Creates a new child menu item for a known item type ('submenu' or an action type).
   *
   * @param typeName The item type identifier.
   * @returns A new child menu item, or null if the type is unknown.
   */
  public createItemByType(typeName: string): ChildMenuItem | null {
    if (typeName === 'submenu') {
      const newItem: SubmenuMenuItem = {
        type: 'submenu',
        name: i18next.t('menu-items.submenu.name'),
        icon: 'submenu-item.svg',
        iconTheme: 'kando',
        children: [],
        activateWorkflow: {
          quickSelectKey: 'Backspace',
          actions: [],
        },
      };
      return newItem;
    }

    const actionType = this.getMetadata(typeName as WorkflowActionType);
    if (!actionType) {
      console.warn(`No action type found for type name ${typeName}`);
      return null;
    }

    const newItem: ButtonMenuItem = {
      type: 'button',
      name: actionType.name,
      icon: actionType.icon,
      iconTheme: actionType.iconTheme,
      selectWorkflow: { actions: [actionType.createAction()] },
    };
    return newItem;
  }

  /**
   * Given some dropped data, this method returns a new menu item fitting to the data
   * type. The data type is a mime type such as 'text/plain' or a Kando-specific type such
   * as 'kando/new-item-type'. The data itself is a string that contains the data in the
   * corresponding format.
   *
   * @param transfer The transferred data.
   * @returns A new menu item with a single action fitting to the data type. If no item
   *   could be created, null is returned.
   */
  public async createItem(transfer: DataTransfer): Promise<ChildMenuItem | null> {
    // We collect all potential data formats first. We have to do this, because some of
    // the following code is asynchronous and the DataTransfer object becomes invalid
    // after an await.
    const data = new Map<string, string | File>();
    for (const type of transfer.types) {
      if (type === 'Files') {
        data.set(type, transfer.files[0]);
      } else if (
        type === 'text/plain' ||
        type === 'text/uri-list' ||
        type === 'kando/new-item-type'
      ) {
        data.set(type, transfer.getData(type));
      }
    }

    // This is used when a new item is dragged from the item type list in the footer. The
    // data will contain the type of the item to be created. See the FooterButton
    // component for more details.
    if (data.has('kando/new-item-type')) {
      return this.createItemByType(data.get('kando/new-item-type') as string);
    }

    // Creating an item for a file may fail. If it does, we try another data type below.
    // This is only called from the settings renderer, so we use the common API to create
    // the item.
    if (data.has('Files') && typeof window !== 'undefined' && hasCommonAPI(window)) {
      const item = await window.commonAPI.createItemForDroppedFile(
        data.get('Files') as File
      );
      if (item && item.type !== 'root') {
        return item;
      }
    }

    // If the dropped data is a URI list, we create an "open URI" action.
    if (data.has('text/uri-list')) {
      return this.createUriItem((data.get('text/uri-list') as string).split(/\r?\n/)[0]);
    }

    // Plain text is used for links and paths dragged from other applications.
    if (data.has('text/plain')) {
      const text = (data.get('text/plain') as string).trim();
      if (/^[a-z][a-z0-9+.-]*:\/\//i.test(text) || text.startsWith('mailto:')) {
        return this.createUriItem(text);
      }
      if (/^([a-z]:\\|\\\\|%[a-z_]+%)/i.test(text)) {
        const fileType = this.getMetadata('open-file');
        return {
          type: 'button',
          name: text.split('\\').filter(Boolean).pop() || fileType.name,
          icon: fileType.icon,
          iconTheme: fileType.iconTheme,
          selectWorkflow: { actions: [{ type: 'open-file', path: text }] },
        };
      }
    }

    return null;
  }

  private createUriItem(uri: string): ButtonMenuItem {
    const uriType = this.getMetadata('open-uri');
    return {
      type: 'button',
      name: uriType.name,
      icon: uriType.icon,
      iconTheme: uriType.iconTheme,
      selectWorkflow: { actions: [{ type: 'open-uri', uri }] },
    };
  }
}
