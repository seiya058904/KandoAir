// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Global shortcuts. Menu shortcuts use Electron's accelerator syntax (as stored by
//! Kando), e.g. `Control+Space` or `Control+Shift+K`, and are registered with
//! `RegisterHotKey`, which costs nothing while idle.
//!
//! Shortcuts can be inhibited temporarily: while a menu is open, its own shortcut is
//! released so that key-repeat events reach the menu (Turbo mode), and all shortcuts are
//! released while KandoAir simulates key presses or when the user pauses them.
//!
//! Input methods have hot keys of their own (Ctrl+Space turns Chinese input methods on
//! and off). While such an input method is active, Windows handles these keys before
//! RegisterHotKey shortcuts, so they never reach KandoAir; [`Hotkeys::bind`] reports them.

use std::collections::{BTreeMap, HashSet};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::Ime::ImmGetHotKey;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::log;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accelerator {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub vk: u32,
}

fn named_key(name: &str) -> Option<u32> {
    let key = match name.to_ascii_lowercase().as_str() {
        "plus" => return char_key('+'),
        "space" => 0x20,
        "tab" => 0x09,
        "capslock" => 0x14,
        "numlock" => 0x90,
        "scrolllock" => 0x91,
        "backspace" => 0x08,
        "delete" => 0x2E,
        "insert" => 0x2D,
        "return" | "enter" => 0x0D,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "escape" | "esc" => 0x1B,
        "volumeup" => 0xAF,
        "volumedown" => 0xAE,
        "volumemute" => 0xAD,
        "medianexttrack" => 0xB0,
        "mediaprevioustrack" => 0xB1,
        "mediastop" => 0xB2,
        "mediaplaypause" => 0xB3,
        "printscreen" => 0x2C,
        "numdec" => 0x6E,
        "numadd" => 0x6B,
        "numsub" => 0x6D,
        "nummult" => 0x6A,
        "numdiv" => 0x6F,
        other => {
            if let Some(digit) = other.strip_prefix("num") {
                let n: u32 = digit.parse().ok()?;
                return (n <= 9).then_some(0x60 + n);
            }
            if let Some(number) = other.strip_prefix('f') {
                let n: u32 = number.parse().ok()?;
                return (1..=24).contains(&n).then_some(0x70 + n - 1);
            }
            return None;
        }
    };
    Some(key)
}

/// Maps a printable character to a virtual key using the current keyboard layout.
fn char_key(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        return Some(c as u32);
    }
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase() as u32);
    }
    let mut buffer = [0u16; 2];
    let encoded = c.encode_utf16(&mut buffer);
    if encoded.len() != 1 {
        return None;
    }
    let result = unsafe { VkKeyScanW(encoded[0]) };
    if result == -1 {
        return None;
    }
    Some((result as u16 & 0xFF) as u32)
}

/// Parses an Electron-style accelerator such as `Control+Shift+K`.
pub fn parse(accelerator: &str) -> Result<Accelerator, String> {
    let text = accelerator.trim();
    if text.is_empty() {
        return Err("empty shortcut".into());
    }

    // A trailing "++" means the key itself is '+'.
    let (text, plus_key) = match text.strip_suffix("++") {
        Some(rest) => (rest, true),
        None => (text, false),
    };

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut vk = if plus_key { char_key('+') } else { None };

    for part in text.split('+').filter(|p| !p.is_empty()) {
        let modifier = match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "commandorcontrol" | "cmdorctrl" => Some(MOD_CONTROL),
            "command" | "cmd" | "super" | "meta" => Some(MOD_WIN),
            "alt" | "option" => Some(MOD_ALT),
            "altgr" => Some(MOD_CONTROL | MOD_ALT),
            "shift" => Some(MOD_SHIFT),
            _ => None,
        };
        if let Some(m) = modifier {
            modifiers |= m;
            continue;
        }
        if vk.is_some() {
            return Err(format!("more than one key in '{accelerator}'"));
        }
        let mut chars = part.chars();
        vk = match (chars.next(), chars.next()) {
            (Some(c), None) => char_key(c),
            _ => named_key(part),
        };
        if vk.is_none() {
            return Err(format!("unknown key '{part}'"));
        }
    }

    match vk {
        Some(vk) => Ok(Accelerator { modifiers, vk }),
        None => Err(format!("no key in '{accelerator}'")),
    }
}

/// A registered shortcut.
struct Entry {
    id: i32,
    accelerator: Option<Accelerator>,
    registered: bool,
}

/// Manages the registered shortcuts of the host window.
pub struct Hotkeys {
    hwnd: HWND,
    /// The bound shortcuts in binding order.
    entries: BTreeMap<String, Entry>,
    next_id: i32,
    /// Active inhibitions. The value "*" inhibits all shortcuts.
    inhibitions: BTreeMap<u32, String>,
    next_inhibition: u32,
}

/// A shortcut which could not be registered, or which an input method takes first.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Failure {
    pub shortcut: String,
    pub reason: String,
    /// The shortcut is registered, but it is a hot key of an installed input method.
    pub input_method: bool,
}

impl Hotkeys {
    pub fn new(hwnd: HWND) -> Hotkeys {
        Hotkeys {
            hwnd,
            entries: BTreeMap::new(),
            next_id: 1,
            inhibitions: BTreeMap::new(),
            next_inhibition: 1,
        }
    }

    /// Replaces the bound shortcuts. Returns the shortcuts which could not be registered.
    pub fn bind(&mut self, shortcuts: &[String]) -> Vec<Failure> {
        // Deduplicate by parsed Accelerator so aliases like "Control+Space" and "Ctrl+Space"
        // do not register twice and trigger false conflict errors.
        let mut wanted: Vec<&str> = Vec::new();
        let mut seen_acc: Vec<Accelerator> = Vec::new();
        let mut seen_raw: HashSet<&str> = HashSet::new();
        for s in shortcuts.iter().map(String::as_str).filter(|s| !s.is_empty()) {
            if !seen_raw.insert(s) {
                continue;
            }
            if let Ok(acc) = parse(s) {
                if seen_acc.contains(&acc) {
                    continue;
                }
                seen_acc.push(acc);
            }
            wanted.push(s);
        }
        let wanted_set: HashSet<&str> = wanted.iter().copied().collect();

        let obsolete: Vec<String> = self
            .entries
            .keys()
            .filter(|k| !wanted_set.contains(k.as_str()))
            .cloned()
            .collect();
        for shortcut in obsolete {
            if let Some(entry) = self.entries.remove(&shortcut)
                && entry.registered
            {
                unsafe {
                    let _ = UnregisterHotKey(Some(self.hwnd), entry.id);
                }
            }
        }

        let mut failures = Vec::new();
        let mut input_method_failures = Vec::new();
        let mut ime_hotkeys = None;
        for shortcut in wanted {
            if self.entries.contains_key(shortcut) {
                continue;
            }
            let accelerator = match parse(shortcut) {
                Ok(a) => {
                    let ime_hotkeys = ime_hotkeys.get_or_insert_with(active_ime_hotkeys);
                    if ime_hotkeys.iter().any(|h| ime_conflict(a, h)) {
                        log!("{shortcut} is a hot key of an input method");
                        input_method_failures.push(Failure {
                            shortcut: shortcut.to_string(),
                            reason: "input method hot key".into(),
                            input_method: true,
                        });
                    }
                    Some(a)
                }
                Err(reason) => {
                    failures.push(Failure {
                        shortcut: shortcut.to_string(),
                        reason,
                        input_method: false,
                    });
                    None
                }
            };
            let id = self.next_id;
            self.next_id += 1;
            self.entries.insert(
                shortcut.to_string(),
                Entry {
                    id,
                    accelerator,
                    registered: false,
                },
            );
        }

        failures.extend(self.apply());
        failures.extend(input_method_failures);
        failures
    }

    /// Returns the shortcut which belongs to a WM_HOTKEY id.
    pub fn shortcut_for_id(&self, id: i32) -> Option<String> {
        self.entries
            .iter()
            .find(|(_, e)| e.id == id)
            .map(|(s, _)| s.clone())
    }

    /// Temporarily releases a shortcut ("*" releases all). Returns an inhibition id.
    pub fn inhibit(&mut self, shortcut: &str) -> u32 {
        let id = self.next_inhibition;
        self.next_inhibition += 1;
        self.inhibitions.insert(id, shortcut.to_string());
        self.apply();
        id
    }

    /// Ends an inhibition. Returns shortcuts which could not be registered again.
    pub fn release(&mut self, inhibition: u32) -> Vec<Failure> {
        if self.inhibitions.remove(&inhibition).is_none() {
            return Vec::new();
        }
        self.apply()
    }

    fn is_inhibited(&self, shortcut: &str) -> bool {
        self.inhibitions
            .values()
            .any(|s| s == "*" || same_shortcut(s, shortcut))
    }

    /// Registers or unregisters shortcuts so that exactly the effective set is active.
    fn apply(&mut self) -> Vec<Failure> {
        let mut failures = Vec::new();
        let inhibited: Vec<bool> = self.entries.keys().map(|s| self.is_inhibited(s)).collect();
        for ((shortcut, entry), inhibited) in self.entries.iter_mut().zip(inhibited) {
            let Some(accelerator) = entry.accelerator else {
                continue;
            };
            let should = !inhibited;
            if should && !entry.registered {
                let result = unsafe {
                    RegisterHotKey(
                        Some(self.hwnd),
                        entry.id,
                        accelerator.modifiers | MOD_NOREPEAT,
                        accelerator.vk,
                    )
                };
                match result {
                    Ok(()) => entry.registered = true,
                    Err(e) => {
                        log!("RegisterHotKey({shortcut}) failed: {e}");
                        failures.push(Failure {
                            shortcut: shortcut.clone(),
                            reason: e.message(),
                            input_method: false,
                        });
                    }
                }
            } else if !should && entry.registered {
                unsafe {
                    let _ = UnregisterHotKey(Some(self.hwnd), entry.id);
                }
                entry.registered = false;
            }
        }
        failures
    }

    /// Unregisters everything (used on exit).
    pub fn clear(&mut self) {
        self.bind(&[]);
    }
}

/// Used for the example menu instead of Kando's default Ctrl+Space if an input method uses
/// Ctrl+Space.
const FALLBACK_SHORTCUT: &str = "Control+Alt+Space";

/// The shortcut for a new example menu: `preferred` unless an input method uses it.
pub fn default_shortcut(preferred: &str) -> String {
    let hotkeys = active_ime_hotkeys();
    [preferred, FALLBACK_SHORTCUT]
        .into_iter()
        .find(|s| parse(s).is_ok_and(|a| !hotkeys.iter().any(|h| ime_conflict(a, h))))
        .unwrap_or(preferred)
        .to_string()
}

/// Returns whether two accelerator strings refer to the same key combination (e.g.
/// `Control+Space` and `Ctrl+Space`).
pub fn same_shortcut(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (parse(a), parse(b)) {
        (Ok(acc_a), Ok(acc_b)) => acc_a == acc_b,
        _ => false,
    }
}

/// Formats a shortcut for messages, e.g. `Control+Space` as `Ctrl+Space`.
pub fn display(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "commandorcontrol" | "cmdorctrl" => "Ctrl",
            "command" | "cmd" | "super" | "meta" => "Win",
            _ => part,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// A hot key of an input method (Settings > Time & language > Typing > Advanced keyboard
/// settings > Input language hot keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ImeHotKey {
    id: u32,
    modifiers: u32,
    vk: u32,
}

const MOD_IGNORE_ALL_MODIFIER: u32 = 0x0400;

fn ime_conflict(accelerator: Accelerator, hotkey: &ImeHotKey) -> bool {
    hotkey.vk == accelerator.vk
        && (hotkey.modifiers & MOD_IGNORE_ALL_MODIFIER != 0
            || hotkey.modifiers & 0x7 == accelerator.modifiers.0 & 0xF)
}

/// The hot keys of a language only apply if an input method of that language is installed.
fn ime_hotkey_active(hotkey: &ImeHotKey, layouts: &[isize]) -> bool {
    let uses = |languages: &[isize]| layouts.iter().any(|l| languages.contains(&(l & 0xFFFF)));
    match hotkey.id {
        0x10..=0x2F => uses(&[0x0804, 0x1004]),
        0x30..=0x4F => uses(&[0x0411]),
        0x50..=0x6F => uses(&[0x0412]),
        0x70..=0x8F => uses(&[0x0404, 0x0C04, 0x1404]),
        _ => false,
    }
}

fn keyboard_layouts() -> Vec<isize> {
    unsafe {
        let count = GetKeyboardLayoutList(None).max(0) as usize;
        let mut layouts = vec![HKL::default(); count];
        let written = GetKeyboardLayoutList(Some(&mut layouts)).max(0) as usize;
        layouts.truncate(written);
        layouts.iter().map(|l| l.0 as isize).collect()
    }
}

/// The language hot keys of input methods which apply on this system (the id ranges of
/// IME_CHOTKEY, IME_JHOTKEY, IME_KHOTKEY and IME_THOTKEY in imm.h). Private hot keys of
/// input methods (IME_HOTKEY_PRIVATE) do not take precedence over RegisterHotKey
/// shortcuts (checked with Ctrl+Alt+K), so they are not reported.
fn active_ime_hotkeys() -> Vec<ImeHotKey> {
    let layouts = keyboard_layouts();
    (0x10..=0x8F)
        .filter_map(|id| {
            let (mut modifiers, mut vk, mut layout) = (0, 0, HKL::default());
            let found = unsafe { ImmGetHotKey(id, &mut modifiers, &mut vk, &mut layout) };
            found.as_bool().then_some(ImeHotKey { id, modifiers, vk })
        })
        .filter(|h| h.vk != 0 && ime_hotkey_active(h, &layouts))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_accelerators() {
        assert_eq!(
            parse("Control+Space"),
            Ok(Accelerator {
                modifiers: MOD_CONTROL,
                vk: 0x20
            })
        );
        assert_eq!(
            parse("Control+Shift+K"),
            Ok(Accelerator {
                modifiers: MOD_CONTROL | MOD_SHIFT,
                vk: 0x4B
            })
        );
        assert_eq!(
            parse("Alt+F4"),
            Ok(Accelerator {
                modifiers: MOD_ALT,
                vk: 0x73
            })
        );
        assert_eq!(
            parse("Meta+F24"),
            Ok(Accelerator {
                modifiers: MOD_WIN,
                vk: 0x87
            })
        );
        assert_eq!(parse("CommandOrControl+num5").unwrap().vk, 0x65);
        assert_eq!(parse("Ctrl+Esc").unwrap().vk, 0x1B);
        assert_eq!(
            parse("F9"),
            Ok(Accelerator {
                modifiers: HOT_KEY_MODIFIERS(0),
                vk: 0x78
            })
        );
        assert_eq!(parse("Control+7").unwrap().vk, 0x37);
        assert_eq!(parse("Control+MediaPlayPause").unwrap().vk, 0xB3);
    }

    #[test]
    fn detects_input_method_hot_keys() {
        let toggle = ImeHotKey {
            id: 0x10,
            modifiers: 0xC002,
            vk: 0x20,
        };
        assert!(ime_conflict(parse("Control+Space").unwrap(), &toggle));
        assert!(!ime_conflict(parse("Control+Alt+Space").unwrap(), &toggle));
        assert!(!ime_conflict(parse("Control+K").unwrap(), &toggle));
        let any_modifiers = ImeHotKey {
            modifiers: MOD_IGNORE_ALL_MODIFIER,
            ..toggle
        };
        assert!(ime_conflict(parse("Alt+Space").unwrap(), &any_modifiers));

        let chinese = [0x0804_0804_isize];
        let english = [0x0409_0409_isize];
        assert!(ime_hotkey_active(&toggle, &chinese));
        assert!(!ime_hotkey_active(&toggle, &english));
        let traditional = ImeHotKey { id: 0x70, ..toggle };
        assert!(!ime_hotkey_active(&traditional, &chinese));
        assert!(ime_hotkey_active(&traditional, &[0x0404_0404]));
        let private = ImeHotKey {
            id: 0x200,
            ..toggle
        };
        assert!(!ime_hotkey_active(&private, &chinese));
    }

    #[test]
    #[ignore = "depends on the input methods of this system"]
    fn prints_input_method_hot_keys() {
        for hotkey in active_ime_hotkeys() {
            println!("{hotkey:x?}");
        }
        println!("default shortcut: {}", default_shortcut("Control+Space"));
    }

    #[test]
    fn displays_shortcuts() {
        assert_eq!(display("Control+Alt+Space"), "Ctrl+Alt+Space");
        assert_eq!(display("CommandOrControl+Shift+K"), "Ctrl+Shift+K");
        assert_eq!(display("Super+F1"), "Win+F1");
    }

    #[test]
    fn rejects_invalid_accelerators() {
        assert!(parse("").is_err());
        assert!(parse("Control").is_err());
        assert!(parse("Control+A+B").is_err());
        assert!(parse("Control+Hyper").is_err());
        assert!(parse("F25").is_err());
    }
}
