//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { IconTheme } from './icon-theme-registry';

/**
 * This class implements an icon theme that uses emojis as icons. The searchable emoji
 * list is attached lazily by the settings window.
 */
export class EmojiTheme implements IconTheme {
  private listIconsFn: (searchTerm: string) => Array<string> = () => [];

  /** Attaches the searchable emoji list implementation (used by the settings window). */
  public setListIcons(fn: (searchTerm: string) => Array<string>) {
    this.listIconsFn = fn;
  }

  /** Returns a human-readable name of the icon theme. */
  get name() {
    return 'Emojis';
  }

  /**
   * Creates a div element that contains the icon with the given name.
   *
   * @param icon One of the icons returned by `listIcons`.
   * @returns A div element that contains the icon.
   */
  createIcon(icon: string) {
    const containerDiv = document.createElement('div');
    containerDiv.classList.add('icon-container');

    const iconDiv = document.createElement('i');
    containerDiv.appendChild(iconDiv);

    iconDiv.classList.add('emoji-icon');
    iconDiv.innerText = icon;

    return containerDiv;
  }

  /** Returns information about the icon picker for this icon theme. */
  get iconPickerInfo() {
    return {
      type: 'list' as const,
      usesTextColor: false,
      listIcons: (searchTerm: string) => this.listIconsFn(searchTerm),
    };
  }
}
