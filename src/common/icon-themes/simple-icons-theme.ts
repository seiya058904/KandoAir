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
 * This class implements an icon theme that uses the Simple Icons font as icons. The
 * searchable icon metadata is attached lazily by the settings window so the menu bundle
 * stays small and fast to parse.
 */
export class SimpleIconsTheme implements IconTheme {
  protected listIconsFn: (searchTerm: string) => Array<string> = () => [];
  protected getTitleFn: (icon: string) => string = (icon) => icon;

  /** Attaches the searchable icon list implementation (used by the settings window). */
  public setListIcons(fn: (searchTerm: string) => Array<string>) {
    this.listIconsFn = fn;
  }

  /** Attaches the icon title lookup implementation (used by the settings window). */
  public setGetTitle(fn: (icon: string) => string) {
    this.getTitleFn = fn;
  }

  /** Returns the human-readable title for a Simple Icons slug. */
  public getTitle(icon: string) {
    return this.getTitleFn(icon);
  }

  /** Returns a human-readable name of the icon theme. */
  get name() {
    return 'Simple Icons';
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

    iconDiv.classList.add('si');
    iconDiv.classList.add('si-' + icon);

    return containerDiv;
  }

  /** Returns information about the icon picker for this icon theme. */
  get iconPickerInfo() {
    return {
      type: 'list' as const,
      usesTextColor: true,
      listIcons: (searchTerm: string) => this.listIconsFn(searchTerm),
    };
  }
}
