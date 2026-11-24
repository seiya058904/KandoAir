//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { matchSorter } from 'match-sorter';
import emojis from 'emojilib';
import iconsData from 'simple-icons-font/font/simple-icons.min.json';

import { IconThemeRegistry } from '../../common/icon-themes/icon-theme-registry';
import { SimpleIconsTheme } from '../../common/icon-themes/simple-icons-theme';
import { MaterialSymbolsTheme } from '../../common/icon-themes/material-symbols-theme';
import { EmojiTheme } from '../../common/icon-themes/emoji-theme';

let simpleIconsList: Array<{ title: string; slug: string }> | null = null;
let simpleIconsTitles: Map<string, string> | null = null;
let materialSymbolNames: Array<string> | null = null;
let emojiEntries: Array<Array<string>> | null = null;

/**
 * Attaches the searchable icon indexes to the built-in icon themes. Keeping these imports
 * inside the settings renderer keeps the menu window bundle ~750 KB smaller and faster.
 */
export function attachIconPickerData() {
  const themes = IconThemeRegistry.getInstance().getThemes();

  const ensureSimpleIcons = () => {
    if (!simpleIconsList || !simpleIconsTitles) {
      simpleIconsList = iconsData.map((icon) => ({
        title: icon.title,
        slug: icon.slug,
      }));
      simpleIconsTitles = new Map(simpleIconsList.map((icon) => [icon.slug, icon.title]));
    }
  };

  const searchSimpleIcons = (searchTerm: string) => {
    ensureSimpleIcons();
    return matchSorter(simpleIconsList!, searchTerm, { keys: ['title', 'slug'] }).map(
      (icon) => icon.slug
    );
  };

  const getSimpleIconTitle = (slug: string) => {
    ensureSimpleIcons();
    return simpleIconsTitles!.get(slug) ?? slug;
  };

  const simple = themes.get('simple-icons');
  if (simple instanceof SimpleIconsTheme) {
    simple.setListIcons(searchSimpleIcons);
    simple.setGetTitle(getSimpleIconTitle);
  }
  const simpleColored = themes.get('simple-icons-colored');
  if (simpleColored instanceof SimpleIconsTheme) {
    simpleColored.setListIcons(searchSimpleIcons);
    simpleColored.setGetTitle(getSimpleIconTitle);
  }

  const material = themes.get('material-symbols-rounded');
  if (material instanceof MaterialSymbolsTheme) {
    material.setListIcons((searchTerm: string) => {
      if (!materialSymbolNames) {
        const raw: string = require('!!raw-loader!material-symbols/index.d.ts').default;
        materialSymbolNames = (raw.match(/"(.*?)"/g) ?? []).map((n) => n.slice(1, -1));
      }
      return matchSorter(materialSymbolNames, searchTerm);
    });
  }

  const emoji = themes.get('emoji');
  if (emoji instanceof EmojiTheme) {
    emoji.setListIcons((searchTerm: string) => {
      if (!emojiEntries) {
        emojiEntries = Object.entries(emojis).map(([glyph, descriptors]) => [
          glyph,
          ...descriptors,
        ]);
      }
      return matchSorter(emojiEntries, searchTerm, {
        threshold: matchSorter.rankings.CONTAINS,
      }).map(([glyph]) => glyph);
    });
  }
}
