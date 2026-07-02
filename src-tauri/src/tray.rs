//! System tray icon and menu. The app starts hidden and lives here; it only
//! exits via the tray's **Quit** item.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

use crate::window;

/// Stable id so other modules (e.g. `hotkey`) can look the tray up.
pub const TRAY_ID: &str = "main";

/// Build the tray icon + menu and attach event handlers.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show/Hide overlay", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &separator, &quit])?;

    // The icon is guaranteed by the `bundle.icon` list in tauri.conf.json.
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon is configured in tauri.conf.json");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("WikiLens — press Shift+C to open")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => window::toggle_overlay(app),
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
