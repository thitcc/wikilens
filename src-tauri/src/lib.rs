//! WikiLens — a Windows desktop overlay for wiki-heavy games.
//!
//! Flow: global hotkey → toggle overlay window → frontend prompt → `ask`
//! command → wiki search/fetch → LLM stream → events → UI.

mod commands;
mod error;
mod hotkey;
mod llm;
mod state;
mod tray;
mod window;
mod wiki;

use tauri::WindowEvent;
use tauri_plugin_global_shortcut::ShortcutState;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Act on press only ("release-safe"); ignore the release event.
                    if event.state == ShortcutState::Pressed && hotkey::is_toggle_shortcut(shortcut) {
                        window::toggle_overlay(app);
                    }
                })
                .build(),
        )
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle();
            tray::create(handle)?;
            hotkey::register(handle);
            Ok(())
        })
        .on_window_event(|win, event| {
            // Closing the window (e.g. Alt+F4) hides it instead of quitting; the
            // app only exits via the tray's Quit item.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = win.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::hide_overlay,
            commands::ask,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
