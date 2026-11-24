//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/menu/kando           //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-License-Identifier: MIT

import { matchSorter } from 'match-sorter';

import { IconTheme } from './icon-theme-registry';

/**
 * This icon theme shows the icons of installed applications. In KandoAir, the icons are
 * rendered on demand by the native host and loaded via URL. The list of icon names (the
 * app names) is only needed by the icon picker of the settings window and is provided via
 * setIconNames().
 */
export class SystemTheme implements IconTheme {
  private iconNames: Array<string> = [];

  get name() {
    return 'System Icons';
  }

  /**
   * Sets the names of all available icons. They are sorted alphabetically.
   *
   * @param names The names of the installed applications.
   */
  public setIconNames(names: Array<string>) {
    this.iconNames = [...names].sort((a, b) => a.localeCompare(b));
  }

  /**
   * Creates a div element that contains the icon of the given application.
   *
   * @param icon The name of the application.
   * @returns A div element that contains the icon.
   */
  createIcon(icon: string, reloadIteration = 0) {
    const containerDiv = document.createElement('div');
    containerDiv.classList.add('icon-container');

    const iconDiv = document.createElement('img');
    const suffix = reloadIteration > 0 ? `?reload=${reloadIteration}` : '';
    iconDiv.src = `https://sys.kandoair.example/app-icon/${encodeURIComponent(icon)}${suffix}`;
    iconDiv.draggable = false;
    containerDiv.appendChild(iconDiv);

    return containerDiv;
  }

  /**
   * The system theme uses a list of icon names for the icon picker.
   *
   * @returns The icon picker info.
   */
  get iconPickerInfo() {
    return {
      type: 'list' as const,
      usesTextColor: false,
      listIcons: (searchTerm: string) => matchSorter(this.iconNames, searchTerm),
    };
  }
}
