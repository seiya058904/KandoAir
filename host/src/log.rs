// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! A tiny file logger. Log lines are prefixed with the milliseconds since startup. The
//! log file is truncated when it grows larger than one megabyte at startup.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static FILE: Mutex<Option<File>> = Mutex::new(None);

/// Opens (or creates) the log file. Calling this is optional; without it, log lines are
/// only written to the debugger output in debug builds.
pub fn init(path: &Path) {
    START.get_or_init(Instant::now);

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let too_large = std::fs::metadata(path)
        .map(|m| m.len() > 1024 * 1024)
        .unwrap_or(false);

    let file = OpenOptions::new()
        .create(true)
        .append(!too_large)
        .write(true)
        .truncate(too_large)
        .open(path);

    if let Ok(file) = file {
        *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    }
}

/// Milliseconds since the logger was initialized (or since the first log call).
pub fn elapsed_ms() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Writes a single line to the log. Use the `log!` macro instead of calling this directly.
pub fn write(line: &str) {
    let text = format!("[{:>10.1}] {line}\n", elapsed_ms());

    #[cfg(debug_assertions)]
    eprint!("{text}");

    if let Some(file) = FILE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = file.write_all(text.as_bytes());
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write(&format!($($arg)*))
    };
}
