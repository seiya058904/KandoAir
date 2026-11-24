//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

import React from 'react';
import { RgbaStringColorPicker } from 'react-colorful';
import chroma from 'chroma-js';

import * as classes from './ColorButton.module.scss';
import Popover from './Popover';

type Props = {
  /** Function to call when the color is changed. */
  readonly onChange?: (color: string) => void;

  /** Name of the color. Will be shown in the popover. */
  readonly name?: string;

  /** Initial color. */
  readonly color: string;
};

/**
 * A customizable color button component. Once clicked, a popover will open with a color
 * picker and an input field to enter a color.
 *
 * @param props - The properties for the color button component.
 * @returns A color button element.
 */
function safeColorCss(color: string): string {
  try {
    return chroma(color).css();
  } catch {
    return 'rgba(255, 255, 255, 1)';
  }
}

export default function ColorButton(props: Props) {
  const [isPopoverOpen, setIsPopoverOpen] = React.useState(false);
  const initialCss = safeColorCss(props.color);
  const [pickerColor, setPickerColor] = React.useState(initialCss);
  const [cssColor, setCSSColor] = React.useState(initialCss);
  const [inputColor, setInputColor] = React.useState(initialCss);
  const throttleTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingColor = React.useRef<string | null>(null);
  const onChangeRef = React.useRef(props.onChange);

  React.useEffect(() => {
    onChangeRef.current = props.onChange;
  }, [props.onChange]);

  React.useEffect(() => {
    return () => {
      if (throttleTimer.current) {
        clearTimeout(throttleTimer.current);
      }
    };
  }, []);

  const emitColorThrottled = React.useCallback((color: string) => {
    pendingColor.current = color;
    if (!throttleTimer.current) {
      throttleTimer.current = setTimeout(() => {
        throttleTimer.current = null;
        if (pendingColor.current !== null) {
          onChangeRef.current?.(pendingColor.current);
          pendingColor.current = null;
        }
      }, 60);
    }
  }, []);

  const flushColor = React.useCallback((color: string) => {
    if (throttleTimer.current) {
      clearTimeout(throttleTimer.current);
      throttleTimer.current = null;
    }
    pendingColor.current = null;
    onChangeRef.current?.(color);
  }, []);

  React.useEffect(() => {
    const color = safeColorCss(props.color);
    setPickerColor(color);
    setCSSColor(color);
    setInputColor(color);
  }, [props.color]);

  return (
    <Popover
      content={
        <>
          {props.name ? <div className={classes.popoverHeader}>{props.name}</div> : null}
          <RgbaStringColorPicker
            color={pickerColor}
            onChange={(newColor) => {
              setPickerColor(newColor);
              const converted = chroma(newColor).css();
              setCSSColor(converted);
              setInputColor(converted);
              emitColorThrottled(converted);
            }}
          />
          <input
            className={classes.colorInput}
            type="text"
            value={inputColor}
            onChange={(event) => {
              setInputColor(event.target.value);
              try {
                const converted = chroma(event.target.value).css();
                setCSSColor(converted);
                setPickerColor(converted);
                emitColorThrottled(converted);
              } catch {
                // ignore intermediate invalid color strings while typing
              }
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                flushColor(cssColor);
                setIsPopoverOpen(false);
              }
            }}
          />
        </>
      }
      isVisible={isPopoverOpen}
      position="top"
      onClose={() => {
        flushColor(cssColor);
        setIsPopoverOpen(false);
      }}>
      <div
        className={classes.colorButton}
        data-tooltip-content={props.name}
        data-tooltip-id="main-tooltip"
        onClick={() => setIsPopoverOpen(!isPopoverOpen)}>
        <div className={classes.color} style={{ backgroundColor: cssColor }} />
      </div>
    </Popover>
  );
}
