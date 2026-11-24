//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import type { ModuleOptions } from 'webpack';

export const rules: Required<ModuleOptions>['rules'] = [
  {
    test: /\.tsx?$/,
    exclude: /(node_modules|\.webpack)/,
    use: {
      loader: 'ts-loader',
      options: {
        transpileOnly: true,
        compilerOptions: {
          module: 'esnext',
        },
      },
    },
  },
  {
    test: /\.(png|svg|jpg|jpeg|gif|mp4|ttf|woff2)$/i,
    type: 'asset/resource',
    generator: {
      filename: 'assets/[name].[hash:8][ext]',
    },
  },
  {
    test: /\.css$/,
    use: ['style-loader', 'css-loader'],
  },
];
