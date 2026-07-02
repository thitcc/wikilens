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

/// Register Shift+C. The actual toggle happens in the plugin handler wired up in
/// `lib.rs`. Registration failure (e.g. another app owns the combo) is non-fatal:
/// we log a warning and surface it via the tray tooltip instead of crashing.
pub fn register(app: &AppHandle) {
    if let Err(e) = app.global_shortcut().register(toggle_shortcut()) {
        eprintln!("[wikilens] failed to register Shift+C hotkey: {e}");
        if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
            let _ = tray.set_tooltip(Some(
                "WikiLens — Shift+C is unavailable (another app may be using it)",
            ));
        }
    }
}
