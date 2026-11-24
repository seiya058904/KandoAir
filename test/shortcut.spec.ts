//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

import { expect } from 'chai';

import { formatShortcutForDisplay } from '../src/common';
import KeyMapper from '../src/common/key-mapper';

describe('formatShortcutForDisplay', () => {
  it('should use native symbols for macOS modifiers', () => {
    expect(formatShortcutForDisplay('Control+Option+Command+Shift+A', true)).to.equal(
      '⌃⌥⌘⇧A'
    );
  });

  it('should normalize accelerator modifier aliases for display', () => {
    expect(formatShortcutForDisplay('Ctrl+Alt+Meta+Space', true)).to.equal('⌃⌥⌘Space');
    expect(formatShortcutForDisplay('CmdOrCtrl+P', true)).to.equal('⌘P');
  });

  it('should leave shortcuts unchanged on other platforms', () => {
    expect(formatShortcutForDisplay('Control+Alt+A', false)).to.equal('Control+Alt+A');
  });
});

describe('KeyMapper', () => {
  it('should fall back to KeyboardEvent.code when an IME reports Process', () => {
    const makeEvent = (key: string, code: string, location = 0) =>
      ({ key, code, location }) as KeyboardEvent;

    expect(KeyMapper.getName(makeEvent('Process', 'KeyA'))).to.equal('A');
    expect(KeyMapper.getName(makeEvent('Process', 'Digit5'))).to.equal('5');
    expect(KeyMapper.getName(makeEvent('Process', 'Space'))).to.equal('Space');
    expect(KeyMapper.getName(makeEvent('Process', 'F9'))).to.equal('F9');
    expect(KeyMapper.getName(makeEvent('Process', 'ArrowUp'))).to.equal('Up');
    expect(KeyMapper.getName(makeEvent('Process', 'Numpad3', 3))).to.equal('num3');
  });
});
