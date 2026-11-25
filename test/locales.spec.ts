//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { expect } from 'chai';
import * as fs from 'fs';
import * as path from 'path';

// KandoAir maintains the Chinese translations itself: every English string must also exist
// in Simplified and Traditional Chinese, otherwise the UI shows English text.

const root = process.cwd();

function keys(node: unknown, prefix = ''): string[] {
  if (node && typeof node === 'object') {
    return Object.entries(node).flatMap(([key, value]) =>
      keys(value, prefix ? `${prefix}.${key}` : key)
    );
  }
  return typeof node === 'string' && node !== '' ? [prefix] : [];
}

function translation(locale: string): unknown {
  const file = path.join(root, 'locales', locale, 'translation.json');
  return JSON.parse(fs.readFileSync(file, 'utf-8'));
}

describe('Chinese translations', () => {
  // Chinese has no singular plural form, so i18next never uses the "_one" variants.
  const english = keys(translation('en')).filter((key) => !key.endsWith('_one'));

  for (const locale of ['zh-Hans', 'zh-Hant']) {
    it(`${locale} contains every English string`, () => {
      const present = new Set(keys(translation(locale)));
      expect(english.filter((key) => !present.has(key))).to.deep.equal([]);
    });
  }
});
