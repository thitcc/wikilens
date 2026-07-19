//! Global hotkey handling: defaults, accelerator (de)serialization, display
//! labels, and (re)registration. The *current* combos live in the managed
//! `SettingsStore` (`settings.rs`) — the plugin handler in `lib.rs` matches
//! incoming shortcuts against those live values, so a `set_hotkey` swap takes
//! effect without a restart.
//
// Why the default summon combo is Ctrl+` (Backquote) and not a Shift+letter:
// a bare Shift+letter global hotkey (the original Shift+C) is registered via
// RegisterHotKey and swallows that letter system-wide — a capital 'C' could
// never be typed into the prompt (the capital-C draft-loss trap, defused at
// the root by this default). Backquote maps to VK_OEM_3, which sits at the
// physical key left of 1 on both US and ABNT2 layouts (verified on-device,
// see vault/2026-07-19_hotkey-config.md), and a Ctrl combo on it collides
// with no typed text.

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::error::AppError;
use crate::settings::{HotkeyRole, SettingsStore};

/// Default summon (overlay toggle) shortcut: Ctrl+`.
pub fn default_summon() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL), Code::Backquote)
}

/// Default region-capture shortcut: Ctrl+Shift+C.
pub fn default_capture() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC)
}

/// The built-in default for a role — the recorder's "Reset" target.
pub fn default_for(role: HotkeyRole) -> Shortcut {
    match role {
        HotkeyRole::Summon => default_summon(),
        HotkeyRole::Capture => default_capture(),
    }
}

/// Parse a stored/IPC accelerator string (e.g. `"Ctrl+Backquote"`).
/// `global-hotkey`'s parser is the authority; anything it rejects is invalid.
pub fn parse_accelerator(s: &str) -> Result<Shortcut, AppError> {
    s.parse::<Shortcut>()
        .map_err(|_| AppError::Hotkey("That key combination isn't valid.".to_string()))
}

/// Canonical accelerator form: `Ctrl+Alt+Shift+Super` order, then the `Code`
/// enum name — `"Ctrl+Backquote"`, `"Ctrl+Shift+KeyC"`. This exact format is
/// what the frontend recorder emits (`toAccelerator` in `src/hotkeys.ts` —
/// keep the two in sync), so equality checks are plain string compares.
pub fn to_accelerator(shortcut: &Shortcut) -> String {
    let mut out = String::new();
    for (bit, name) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Super"),
    ] {
        if shortcut.mods.contains(bit) {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(&format!("{:?}", shortcut.key));
    out
}

/// Player-facing label: `"Ctrl+`"`, `"Ctrl+Shift+C"`, `"F8"`. Modifier names
/// and the glyph table are paired with `labelParts` in `src/hotkeys.ts` —
/// keep the two in sync.
pub fn display_label(shortcut: &Shortcut) -> String {
    let mut out = String::new();
    for (bit, name) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Win"),
    ] {
        if shortcut.mods.contains(bit) {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(&key_glyph(shortcut.key));
    out
}

/// Glyph for a `Code`: `KeyC` → `C`, `Digit1` → `1`, `Backquote` → `` ` ``,
/// `F8` → `F8`; unmapped codes fall back to the raw enum name.
fn key_glyph(code: Code) -> String {
    let name = format!("{code:?}");
    if let Some(letter) = name.strip_prefix("Key").filter(|r| r.len() == 1) {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit").filter(|r| r.len() == 1) {
        return digit.to_string();
    }
    match code {
        Code::Backquote => "`",
        Code::Minus => "-",
        Code::Equal => "=",
        Code::BracketLeft => "[",
        Code::BracketRight => "]",
        Code::Backslash => "\\",
        Code::Semicolon => ";",
        Code::Quote => "'",
        Code::Comma => ",",
        Code::Period => ".",
        Code::Slash => "/",
        _ => return name,
    }
    .to_string()
}

/// Register both configured shortcuts at startup. The `SettingsStore` must
/// already be managed (setup order in `lib.rs`). Each registration is
/// independently non-fatal: a combo another app already owns is logged and
/// surfaced via the tray tooltip instead of crashing.
pub fn register(app: &AppHandle) {
    let hk = app.state::<SettingsStore>().hotkeys();
    register_one(app, hk.summon);
    register_one(app, hk.capture);
}

fn register_one(app: &AppHandle, shortcut: Shortcut) {
    let label = display_label(&shortcut);
    if let Err(e) = app.global_shortcut().register(shortcut) {
        eprintln!("[wikilens] failed to register {label} hotkey: {e}");
        tooltip_unavailable(app, &label);
    }
}

fn tooltip_unavailable(app: &AppHandle, label: &str) {
    if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!(
            "WikiLens — {label} is unavailable (another app may be using it)"
        )));
    }
}

fn claim_error(shortcut: &Shortcut) -> AppError {
    AppError::Hotkey(format!(
        "Couldn't claim {} — another app may already be using it.",
        display_label(shortcut)
    ))
}

/// Swap a live registration `old` → `new`. On registration failure the old
/// combo is restored (best-effort) so the app never ends up with neither.
/// The plugin flattens "already registered" into a string error, so all
/// failures share one user-readable message — no variant branching.
pub fn apply_change(app: &AppHandle, old: Shortcut, new: Shortcut) -> Result<(), AppError> {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister(old) {
        // Old may never have registered (startup collision) — log and go on.
        eprintln!(
            "[wikilens] couldn't unregister {}: {e}",
            display_label(&old)
        );
    }
    if let Err(e) = gs.register(new) {
        eprintln!("[wikilens] couldn't register {}: {e}", display_label(&new));
        if let Err(e2) = gs.register(old) {
            eprintln!(
                "[wikilens] rollback to {} also failed: {e2}",
                display_label(&old)
            );
        }
        return Err(claim_error(&new));
    }
    Ok(())
}

/// Try to claim a combo and release it immediately — instant "another app
/// owns it" feedback while the recorder has the real registrations suspended.
pub fn probe(app: &AppHandle, shortcut: Shortcut) -> Result<(), AppError> {
    let gs = app.global_shortcut();
    gs.register(shortcut).map_err(|e| {
        eprintln!(
            "[wikilens] probe of {} failed: {e}",
            display_label(&shortcut)
        );
        claim_error(&shortcut)
    })?;
    if let Err(e) = gs.unregister(shortcut) {
        eprintln!(
            "[wikilens] probe couldn't release {}: {e}",
            display_label(&shortcut)
        );
    }
    Ok(())
}

/// Drop both live registrations while the recorder is armed. Failures are
/// logged only — a leftover registration just means a press may still fire,
/// and the frontend's `overlay://hidden` disarm path contains that.
pub fn unregister_all(app: &AppHandle) {
    let hk = app.state::<SettingsStore>().hotkeys();
    let gs = app.global_shortcut();
    for s in [hk.summon, hk.capture] {
        if let Err(e) = gs.unregister(s) {
            eprintln!("[wikilens] couldn't suspend {}: {e}", display_label(&s));
        }
    }
}

/// Re-register both configured shortcuts after recording. Skips combos this
/// app still holds (a failed suspend). Failures aggregate into one
/// user-readable error and land on the tray tooltip, like startup.
pub fn register_all(app: &AppHandle) -> Result<(), AppError> {
    let hk = app.state::<SettingsStore>().hotkeys();
    let gs = app.global_shortcut();
    let mut failed: Vec<String> = Vec::new();
    for s in [hk.summon, hk.capture] {
        if gs.is_registered(s) {
            continue;
        }
        if let Err(e) = gs.register(s) {
            let label = display_label(&s);
            eprintln!("[wikilens] failed to re-register {label}: {e}");
            tooltip_unavailable(app, &label);
            failed.push(label);
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(AppError::Hotkey(format!(
            "Couldn't restore {} — another app may have taken it while recording.",
            failed.join(" and ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_from_their_canonical_strings_and_are_disjoint() {
        assert_eq!(parse_accelerator("Ctrl+Backquote").unwrap(), default_summon());
        assert_eq!(
            parse_accelerator("Ctrl+Shift+KeyC").unwrap(),
            default_capture()
        );
        // The settings loader's conflict repair terminates because of this.
        assert_ne!(default_summon(), default_capture());
    }

    #[test]
    fn to_accelerator_round_trips_through_the_parser() {
        // Every main key the frontend recorder can emit, across the modifier
        // sets it allows (F-keys may also come through bare).
        let codes = [
            Code::KeyA,
            Code::KeyC,
            Code::KeyZ,
            Code::Digit0,
            Code::Digit9,
            Code::F1,
            Code::F12,
            Code::Backquote,
            Code::Minus,
            Code::Equal,
            Code::BracketLeft,
            Code::BracketRight,
            Code::Backslash,
            Code::Semicolon,
            Code::Quote,
            Code::Comma,
            Code::Period,
            Code::Slash,
        ];
        let mod_sets = [
            Some(Modifiers::CONTROL),
            Some(Modifiers::ALT),
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
            None,
        ];
        for code in codes {
            for mods in mod_sets {
                let shortcut = Shortcut::new(mods, code);
                let accel = to_accelerator(&shortcut);
                assert_eq!(
                    parse_accelerator(&accel).unwrap(),
                    shortcut,
                    "round-trip failed for {accel:?}"
                );
            }
        }
    }

    #[test]
    fn unparseable_accelerators_are_hotkey_errors() {
        for bad in ["", "Ctrl+", "Ctrl+Shift", "NotAKey", "Ctrl+Kaboom"] {
            assert!(
                matches!(parse_accelerator(bad), Err(AppError::Hotkey(_))),
                "{bad:?} should not parse"
            );
        }
    }

    #[test]
    fn display_labels_use_player_glyphs() {
        let cases = [
            (default_summon(), "Ctrl+`"),
            (default_capture(), "Ctrl+Shift+C"),
            (Shortcut::new(None, Code::F8), "F8"),
            (Shortcut::new(Some(Modifiers::ALT), Code::Digit1), "Alt+1"),
            (
                Shortcut::new(
                    Some(Modifiers::CONTROL | Modifiers::ALT),
                    Code::BracketLeft,
                ),
                "Ctrl+Alt+[",
            ),
        ];
        for (shortcut, expected) in cases {
            assert_eq!(display_label(&shortcut), expected);
        }
    }
}
