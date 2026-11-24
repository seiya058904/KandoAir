<!--
SPDX-FileCopyrightText: KandoAir contributors
SPDX-License-Identifier: MIT
-->

# KandoAir – notes for coding agents

KandoAir is a Windows-only derivative of Kando 3.0.1 (branch `kandoair`, remote `upstream`
= kando-menu/kando). The Electron main process was replaced by a native Rust host; the
menu and settings pages are Kando's original web frontends hosted in WebView2.

## Layout and boundaries

- `host/` – Rust host (`windows` + `webview2-com` crates, no async runtime). One UI thread
  with a message loop; blocking work runs on worker threads (`app::spawn_worker`) and
  reports back via `WM_APP_WORKER`. Modal dialogs must run via `App::defer`, never inside
  WebView2 event handlers.
- `src/` – web frontend (TypeScript/React/SCSS). It must not import Electron or Node.js
  modules (enforced by ESLint `no-restricted-imports`). All host calls go through
  `src/common/host-bridge.ts`; channel names are Kando's original IPC channel names
  (`common.*`, `menu-window.*`, `settings-window.*`) and are routed in `host/src/app.rs`
  and `host/src/settings_window.rs`.
- Pages are served from virtual hosts: `app.kandoair.example` (bundled `dist/web`),
  `user.kandoair.example` (config dir), `sys.kandoair.example` (generated app icons and
  local image files). Never use `file://` URLs in the frontend.

## Invariants (checked by tests)

- Exactly nine single-step native actions: `launch-app`, `open-file`, `open-uri`,
  `simulate-hotkey`, `execute-command`, `set-clipboard`, `execute-macro`,
  `focus-window`, `open-menu`. The list exists in `host/src/config.rs` (`ACTION_TYPES`),
  `src/common/action-type-registry.ts` (`ACTION_TYPES`) and the zod schema in
  `src/common/settings-schemata/menu-settings-v2.ts`; Rust and mocha tests compare them.
- The general settings keys of `GeneralSettings` (Rust) and `general-settings-v1.ts` (zod)
  must be identical (Rust test `general_settings_match_typescript_schema`).
- Host i18n keys live in `host/src/strings.rs` and must exist in `locales/en`,
  `locales/zh-Hans` and `locales/zh-Hant` (Rust test). New UI strings: add them to
  `locales/en/translation.json` (plus zh-Hans/zh-Hant), then run `npm run i18n:types` so
  that `tsc` checks the keys. Do not run `i18n:extract` (it rewrites translation files).
- KandoAir is a project of its own: user-visible text says KandoAir. The original Kando is
  credited only in the about dialog (`settings.about-dialog.message-air`), the README and
  the license files; nothing links to Kando's website, Discord or donation pages
  (`test/branding.spec.ts`).
- Text files use LF line endings. When editing files with Python on Windows, write bytes
  or pass `newline='\n'`.

## Commands

```powershell
npm ci                      # install frontend dependencies
npm run build:web           # build dist\web (menu + settings pages)
npm run build               # dist\KandoAir: release host + web frontend
npm run lint; npm run tscheck; npm test
cargo test --manifest-path host/Cargo.toml
cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings
./scripts/measure.ps1 -Name KandoAir -Seconds 60   # memory/CPU of the process tree
```

Run the host against a development frontend with `KANDOAIR_WEB_DIR=<repo>\dist\web` and an
isolated config with `--config-dir <dir>`. `KANDOAIR_DEBUG_PORT=9222` enables WebView2
remote debugging (CDP): the menu page is on that port, the settings page on port + 1,
because the settings window runs in its own WebView2 browser processes (they exit with the
window, which releases its memory).

To quit a running instance gracefully (saves settings, removes the tray icon), post
`WM_CLOSE` to the window of class `KandoAirHostWindow`; `taskkill` without /f does not
reach this hidden window. From PowerShell, pass `[NullString]::Value` as the window name
to `FindWindowW`, since `$null` becomes an empty string.

## Testing input simulation safely

Tests which inject keys or mouse input must first verify that the expected window is in
the foreground and abort otherwise; never inject input while another app may be active.
Chinese IMEs (TSF) compose injected letter keys in the target window; disable the IME of
test target windows (`ImmDisableIME(-1)`) when checking typed text.
