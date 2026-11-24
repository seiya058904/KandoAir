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

// KandoAir is a project of its own: user-visible text uses the name KandoAir. The original
// Kando is only credited in the about dialog, and nothing links to its website or community.

const root = process.cwd();
const NAME = /(?<![A-Za-z])Kando(?![A-Za-z])/;
const CREDITS = ['settings.about-dialog.message-air'];
const ORIGINAL_LINKS = /kando\.menu|github\.com\/kando-menu|discord\.gg|ko-fi\.com/;

function strings(node: unknown, prefix: string): [string, string][] {
  if (typeof node === 'string') {
    return [[prefix, node]];
  }
  if (node && typeof node === 'object') {
    return Object.entries(node).flatMap(([key, value]) =>
      strings(value, prefix ? `${prefix}.${key}` : key)
    );
  }
  return [];
}

function codeLines(file: string): [number, string][] {
  return fs
    .readFileSync(file, 'utf-8')
    .split('\n')
    .map((line, index): [number, string] => [index + 1, line.trim()])
    .filter(([, line]) => !/^(\/\/|\*|\/\*)/.test(line));
}

function sourceFiles(dir: string): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return sourceFiles(file);
    }
    return /\.tsx?$/.test(entry.name) && !entry.name.endsWith('.d.ts') ? [file] : [];
  });
}

describe('Branding', () => {
  it('translations use the name KandoAir', () => {
    const offending: string[] = [];
    for (const language of fs.readdirSync(path.join(root, 'locales'))) {
      const file = path.join(root, 'locales', language, 'translation.json');
      for (const [key, value] of strings(
        JSON.parse(fs.readFileSync(file, 'utf-8')),
        ''
      )) {
        if (NAME.test(value) && !CREDITS.includes(key)) {
          offending.push(`${language}: ${key}`);
        }
      }
    }
    expect(offending).to.deep.equal([]);
  });

  it('the example menu and the page titles do not use the original name', () => {
    const menu = fs.readFileSync(path.join(root, 'assets', 'example-menu.json'), 'utf-8');
    expect(menu.toLowerCase()).to.not.include('kando');
    for (const page of ['menu-renderer', 'settings-renderer']) {
      const html = fs.readFileSync(path.join(root, 'src', page, 'index.html'), 'utf-8');
      expect(html).to.match(/<title>KandoAir[^<]*<\/title>/);
    }
  });

  it('only the about dialog links to the original project', () => {
    const offending = sourceFiles(path.join(root, 'src'))
      .filter((file) => !file.endsWith('AboutDialog.tsx'))
      .flatMap((file) =>
        codeLines(file)
          .filter(([, line]) => ORIGINAL_LINKS.test(line))
          .map(([number]) => `${path.relative(root, file)}:${number}`)
      );
    expect(offending).to.deep.equal([]);
  });

  it('strings of the host use the name KandoAir', () => {
    const dir = path.join(root, 'host', 'src');
    const offending = fs.readdirSync(dir).flatMap((name) =>
      codeLines(path.join(dir, name))
        // Messages about the file format of Kando itself may name it.
        .filter(([, line]) => !line.includes('Menus from Kando versions'))
        .filter(([, line]) => (line.match(/"[^"]*"/g) ?? []).some((s) => NAME.test(s)))
        .map(([number]) => `${name}:${number}`)
    );
    expect(offending).to.deep.equal([]);
  });
});
