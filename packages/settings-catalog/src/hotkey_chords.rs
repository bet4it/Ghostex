//! What a stored chord binds to on each platform: the Windows and Linux default that replaces a
//! saved macOS default, and the GPUI keystroke (`cmd-shift-p`) a chord turns into.
//!
//! The desktop binds through these two functions (`gpui_platform_hotkey_for_action` and
//! `gpui_keystroke_from_shared_hotkey` in apps/desktop/src/app/hotkeys.rs), so the tests in
//! `tests/platform_hotkeys.rs` can pin every platform's bindings without building the app.

use crate::hotkeys::hotkey_definition;
use crate::Platform;

/// The chord `key` runs on for `action_id` on `platform`: on Windows and Linux a chord equal to
/// the action's macOS default moves to its Windows and Linux default (an empty default leaves it
/// unassigned there); any other chord, and every chord on macOS, stays as saved.
///
/// CDXC:Hotkeys 2026-07-30:
/// Control is a distinct app modifier on macOS, while Windows and Linux use Ctrl as the primary modifier replacing Command. Every Mac default that is wrong elsewhere gets an explicit, conflict-free Windows and Linux chord, and a saved value that still equals the Mac default migrates to it. Custom bindings remain unchanged.
pub fn platform_hotkey<'a>(action_id: &str, key: &'a str, platform: Platform) -> &'a str {
    if platform == Platform::MacOs {
        return key;
    }
    match hotkey_definition(action_id) {
        Some(definition)
            if definition.windows_linux_default_key.is_some()
                && key.trim().eq_ignore_ascii_case(definition.default_key) =>
        {
            definition.windows_linux_default_key.unwrap_or(key)
        }
        _ => key,
    }
}

/// Converts a stored chord ("cmd+shift+p") into GPUI keystroke syntax ("cmd-shift-p" on macOS,
/// "ctrl-shift-p" on Windows and Linux, where `cmd` is Ctrl and `cmd+ctrl` is Ctrl+Alt). Returns
/// None for unbound or invalid chords and for chords without a non-Shift modifier, which must never
/// be taken from terminal or web surfaces, except on keys that never type text (Escape and F1-F24).
pub fn hotkey_keystroke(key: &str, platform: Platform) -> Option<String> {
    let mut command = false;
    let mut control = false;
    let mut option = false;
    let mut shift = false;
    let mut key_token: Option<String> = None;
    for token in key.split('+') {
        let token = token.trim().to_ascii_lowercase();
        if token.is_empty() {
            return None;
        }
        match token.as_str() {
            "cmd" | "command" | "meta" => command = true,
            "ctrl" | "control" => control = true,
            "alt" | "opt" | "option" => option = true,
            "shift" => shift = true,
            _ => {
                if key_token.is_some() {
                    return None;
                }
                key_token = Some(match token.as_str() {
                    "arrowup" => "up".to_string(),
                    "arrowdown" => "down".to_string(),
                    "arrowleft" => "left".to_string(),
                    "arrowright" => "right".to_string(),
                    "esc" => "escape".to_string(),
                    "return" => "enter".to_string(),
                    other => other.to_string(),
                });
            }
        }
    }
    let mut key_token = key_token?;
    if option && key_token == "ß" {
        /*
        CDXC:Hotkeys 2026-07-30:
        Older Settings builds persisted macOS Option+S as the produced `ß`
        character. GPUI receives the physical S key, so migrate that known
        recorder value at the settings boundary until the corrected recorder
        writes `alt+s`.
        */
        key_token = "s".to_string();
    }
    let mut modifiers = Vec::new();
    if platform == Platform::MacOs {
        if command {
            modifiers.push("cmd");
        }
        if control {
            modifiers.push("ctrl");
        }
    } else {
        if command {
            modifiers.push("ctrl");
        }
        if control {
            modifiers.push(if command { "alt" } else { "ctrl" });
        }
    }
    if option && !modifiers.contains(&"alt") {
        modifiers.push("alt");
    }
    if shift {
        modifiers.push("shift");
    }
    // CDXC:Hotkeys 2026-09-27 WHY:
    // Focus Chat Box is Shift+Esc, and dropping every Shift-only chord here left it with no binding at all: the native router matched the chord, but the window had nothing to run, so the key did nothing. Escape and the F-keys never type text, so they may go without Cmd, Ctrl or Alt; typing keys still may not.
    let non_typing_key = key_token == "escape"
        || key_token
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number));
    if !non_typing_key && !modifiers.iter().any(|modifier| *modifier != "shift") {
        return None;
    }
    modifiers.push(&key_token);
    Some(modifiers.join("-"))
}
