//! WikiLens — a Windows desktop overlay for wiki-heavy games.
//!
//! Flow: global hotkey → toggle overlay window → frontend prompt → `ask`
//! command → wiki search/fetch → LLM stream → events → UI.

mod commands;
mod error;
mod hotkey;
mod llm;
mod models;
mod providers;
mod state;
mod tray;
mod window;
mod wiki;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use state::AppState;
use wiki::user::UserWikiStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load a `.env` (dev convenience) before anything reads a provider key. Real
    // OS env vars are NOT overwritten, so exported/`setx` vars take precedence.
    // `.ok()` ignores "no .env found". See CLAUDE.md for the packaged-app caveat.
    dotenvy::dotenv().ok();

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
            // User-added wikis, persisted in the app-data dir. Loaded here
            // (not in AppState) because the path resolver needs the handle.
            let data_dir = app.path().app_data_dir()?;
            app.manage(UserWikiStore::load(data_dir.join("wikis.json")));
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
            commands::suggest_wikis,
            commands::add_game,
            commands::remove_game,
            commands::list_providers,
            commands::list_models,
            commands::hide_overlay,
            commands::ask,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
