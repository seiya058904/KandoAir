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
import { useAutoAnimate } from '@formkit/auto-animate/react';
import i18next from 'i18next';

import * as classes from './ActionList.module.scss';

import { ShortcutPicker, Button, Note, ThemedIcon } from '../common';
import { TbPlus, TbTrash } from 'react-icons/tb';
import { Workflow, WorkflowAction, ActionTypeRegistry } from '../../../common';
import ActionPicker from './ActionPicker';
import { getConfigComponent } from './actions';

type Props = {
  /** A description shown below the quick-select key. */
  readonly description: string;

  /** The workflow to edit. */
  readonly workflow?: Workflow;

  /** The maximum number of actions. KandoAir uses one action for buttons. */
  readonly maxActions: number;

  /** A hint which is shown if the workflow has no actions. */
  readonly emptyHint: string;

  /** The placeholder of the quick-select key picker. */
  readonly quickSelectKeyPlaceholder: string;

  /** The info text of the quick-select key picker. */
  readonly quickSelectKeyInfo: string;

  /** Called when the workflow is modified. */
  readonly onUpdateWorkflow: (workflow: Workflow) => void;

  /** Called when an action wants to modify the containing menu item. */
  readonly onUpdateItem: (info: {
    name?: string;
    icon?: string;
    iconTheme?: string;
  }) => void;
};

/**
 * Shows the quick-select key of a workflow and its action. Selecting an item runs the
 * action and closes the menu.
 */
export default function ActionList(props: Props) {
  const [animatedList] = useAutoAnimate({ duration: 200 });
  const [actionPickerVisible, setActionPickerVisible] = React.useState(false);

  const actions = (props.workflow?.actions || []).slice(0, props.maxActions);

  const updateActions = (newActions: WorkflowAction[]) => {
    props.onUpdateWorkflow({
      ...props.workflow,
      actions: newActions,
    });
  };

  const renderAction = (action: WorkflowAction, index: number) => {
    const typeMeta = ActionTypeRegistry.getInstance().getMetadata(action.type);
    if (!typeMeta) {
      return null;
    }

    const configWidget = getConfigComponent(
      action,
      (updatedAction) => {
        const newActions = [...actions];
        newActions[index] = updatedAction;
        updateActions(newActions);
      },
      props.onUpdateItem
    );

    return (
      <div key={`action-${action.type}`} className={classes.actionItem}>
        <div className={classes.actionItemHeader}>
          <ThemedIcon name={typeMeta.icon} size={16} theme={typeMeta.iconTheme} />
          <div style={{ flexGrow: 1 }}>{typeMeta.name}</div>
          <Button
            icon={<TbTrash />}
            size="small"
            variant="invisible"
            onClick={() => updateActions(actions.filter((_, i) => i !== index))}
          />
        </div>
        {configWidget ? (
          <div className={classes.actionItemContent}>{configWidget}</div>
        ) : null}
      </div>
    );
  };

  return (
    <div className={classes.actionListContainer}>
      <div ref={animatedList} className={classes.actionList}>
        <div key="workflow-options" className={classes.actionItem}>
          <div className={classes.actionItemContent} style={{ paddingTop: 10 }}>
            <ShortcutPicker
              info={props.quickSelectKeyInfo}
              initialValue={props.workflow?.quickSelectKey || ''}
              label={i18next.t('settings.quick-select-key-label')}
              mode="key-names"
              placeholder={props.quickSelectKeyPlaceholder}
              recordingPlaceholder={i18next.t('settings.quick-select-key-recording')}
              useModifiers={false}
              onChange={(shortcut) => {
                props.onUpdateWorkflow({
                  ...props.workflow,
                  actions: props.workflow?.actions || [],
                  quickSelectKey: shortcut || undefined,
                });
              }}
            />
            {props.description ? <Note marginTop={4}>{props.description}</Note> : null}
          </div>
        </div>

        {actions.map((action, index) => renderAction(action, index))}

        {actions.length < props.maxActions ? (
          <div key="add-action" className={classes.addActionContainer}>
            <Button
              isBlock
              icon={<TbPlus />}
              label={i18next.t('settings.workflow-editor.add-action')}
              variant="pill"
              onClick={() => setActionPickerVisible(true)}
            />
          </div>
        ) : null}

        {props.maxActions > 0 && actions.length === 0 && (
          <Note key="empty-hint" isCentered marginTop={16}>
            {props.emptyHint}
          </Note>
        )}
      </div>
      <ActionPicker
        isVisible={actionPickerVisible}
        onClose={() => setActionPickerVisible(false)}
        onSelect={(action) => {
          updateActions([action]);
          setActionPickerVisible(false);
        }}
      />
    </div>
  );
}
