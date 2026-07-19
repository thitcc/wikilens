//! System tray icon and menu. The app starts hidden and lives here; it only
//! exits via the tray's **Quit** item.

use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Wry,
};

use tauri::Manager;

use crate::settings::SettingsStore;
use crate::{debug, debug_window, hotkey, window};

/// Stable id so other modules (e.g. `hotkey`) can look the tray up.
pub const TRAY_ID: &str = "main";

/// Tooltip advertising the current summon combo. Derived from the managed
/// `SettingsStore`, so it must be created after the store (setup order in
/// `lib.rs`).
fn summon_tooltip(app: &AppHandle) -> String {
    let label = hotkey::display_label(&app.state::<SettingsStore>().hotkeys().summon);
    format!("WikiLens — press {label} to open")
}

/// Point the tooltip at the current summon combo. Called after a successful
/// hotkey change — which also clears a stale "unavailable" message.
pub fn update_summon_tooltip(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(summon_tooltip(app)));
    }
}

/// Build the tray icon + menu and attach event handlers.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show/Hide overlay", true, None::<&str>)?;
    // Present only when the debug window exists (WIKILENS_DEBUG at startup) —
    // its close button hides it, and this is the way back.
    let show_debug = if debug::debug_enabled() {
        Some(MenuItem::with_id(app, "show-debug", "Show debug panel", true, None::<&str>)?)
    } else {
        None
    };
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&toggle];
    if let Some(item) = &show_debug {
        items.push(item);
    }
    items.push(&separator);
    items.push(&quit);
    let menu = Menu::with_items(app, &items)?;

    // The icon is guaranteed by the `bundle.icon` list in tauri.conf.json.
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon is configured in tauri.conf.json");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(summon_tooltip(app))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => window::toggle_overlay(app),
            "show-debug" => debug_window::show(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click the tray icon also toggles the overlay.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_overlay(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
