//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

/**
 * A utility class to get the name of a key from a keyboard event. The result is similar
 * to the `name` property of the KeyboardEvent, but it also handles some special cases
 * like the numpad keys, the digit keys, and active IME composition (`event.key ===
 * 'Process'`). It also normalizes the names to be more consistent with Electron's
 * accelerator format.
 */
export default class KeyMapper {
  /**
   * A map to convert DOM key codes to their names. This is initialized in the init
   * method.
   */
  private static keymap: Map<string, string> = new Map();

  /** A map to convert numpad keys to their names. */
  private static numpadKeys = new Map([
    ['0', 'num0'],
    ['1', 'num1'],
    ['2', 'num2'],
    ['3', 'num3'],
    ['4', 'num4'],
    ['5', 'num5'],
    ['6', 'num6'],
    ['7', 'num7'],
    ['8', 'num8'],
    ['9', 'num9'],
    [',', 'numdec'],
    ['.', 'numdec'],
    ['+', 'numadd'],
    ['-', 'numsub'],
    ['*', 'nummult'],
    ['/', 'numdiv'],
  ]);

  /** Fallback map from KeyboardEvent.code when an IME intercepts event.key ('Process'). */
  private static codeFallbackMap = new Map([
    ['Space', 'Space'],
    ['Enter', 'Return'],
    ['NumpadEnter', 'Return'],
    ['Tab', 'Tab'],
    ['Backspace', 'Backspace'],
    ['Delete', 'Delete'],
    ['Insert', 'Insert'],
    ['Escape', 'Escape'],
    ['Home', 'Home'],
    ['End', 'End'],
    ['PageUp', 'PageUp'],
    ['PageDown', 'PageDown'],
    ['ArrowUp', 'Up'],
    ['ArrowDown', 'Down'],
    ['ArrowLeft', 'Left'],
    ['ArrowRight', 'Right'],
    ['Minus', '-'],
    ['Equal', '='],
    ['BracketLeft', '['],
    ['BracketRight', ']'],
    ['Backslash', '\\'],
    ['Semicolon', ';'],
    ['Quote', "'"],
    ['Backquote', '`'],
    ['Comma', ','],
    ['Period', '.'],
    ['Slash', '/'],
    ['Numpad0', 'num0'],
    ['Numpad1', 'num1'],
    ['Numpad2', 'num2'],
    ['Numpad3', 'num3'],
    ['Numpad4', 'num4'],
    ['Numpad5', 'num5'],
    ['Numpad6', 'num6'],
    ['Numpad7', 'num7'],
    ['Numpad8', 'num8'],
    ['Numpad9', 'num9'],
    ['NumpadDecimal', 'numdec'],
    ['NumpadAdd', 'numadd'],
    ['NumpadSubtract', 'numsub'],
    ['NumpadMultiply', 'nummult'],
    ['NumpadDivide', 'numdiv'],
  ]);

  /**
   * Initializes the key mapper by getting the keyboard layout map from the navigator.
   * This should be called once when the application starts.
   */
  static async init() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const keyboard = (globalThis as any).navigator?.keyboard;
    if (typeof keyboard?.getLayoutMap !== 'function') {
      return;
    }
    try {
      this.keymap = await keyboard.getLayoutMap();
    } catch {
      // Ignore if keyboard layout API is unavailable in this environment.
    }
  }

  /** Fallback conversion from KeyboardEvent.code to a key name. */
  private static nameFromCode(code: string): string | undefined {
    if (!code) {
      return undefined;
    }
    if (code.startsWith('Key') && code.length === 4) {
      return code.slice(3);
    }
    if (code.startsWith('Digit') && code.length === 6) {
      return code.slice(5);
    }
    if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(code)) {
      return code;
    }
    return this.codeFallbackMap.get(code);
  }

  /**
   * Gets the name of the key from the given keyboard event. This method takes into
   * account the key code, the key value, and the location of the key to determine the
   * correct name. It also normalizes some names to be more consistent with Electron's
   * accelerator format.
   *
   * @param event The keyboard event to get the key name from.
   * @returns The name of the key.
   */
  static getName(event: KeyboardEvent): string {
    let key = this.keymap.get(event.code) || event.key;

    // When an IME (such as Chinese Pinyin) is active, Chromium reports event.key as
    // 'Process' (or 'Unidentified'). Fall back to the physical key code so shortcut
    // recording and quick-select keys keep working.
    if (key === 'Process' || key === 'Unidentified') {
      key = this.nameFromCode(event.code) || key;
    }

    // Some DOM names differ from the names used by Electron. We need to map them.
    const nameMap = new Map([
      ['+', 'Plus'],
      [' ', 'Space'],
      ['Enter', 'Return'],
      ['ArrowUp', 'Up'],
      ['ArrowDown', 'Down'],
      ['ArrowLeft', 'Left'],
      ['ArrowRight', 'Right'],
    ]);

    key = nameMap.get(key) || key;

    // We can explicitly bind to numpad keys. We check location property to determine
    // if the key is on the numpad.
    const numpadLocation =
      typeof KeyboardEvent !== 'undefined' ? KeyboardEvent.DOM_KEY_LOCATION_NUMPAD : 3;
    if (event.location === numpadLocation) {
      key = this.numpadKeys.get(key) || key;
    }

    return key;
  }
}
