//! Global hotkey registration (Shift+C toggles the overlay).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

// TODO: make the toggle shortcut user-configurable (settings UI / config file).
//
// Caveat of the spec'd combo: Shift+C is a bare Shift+letter global hotkey. On
// Windows it is registered via RegisterHotKey, which swallows Shift+C system-wide
// while registered — so a capital 'C' can't be typed into the prompt (pressing it
// toggles the overlay instead), and an in-game Shift+C also toggles the panel.
// Wiki search is case-insensitive, so lowercase queries are unaffected. When this
// becomes configurable, prefer a Ctrl/Alt/Win-based combo (e.g. Ctrl+Shift+C).

/// The overlay toggle shortcut: Shift+C.
pub fn toggle_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SHIFT), Code::KeyC)
}

/// Whether an incoming shortcut is our toggle shortcut. Guards the shared
/// plugin handler in case more shortcuts are registered later.
pub fn is_toggle_shortcut(shortcut: &Shortcut) -> bool {
    shortcut.matches(Modifiers::SHIFT, Code::KeyC)
}

/// The region-capture shortcut: Ctrl+Shift+C. A Ctrl-based combo on purpose —
/// a bare Shift+letter would swallow that letter system-wide (the Shift+C
/// gotcha above), whereas Ctrl+Shift+C stays clear of typed text.
pub fn capture_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC)
}

/// Whether an incoming shortcut is the capture shortcut. Disjoint from
/// `is_toggle_shortcut`: `matches` compares the modifier set exactly, and the
/// toggle has no CONTROL bit, so Ctrl+Shift+C fires capture only.
pub fn is_capture_shortcut(shortcut: &Shortcut) -> bool {
    shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyC)
}

/// Register both global shortcuts (Shift+C toggle, Ctrl+Shift+C capture). The
/// actions happen in the plugin handler wired up in `lib.rs`. Each registration
/// is independently non-fatal: a combo another app already owns is logged and
/// surfaced via the tray tooltip instead of crashing.
pub fn register(app: &AppHandle) {
    register_one(app, toggle_shortcut(), "Shift+C");
    register_one(app, capture_shortcut(), "Ctrl+Shift+C");
}

fn register_one(app: &AppHandle, shortcut: Shortcut, label: &str) {
    if let Err(e) = app.global_shortcut().register(shortcut) {
        eprintln!("[wikilens] failed to register {label} hotkey: {e}");
        if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
            let _ = tray.set_tooltip(Some(format!(
                "WikiLens — {label} is unavailable (another app may be using it)"
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_capture_shortcuts_are_disjoint() {
        assert!(is_toggle_shortcut(&toggle_shortcut()));
        assert!(!is_capture_shortcut(&toggle_shortcut()));
        assert!(is_capture_shortcut(&capture_shortcut()));
        assert!(!is_toggle_shortcut(&capture_shortcut()));
    }
}
