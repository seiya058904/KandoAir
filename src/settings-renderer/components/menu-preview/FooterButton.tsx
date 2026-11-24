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
import classNames from 'classnames/bind';
import { TbPlus } from 'react-icons/tb';

import * as classes from './FooterButton.module.scss';
const cx = classNames.bind(classes);

import { Button, ThemedIcon, Popover, Note } from '../common';

type Props = {
  /** A unique ID for the menu-item type. */
  readonly id: string;

  /** The name of the menu-item type. */
  readonly name: string;

  /** A short description of the menu-item type. */
  readonly description: string;

  /** The icon to display for the menu-item type. */
  readonly icon: string;

  /** The theme of the icon to display for the menu-item type. */
  readonly iconTheme: string;
};

/**
 * This component encapsules a single menu-item type which can be dragged to the menu
 * preview, or double-clicked / added from its popover to append to the current menu.
 *
 * @param props - The properties for the footer button component.
 * @returns A footer button element.
 */
export default function FooterButton(props: Props) {
  const [dragging, setDragging] = React.useState(false);
  const [isPopoverOpen, setIsPopoverOpen] = React.useState(false);

  const quickAdd = () => {
    window.dispatchEvent(
      new CustomEvent('kando/quick-add-menu-item', { detail: props.id })
    );
    setIsPopoverOpen(false);
  };

  return (
    <Popover
      content={
        <div className={classes.popoverContent}>
          <p>{props.description}</p>
          <Note>{i18next.t('settings.add-menu-item-hint')}</Note>
          <Button
            isBlock
            icon={<TbPlus />}
            label={i18next.t('settings.add-to-menu')}
            size="small"
            variant="primary"
            onClick={quickAdd}
          />
        </div>
      }
      isVisible={isPopoverOpen}
      position="top"
      onClose={() => {
        setIsPopoverOpen(false);
      }}>
      <div
        draggable
        className={cx({
          menuItem: true,
          dragging,
        })}
        data-tooltip-content={isPopoverOpen || dragging ? undefined : props.name}
        data-tooltip-id="main-tooltip"
        onClick={() => setIsPopoverOpen(!isPopoverOpen)}
        onDoubleClick={quickAdd}
        onDragEnd={() => setDragging(false)}
        onDragStart={(event) => {
          event.dataTransfer.setData('kando/new-item-type', props.id);
          setDragging(true);
          setIsPopoverOpen(false);
        }}>
        <ThemedIcon name={props.icon} size="100%" theme={props.iconTheme} />
      </div>
    </Popover>
  );
}
