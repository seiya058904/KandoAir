//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

export type { GeneralSettingsV1 as GeneralSettings } from './general-settings-v1';
export { GENERAL_SETTINGS_SCHEMA_V1 as GENERAL_SETTINGS_SCHEMA } from './general-settings-v1';

export type {
  LaunchAppActionV2 as LaunchAppAction,
  ExecuteCommandActionV2 as ExecuteCommandAction,
  ExecuteMacroActionV2 as ExecuteMacroAction,
  FocusWindowActionV2 as FocusWindowAction,
  MacroEventV2 as MacroEvent,
  OpenFileActionV2 as OpenFileAction,
  OpenMenuActionV2 as OpenMenuAction,
  OpenURIActionV2 as OpenURIAction,
  SetClipboardActionV2 as SetClipboardAction,
  SimulateHotkeyActionV2 as SimulateHotkeyAction,
  MenuConditionsV2 as MenuConditions,
  MenuItemV2 as MenuItem,
  MenuItemTypeV2 as MenuItemType,
  RootMenuItemV2 as RootMenuItem,
  ChildMenuItemV2 as ChildMenuItem,
  SubmenuMenuItemV2 as SubmenuMenuItem,
  ButtonMenuItemV2 as ButtonMenuItem,
  MenuV2 as Menu,
  MenuCollectionV2 as MenuCollection,
  MenuSettingsV2 as MenuSettings,
  WorkflowActionV2 as WorkflowAction,
  WorkflowActionTypeV2 as WorkflowActionType,
  WorkflowV2 as Workflow,
} from './menu-settings-v2';
export {
  MENU_SETTINGS_SCHEMA_V2 as MENU_SETTINGS_SCHEMA,
  MENU_ITEM_SCHEMA_V2 as MENU_ITEM_SCHEMA,
  MENU_SCHEMA_V2 as MENU_SCHEMA,
  ROOT_MENU_ITEM_SCHEMA_V2 as ROOT_MENU_ITEM_SCHEMA,
} from './menu-settings-v2';

export type { ExportedMenuV2 as ExportedMenu } from './exported-menu-v2';
export { EXPORTED_MENU_SCHEMA_V2 as EXPORTED_MENU_SCHEMA } from './exported-menu-v2';
