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
import { TbBackspaceFilled } from 'react-icons/tb';

import { Button, Modal, Scrollbox, ThemedIcon } from '../common';
import * as classes from './ActionPicker.module.scss';
import { WorkflowAction, WorkflowActionType } from '../../../common';
import { ActionTypeRegistry, ACTION_TYPES } from '../../../common/action-type-registry';

type Props = {
  /** Whether the action picker modal is visible. */
  readonly isVisible: boolean;

  /** Callback to close the modal. */
  readonly onClose: () => void;

  /** Callback when an action is selected. */
  readonly onSelect: (action: WorkflowAction) => void;
};

/**
 * This component displays a modal dialog for selecting a new action to add to a workflow.
 * It shows all available action types with their icons and descriptions.
 */
export default function ActionPicker(props: Props) {
  const registry = ActionTypeRegistry.getInstance();
  const [filterTerm, setFilterTerm] = React.useState('');
  const searchInputRef = React.useRef<HTMLInputElement>(null);

  const allMetadata = registry.getAllMetadata();

  // The action types are listed in the order of ACTION_TYPES.
  const query = filterTerm.toLowerCase();
  const actionTypes = ACTION_TYPES.map(
    (actionType) => [actionType, allMetadata[actionType]] as const
  ).filter(
    ([, metadata]) =>
      metadata.name.toLowerCase().includes(query) ||
      metadata.description.toLowerCase().includes(query)
  );

  React.useEffect(() => {
    if (props.isVisible) {
      setFilterTerm('');
      setTimeout(() => searchInputRef.current?.focus(), 0);
    }
  }, [props.isVisible]);

  const handleSelectActionType = (actionType: WorkflowActionType) => {
    const metadata = registry.getMetadata(actionType);
    props.onSelect(metadata.createAction());
  };

  return (
    <Modal
      isVisible={props.isVisible}
      maxWidth={500}
      paddingBottom={5}
      paddingTop={0}
      title={i18next.t('settings.workflow-editor.add-action')}
      onClose={props.onClose}>
      <div className={classes.container}>
        <div className={classes.searchInput}>
          <input
            ref={searchInputRef}
            placeholder={i18next.t(
              'settings.workflow-editor.action-picker.search-placeholder'
            )}
            type="text"
            value={filterTerm}
            onChange={(event) => {
              setFilterTerm(event.target.value);
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
        <Scrollbox paddingLeft={0} width="100%">
          <div className={classes.actionTypeList}>
            {actionTypes.map(([actionType, metadata]) => (
              <button
                key={actionType}
                className={classes.actionTypeItem}
                type="button"
                onClick={() => {
                  handleSelectActionType(actionType);
                  props.onClose();
                }}>
                <div className={classes.actionTypeIcon}>
                  <ThemedIcon name={metadata.icon} theme={metadata.iconTheme} />
                </div>

                <div>
                  <div className={classes.actionTypeName}>{metadata.name}</div>
                  <div className={classes.actionTypeDetails}>{metadata.description}</div>
                </div>
              </button>
            ))}
          </div>
        </Scrollbox>
      </div>
    </Modal>
  );
}
