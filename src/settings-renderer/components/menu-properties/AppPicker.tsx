//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

import React from 'react';
import i18next from 'i18next';
import { FixedSizeList, ListChildComponentProps } from 'react-window';
import { TbCheck, TbX, TbBackspaceFilled } from 'react-icons/tb';

import { Modal, Button, ThemedIcon, Note, Swirl } from '../common';
import { AppDescription } from '../../../common';
import { useAppState } from '../../state';

import * as classes from './AppPicker.module.scss';

type RowData = {
  apps: AppDescription[];
  selectedId?: string;
  onSelect: (app: AppDescription) => void;
  onConfirm: (app: AppDescription) => void;
};

// eslint-disable-next-line @typescript-eslint/naming-convention
const AppRow = React.memo(function AppRow({
  index,
  style,
  data,
}: ListChildComponentProps<RowData>) {
  const app = data.apps[index];
  return (
    <div style={style} onDoubleClick={() => data.onConfirm(app)}>
      <Button
        isBlock
        align="left"
        icon={<ThemedIcon name={app.icon} size={24} theme={app.iconTheme} />}
        label={app.name}
        variant={app.id === data.selectedId ? 'secondary' : 'invisible'}
        onClick={() => data.onSelect(app)}
      />
    </div>
  );
});

type Props = {
  /** Function to call when a new window is selected. */
  readonly onSelect: (value: AppDescription) => void;

  /** Function to call when the dialog should be closed. */
  readonly onClose: () => void;

  /** Visibility of the modal. */
  readonly isVisible: boolean;
};

/**
 * This component allows the user to select an application from a list of all installed
 * applications. Uses a virtualized list so hundreds of installed apps do not trigger
 * simultaneous icon fetches on open.
 */
export default function AppPicker(props: Props) {
  const [value, setValue] = React.useState<AppDescription | null>(null);
  const [filterTerm, setFilterTerm] = React.useState('');
  const searchInputRef = React.useRef<HTMLInputElement>(null);
  const installedApps = useAppState((state) => state.installedApps);
  const { onSelect, onClose, isVisible } = props;

  // Filter the installed apps based on the search term.
  const filteredApps = React.useMemo(() => {
    const query = filterTerm.toLowerCase();
    return installedApps.filter(
      (app) => app.name && app.name.toLowerCase().includes(query)
    );
  }, [installedApps, filterTerm]);

  const confirmApp = React.useCallback(
    (app: AppDescription) => {
      onSelect(app);
      onClose();
    },
    [onSelect, onClose]
  );

  const itemData = React.useMemo<RowData>(
    () => ({
      apps: filteredApps,
      selectedId: value?.id,
      onSelect: setValue,
      onConfirm: confirmApp,
    }),
    [filteredApps, value?.id, confirmApp]
  );

  // Reset state and auto-focus search when the modal is shown.
  React.useEffect(() => {
    if (isVisible) {
      setValue(null);
      setFilterTerm('');
      setTimeout(() => searchInputRef.current?.focus(), 0);
    }
  }, [isVisible]);

  return (
    <Modal
      isVisible={props.isVisible}
      maxWidth={450}
      paddingTop={15}
      onClose={props.onClose}>
      <div className={classes.container}>
        <Note isCentered useMarkdown marginLeft="10%" marginRight="10%">
          {i18next.t('settings.app-picker.hint')}
        </Note>

        <Swirl marginBottom={10} variant="2" width={350} />
        <div className={classes.searchInput}>
          <input
            ref={searchInputRef}
            placeholder={i18next.t('settings.app-picker.search-placeholder')}
            type="text"
            value={filterTerm}
            onChange={(event) => {
              setFilterTerm(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                const chosen =
                  value ?? (filteredApps.length === 1 ? filteredApps[0] : null);
                if (chosen) {
                  confirmApp(chosen);
                }
              }
            }}
          />
          <Button
            isGrouped
            icon={<TbBackspaceFilled />}
            onClick={() => {
              setFilterTerm('');
              searchInputRef.current?.focus();
            }}
          />
        </div>
        <div className={classes.listWrapper}>
          {props.isVisible ? (
            <FixedSizeList
              height={320}
              itemCount={filteredApps.length}
              itemData={itemData}
              itemKey={(index, data) => data.apps[index].id}
              itemSize={38}
              width="100%">
              {AppRow}
            </FixedSizeList>
          ) : null}
        </div>
        <div className={classes.buttons}>
          <Button
            isBlock
            icon={<TbX />}
            label={i18next.t('settings.cancel')}
            onClick={() => {
              props.onClose();
            }}
          />
          <Button
            isBlock
            icon={<TbCheck />}
            isDisabled={!value}
            label={i18next.t('settings.app-picker.use-selected')}
            variant="primary"
            onClick={() => {
              props.onSelect(value);
              props.onClose();
            }}
          />
        </div>
      </div>
    </Modal>
  );
}
