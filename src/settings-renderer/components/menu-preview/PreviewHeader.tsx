//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

import { WindowWithAPIs } from '../../settings-window-api';
declare const window: WindowWithAPIs;

import React from 'react';
import i18next from 'i18next';
import {
  TbSettingsFilled,
  TbInfoSquareRoundedFilled,
  TbPaletteFilled,
  TbPlayerPlayFilled,
} from 'react-icons/tb';
import { IoArrowUndo, IoArrowRedo, IoSchool } from 'react-icons/io5';

import { useAppState, useMenuSettings } from '../../state';
import { Headerbar, Button } from '../common';

/** Undo/Redo buttons hoisted out of PreviewHeader so they are not remounted every render. */
// eslint-disable-next-line @typescript-eslint/naming-convention
const UndoRedoButtons = React.memo(function UndoRedoButtons() {
  const { futureStates, pastStates } = useMenuSettings.temporal.getState();
  // Subscribe to any menu change to update buttons.
  useMenuSettings();

  return (
    <>
      <Button
        isGrouped
        icon={<IoArrowUndo />}
        isDisabled={pastStates.length === 0}
        tooltip={i18next.t('settings.undo')}
        variant="tool"
        onClick={() => useMenuSettings.temporal.getState().undo()}
      />
      <Button
        isGrouped
        icon={<IoArrowRedo />}
        isDisabled={futureStates.length === 0}
        tooltip={i18next.t('settings.redo')}
        variant="tool"
        onClick={() => useMenuSettings.temporal.getState().redo()}
      />
    </>
  );
});

/**
 * This is the toolbar at the top of the menu-preview area. It contains some buttons for
 * undo/redo, and for opening the settings dialogs.
 */
export default function PreviewHeader() {
  const selectedMenu = useAppState((state) => state.selectedMenu);
  const menuCount = useMenuSettings((state) => state.menus.length);
  const setAboutDialogVisible = useAppState((state) => state.setAboutDialogVisible);
  const setIntroDialogVisible = useAppState((state) => state.setIntroDialogVisible);
  const setThemesDialogVisible = useAppState((state) => state.setThemesDialogVisible);
  const setSettingsDialogVisible = useAppState((state) => state.setSettingsDialogVisible);

  const canTestMenu = selectedMenu >= 0 && selectedMenu < menuCount;

  return (
    <Headerbar
      center={
        <>
          <span style={{ marginRight: '8px' }}>
            <Button
              isGrouped
              icon={<IoSchool />}
              tooltip={i18next.t('settings.introduction-dialog.title')}
              variant="tool"
              onClick={() => setIntroDialogVisible(true)}
            />
            <Button
              isGrouped
              icon={<TbInfoSquareRoundedFilled />}
              tooltip={i18next.t('settings.about-dialog.title')}
              variant="tool"
              onClick={() => setAboutDialogVisible(true)}
            />
          </span>
          <span style={{ marginRight: '8px' }}>
            <Button
              isGrouped
              icon={<TbPaletteFilled />}
              tooltip={i18next.t('settings.menu-themes-dialog.title')}
              variant="tool"
              onClick={() => setThemesDialogVisible(true)}
            />
            <Button
              isGrouped
              icon={<TbSettingsFilled />}
              tooltip={i18next.t('settings.general-settings-dialog.title')}
              variant="tool"
              onClick={() => setSettingsDialogVisible(true)}
            />
          </span>
          <span style={{ marginRight: '8px' }}>
            <Button
              icon={<TbPlayerPlayFilled />}
              isDisabled={!canTestMenu}
              tooltip={i18next.t('settings.test-menu')}
              variant="tool"
              onClick={() => window.settingsAPI.openMenu(selectedMenu)}
            />
          </span>
          <span>
            <UndoRedoButtons />
          </span>
        </>
      }
    />
  );
}
