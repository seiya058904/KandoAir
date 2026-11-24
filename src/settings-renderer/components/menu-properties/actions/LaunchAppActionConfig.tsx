//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import React from 'react';
import i18next from 'i18next';
import { TbApps } from 'react-icons/tb';

import { Button, FilePicker } from '../../common';
import { LaunchAppAction } from '../../../../common';
import AppPicker from '../AppPicker';

type Props = {
  /** The action to configure. */
  readonly action: LaunchAppAction;

  /** Function to call when the action changes. */
  readonly onUpdateAction: (action: LaunchAppAction) => void;

  /** Function to call when the container menu item should be modified. */
  readonly onUpdateItem: (info: {
    name?: string;
    icon?: string;
    iconTheme?: string;
  }) => void;
};

/**
 * The configuration component for launch-app actions. The app can be chosen from the
 * installed apps (including Store apps) or as a program file.
 */
export function LaunchAppActionConfig(props: Props) {
  const [appPickerVisible, setAppPickerVisible] = React.useState(false);

  return (
    <>
      <FilePicker
        hideDirectoryButton
        extraButton={
          <Button
            isGrouped
            icon={<TbApps />}
            tooltip={i18next.t('menu-actions.execute-command.choose-app')}
            variant="secondary"
            onClick={() => setAppPickerVisible(true)}
          />
        }
        filters={[
          { name: '*.exe, *.lnk', extensions: ['exe', 'lnk', 'bat', 'cmd', 'com'] },
        ]}
        initialValue={props.action.target}
        placeholder={i18next.t('menu-actions.launch-app.placeholder')}
        onChange={(target) => {
          props.onUpdateAction({ ...props.action, target });

          // Name the item after the chosen program file.
          if (!target.toLowerCase().startsWith('shell:')) {
            const parts = target.split(/[/\\]/);
            const name = parts[parts.length - 1].replace(/\.(exe|lnk|bat|cmd|com)$/i, '');
            if (name) {
              props.onUpdateItem({ name });
            }
          }
        }}
      />
      <AppPicker
        isVisible={appPickerVisible}
        onClose={() => setAppPickerVisible(false)}
        onSelect={(app) => {
          props.onUpdateAction({
            ...props.action,
            target: `shell:AppsFolder\\${app.id}`,
          });
          props.onUpdateItem({
            name: app.name,
            icon: app.icon,
            iconTheme: app.iconTheme,
          });
        }}
      />
    </>
  );
}
