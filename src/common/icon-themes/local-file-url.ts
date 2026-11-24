//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

/**
 * Kando allows `file://` URIs as icons. Pages hosted by KandoAir cannot load such URIs
 * directly, so they are loaded via the host instead. Other sources are returned as is.
 *
 * @param source An icon source (data URI, http(s) URL or file URI).
 * @returns A URL which can be loaded by the page.
 */
export function toLoadableUrl(source: string): string {
  if (!source.toLowerCase().startsWith('file:')) {
    return source;
  }
  // Preserve UNC paths (`file://server/share/...` -> `//server/share/...`), while
  // stripping the leading triple slash on drive paths (`file:///C:/...` -> `C:/...`).
  let path = source.replace(/^file:/i, '');
  if (path.startsWith('///')) {
    path = path.replace(/^\/+/, '');
  } else if (path.startsWith('//') && /^\/\/[a-zA-Z]:/.test(path)) {
    path = path.slice(2);
  }
  path = path
    .split('/')
    .map((segment) => {
      try {
        return decodeURIComponent(segment);
      } catch {
        return segment;
      }
    })
    .join('/');
  return `https://sys.kandoair.example/file?path=${encodeURIComponent(path)}`;
}
