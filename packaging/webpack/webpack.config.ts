//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import path from 'path';
import HtmlWebpackPlugin from 'html-webpack-plugin';
import type { Configuration } from 'webpack';

import { rendererConfig } from './webpack.renderer.config';

const root = path.resolve(__dirname, '../..');

/**
 * Builds the two pages hosted by the native KandoAir host into dist/web:
 *
 * - Menu/index.html: the pie menu (kept loaded while hidden)
 * - Settings/index.html: the settings window (loaded on demand)
 */
export default (_env: unknown, argv: { mode?: string }): Configuration => ({
  ...rendererConfig,
  context: root,
  target: 'web',
  devtool: argv.mode === 'production' ? false : 'source-map',
  entry: {
    menu: './src/menu-renderer/index.ts',
    settings: './src/settings-renderer/index.tsx',
  },
  output: {
    path: path.join(root, 'dist', 'web'),
    filename: '[name]/index.js',
    publicPath: '/',
    clean: true,
  },
  plugins: [
    ...(rendererConfig.plugins ?? []),
    new HtmlWebpackPlugin({
      template: './src/menu-renderer/index.html',
      filename: 'menu/index.html',
      chunks: ['menu'],
    }),
    new HtmlWebpackPlugin({
      template: './src/settings-renderer/index.html',
      filename: 'settings/index.html',
      chunks: ['settings'],
    }),
  ],
  performance: { hints: false },
});
