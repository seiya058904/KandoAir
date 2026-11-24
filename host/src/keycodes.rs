// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! Maps DOM `KeyboardEvent.code` names (lower case) to Windows scan codes. Generated from
//! the Windows column of Kando's `src/common/key-codes.ts`. Codes above 0xFF carry the
//! 0xE0 prefix of extended keys.

/// Sorted by key name for binary search.
const SCAN_CODES: &[(&str, u16)] = &[
    ("altleft", 0x0038),
    ("altright", 0xE038),
    ("arrowdown", 0xE050),
    ("arrowleft", 0xE04B),
    ("arrowright", 0xE04D),
    ("arrowup", 0xE048),
    ("audiovolumedown", 0xE02E),
    ("audiovolumemute", 0xE020),
    ("audiovolumeup", 0xE030),
    ("backquote", 0x0029),
    ("backslash", 0x002B),
    ("backspace", 0x000E),
    ("bracketleft", 0x001A),
    ("bracketright", 0x001B),
    ("browserback", 0xE06A),
    ("browserfavorites", 0xE066),
    ("browserforward", 0xE069),
    ("browserhome", 0xE032),
    ("browserrefresh", 0xE067),
    ("browsersearch", 0xE065),
    ("browserstop", 0xE068),
    ("capslock", 0x003A),
    ("comma", 0x0033),
    ("contextmenu", 0xE05D),
    ("controlleft", 0x001D),
    ("controlright", 0xE01D),
    ("convert", 0x0079),
    ("copy", 0xE018),
    ("cut", 0xE017),
    ("delete", 0xE053),
    ("digit0", 0x000B),
    ("digit1", 0x0002),
    ("digit2", 0x0003),
    ("digit3", 0x0004),
    ("digit4", 0x0005),
    ("digit5", 0x0006),
    ("digit6", 0x0007),
    ("digit7", 0x0008),
    ("digit8", 0x0009),
    ("digit9", 0x000A),
    ("eject", 0xE02C),
    ("end", 0xE04F),
    ("enter", 0x001C),
    ("equal", 0x000D),
    ("escape", 0x0001),
    ("f1", 0x003B),
    ("f10", 0x0044),
    ("f11", 0x0057),
    ("f12", 0x0058),
    ("f13", 0x0064),
    ("f14", 0x0065),
    ("f15", 0x0066),
    ("f16", 0x0067),
    ("f17", 0x0068),
    ("f18", 0x0069),
    ("f19", 0x006A),
    ("f2", 0x003C),
    ("f20", 0x006B),
    ("f21", 0x006C),
    ("f22", 0x006D),
    ("f23", 0x006E),
    ("f24", 0x0076),
    ("f3", 0x003D),
    ("f4", 0x003E),
    ("f5", 0x003F),
    ("f6", 0x0040),
    ("f7", 0x0041),
    ("f8", 0x0042),
    ("f9", 0x0043),
    ("help", 0xE03B),
    ("home", 0xE047),
    ("insert", 0xE052),
    ("intlbackslash", 0x0056),
    ("intlro", 0x0073),
    ("intlyen", 0x007D),
    ("kanamode", 0x0070),
    ("keya", 0x001E),
    ("keyb", 0x0030),
    ("keyc", 0x002E),
    ("keyd", 0x0020),
    ("keye", 0x0012),
    ("keyf", 0x0021),
    ("keyg", 0x0022),
    ("keyh", 0x0023),
    ("keyi", 0x0017),
    ("keyj", 0x0024),
    ("keyk", 0x0025),
    ("keyl", 0x0026),
    ("keym", 0x0032),
    ("keyn", 0x0031),
    ("keyo", 0x0018),
    ("keyp", 0x0019),
    ("keyq", 0x0010),
    ("keyr", 0x0013),
    ("keys", 0x001F),
    ("keyt", 0x0014),
    ("keyu", 0x0016),
    ("keyv", 0x002F),
    ("keyw", 0x0011),
    ("keyx", 0x002D),
    ("keyy", 0x0015),
    ("keyz", 0x002C),
    ("lang1", 0x0072),
    ("lang2", 0x0071),
    ("lang3", 0x0078),
    ("lang4", 0x0077),
    ("launchapp1", 0xE06B),
    ("launchapp2", 0xE021),
    ("launchmail", 0xE06C),
    ("mediaplaypause", 0xE022),
    ("mediaselect", 0xE06D),
    ("mediastop", 0xE024),
    ("mediatracknext", 0xE019),
    ("mediatrackprevious", 0xE010),
    ("metaleft", 0xE05B),
    ("metaright", 0xE05C),
    ("minus", 0x000C),
    ("nonconvert", 0x007B),
    ("numlock", 0xE045),
    ("numpad0", 0x0052),
    ("numpad1", 0x004F),
    ("numpad2", 0x0050),
    ("numpad3", 0x0051),
    ("numpad4", 0x004B),
    ("numpad5", 0x004C),
    ("numpad6", 0x004D),
    ("numpad7", 0x0047),
    ("numpad8", 0x0048),
    ("numpad9", 0x0049),
    ("numpadadd", 0x004E),
    ("numpadcomma", 0x007E),
    ("numpaddecimal", 0x0053),
    ("numpaddivide", 0xE035),
    ("numpadenter", 0xE01C),
    ("numpadequal", 0x0059),
    ("numpadmultiply", 0x0037),
    ("numpadsubtract", 0x004A),
    ("pagedown", 0xE051),
    ("pageup", 0xE049),
    ("paste", 0xE00A),
    ("pause", 0x0045),
    ("period", 0x0034),
    ("power", 0xE05E),
    ("printscreen", 0xE037),
    ("quote", 0x0028),
    ("scrolllock", 0x0046),
    ("semicolon", 0x0027),
    ("shiftleft", 0x002A),
    ("shiftright", 0x0036),
    ("slash", 0x0035),
    ("sleep", 0xE05F),
    ("space", 0x0039),
    ("tab", 0x000F),
    ("undo", 0xE008),
    ("wakeup", 0xE063),
];

/// Returns the scan code for a DOM key code name such as `ControlLeft` or `KeyV`.
pub fn scan_code(name: &str) -> Option<u16> {
    let name = name.to_ascii_lowercase();
    SCAN_CODES
        .binary_search_by(|(key, _)| (*key).cmp(name.as_str()))
        .ok()
        .map(|index| SCAN_CODES[index].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        assert!(SCAN_CODES.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn known_keys() {
        assert_eq!(scan_code("ControlLeft"), Some(0x001D));
        assert_eq!(scan_code("KeyV"), Some(0x002F));
        assert_eq!(scan_code("AltLeft"), Some(0x0038));
        assert_eq!(scan_code("ArrowRight"), Some(0xE04D));
        assert_eq!(scan_code("MetaLeft"), Some(0xE05B));
        assert_eq!(scan_code("F4"), Some(0x003E));
        assert_eq!(scan_code("MediaPlayPause"), Some(0xE022));
        assert_eq!(scan_code("AudioVolumeMute"), Some(0xE020));
        assert_eq!(scan_code("Space"), Some(0x0039));
        assert_eq!(scan_code("Enter"), Some(0x001C));
        assert_eq!(scan_code("NoSuchKey"), None);
    }
}
