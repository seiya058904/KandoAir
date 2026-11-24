//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { NormalModuleReplacementPlugin, DefinePlugin } from 'webpack';
import CopyPlugin from 'copy-webpack-plugin';
import JSON5 from 'json5';
import fs from 'fs';
import path from 'path';

// Get a list of all available locales.
const localesDir = path.resolve(__dirname, '../../locales');
const cLocales = fs
  .readdirSync(localesDir, { withFileTypes: true })
  .filter((dirent) => dirent.isDirectory())
  .map((dirent) => dirent.name);

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const plugins: any[] = [
  // WebView2 only needs SimpleIcons.woff2. Redirect all legacy font formats and the
  // unused SimpleIcons-Fit variants so webpack does not emit ~6.4 MB of dead fonts.
  new NormalModuleReplacementPlugin(
    /SimpleIcons(?:-Fit)?\.(otf|ttf|eot|woff)$|SimpleIcons-Fit\.woff2$/,
    'SimpleIcons.woff2'
  ),

  // KandoAir only runs on Windows.
  new DefinePlugin({
    cIsMac: false,
    cIsWindows: true,
    cIsLinux: false,
    cLocales: JSON.stringify(cLocales),
  }),

  // Copy the bundled icon theme, all bundled menu/sound themes and the translations.
  // Each theme's theme.json5 is converted to JSON so that the native host can read it.
  new CopyPlugin({
    patterns: [
      { from: 'icon-themes', to: 'assets/icon-themes', context: 'assets/' },
      {
        from: 'menu-themes',
        to: 'assets/menu-themes',
        context: 'assets/',
        globOptions: { ignore: ['**/theme.json5'] },
      },
      {
        from: 'menu-themes/*/theme.json5',
        to: ({ absoluteFilename }) => {
          const themeDir = path.basename(path.dirname(absoluteFilename));
          return `assets/menu-themes/${themeDir}/theme.json`;
        },
        context: 'assets/',
        transform: (content: Buffer) =>
          JSON.stringify(JSON5.parse(content.toString()), null, 2),
      },
      {
        from: 'sound-themes',
        to: 'assets/sound-themes',
        context: 'assets/',
        globOptions: { ignore: ['**/theme.json5'] },
      },
      {
        from: 'sound-themes/*/theme.json5',
        to: ({ absoluteFilename }) => {
          const themeDir = path.basename(path.dirname(absoluteFilename));
          return `assets/sound-themes/${themeDir}/theme.json`;
        },
        context: 'assets/',
        transform: (content: Buffer) =>
          JSON.stringify(JSON5.parse(content.toString()), null, 2),
      },
      { from: 'locales', to: 'locales' },
    ],
  }),
];
