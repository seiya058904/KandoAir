//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import React from 'react';
import i18next from 'i18next';

import * as classes from './WorkflowEditor.module.scss';
import classNames from 'classnames/bind';
const cx = classNames.bind(classes);

import { useAppState, useMenuSettings, getSelectedChild } from '../../state';
import { Workflow } from '../../../common';
import ActionList from './ActionList';

/**
 * Edits what happens when a menu item is selected. In KandoAir, buttons run exactly one
 * action (the menu closes afterwards) and submenus only have a quick-select key. The
 * center-click behavior of the root menu (close) and of submenus (go back) is fixed.
 */
export default function WorkflowEditor() {
  const menus = useMenuSettings((state) => state.menus);
  const selectedMenu = useAppState((state) => state.selectedMenu);
  const selectedChildPath = useAppState((state) => state.selectedChildPath);
  const editMenuItem = useMenuSettings((state) => state.editMenuItem);

  // Get the selected item.
  const selectedItem = getSelectedChild(menus, selectedMenu, selectedChildPath);

  // Root items have no editable workflow.
  if (!selectedItem || selectedItem.type === 'root') {
    return null;
  }

  const isButton = selectedItem.type === 'button';
  const workflow: Workflow | undefined = isButton
    ? selectedItem.selectWorkflow
    : selectedItem.openWorkflow;

  const updateWorkflow = (updated: Workflow) => {
    editMenuItem(selectedMenu, selectedChildPath, (item) => {
      if (item.type === 'button') {
        item.selectWorkflow = updated;
      } else if (item.type === 'submenu') {
        item.openWorkflow = { ...updated, actions: [] };
      }
      return item;
    });
  };

  // Items 1 to 9 can be selected with the number keys by default.
  const itemIndex =
    selectedChildPath.length > 0 ? selectedChildPath[selectedChildPath.length - 1] : -1;
  const quickSelectKeyPlaceholder =
    itemIndex < 9 ? `${itemIndex + 1}` : i18next.t('settings.not-bound');

  return (
    <div className={classes.workflowEditor}>
      <div className={cx({ workflow: true, active: true })}>
        <ActionList
          description={
            isButton
              ? i18next.t('settings.workflow-editor.button-select-workflow.description')
              : ''
          }
          emptyHint={i18next.t(
            'settings.workflow-editor.button-select-workflow.empty-hint'
          )}
          maxActions={isButton ? 1 : 0}
          quickSelectKeyInfo={
            isButton
              ? i18next.t(
                  'settings.workflow-editor.button-select-workflow.quick-select-key-hint'
                )
              : i18next.t(
                  'settings.workflow-editor.submenu-open-workflow.quick-select-key-hint'
                )
          }
          quickSelectKeyPlaceholder={quickSelectKeyPlaceholder}
          workflow={workflow}
          onUpdateItem={(info) => {
            editMenuItem(selectedMenu, selectedChildPath, (item) => {
              if (info.name) {
                item.name = info.name;
              }
              if (info.icon) {
                item.icon = info.icon;
              }
              if (info.iconTheme) {
                item.iconTheme = info.iconTheme;
              }
              return item;
            });
          }}
          onUpdateWorkflow={updateWorkflow}
        />
      </div>
    </div>
  );
}
