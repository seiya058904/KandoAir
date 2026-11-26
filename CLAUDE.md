<!--
SPDX-FileCopyrightText: KandoAir contributors
SPDX-License-Identifier: MIT
-->

# KandoAir – notes for coding agents

Windows-only derivative of Kando 3.0.1 (branch `kandoair`, remote `upstream` = `kando-menu/kando`): native Rust host (`host/`, `windows` + `webview2-com`, no async runtime) + original web frontends (`src/`, WebView2).

## Critical Architecture & Runtime Invariants
- **Rust UI thread**: single message loop; run blocking work on worker threads (`app::spawn_worker` → `WM_APP_WORKER`); run modal dialogs via `App::defer`, never inside WebView2 event handlers.
- **RefCell re-entrancy trap**: never hold a `RefCell` borrow across Win32/WebView2 calls that can re-enter a window proc (`DestroyWindow`, `ShowWindow`, `TrackPopupMenuEx`, modal dialogs) — panics across `extern "system"` abort the process; copy values into locals first.
- **Web frontend & virtual hosts**: all host calls use `src/common/host-bridge.ts` (`common.*`, `menu-window.*`, `settings-window.*`). Pages load from virtual hosts (`app`/`user`/`sys.kandoair.example`) — **never use `file://` URLs**.
- **i18n trap**: add new UI strings to `locales/{en,zh-Hans,zh-Hant}/translation.json` and run `npm run i18n:types`. **Never run `npm run i18n:extract`** (it rewrites translation files).
- **Line endings**: text files use LF; when editing via Python on Windows, write bytes or pass `newline='\n'`.
- *(Action list, `GeneralSettings`, host i18n keys, Electron/Node import ban, and KandoAir branding are mechanically enforced by ESLint, mocha, and `cargo test`.)*

## Non-Obvious Dev, Debug & Input-Test Notes
- Standard build/lint/test commands are in `package.json` and `host/Cargo.toml` (`cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings`); process profiling: `./scripts/measure.ps1 -Name KandoAir -Seconds 60`.
- Dev env: `KANDOAIR_WEB_DIR=<repo>\dist\web`, `--config-dir <dir>`, `KANDOAIR_DEBUG_PORT=9222` (CDP: menu on port, settings on port + 1).
- Graceful exit: post `WM_CLOSE` to hidden window class `KandoAirHostWindow` (`taskkill` without `/f` misses it; in PowerShell pass `[NullString]::Value` as window name to `FindWindowW`).
- **Input simulation safety**: always verify expected window is foreground before injecting keys/mouse (abort otherwise). Disable Chinese TSF IME on test target windows (`ImmDisableIME(-1)`) when testing typed text.
