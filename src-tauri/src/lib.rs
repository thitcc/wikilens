//! WikiLens — a Windows desktop overlay for wiki-heavy games.
//!
//! Flow: global hotkey → toggle overlay window → frontend prompt → `ask`
//! command → wiki search/fetch → LLM stream → events → UI.

mod capture;
mod commands;
#[cfg(test)]
mod config_guardrails;
mod debug;
mod debug_window;
mod error;
mod hotkey;
mod http;
mod llm;
mod models;
mod providers;
mod settings;
mod state;
#[cfg(test)]
mod test_support;
mod tray;
mod window;
mod wiki;

use tauri::{Emitter, Manager, WindowEvent};
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
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    // The live combos come from the managed store (loaded in
                    // setup before any registration, so a routed shortcut
                    // always finds it — try_state is belt-and-braces).
                    let Some(settings) = app.try_state::<settings::SettingsStore>() else {
                        return;
                    };
                    match settings.role_of(shortcut) {
                        Some(settings::HotkeyRole::Summon) => window::toggle_overlay(app),
                        Some(settings::HotkeyRole::Capture) => {
                            // Route capture through the overlay webview so the
                            // frontend stays the single entry point (where the
                            // guardrails plan hangs its gating); it invokes
                            // begin_capture. Delivered even while the panel is
                            // hidden — the webview stays mounted and listening.
                            let _ = app.emit_to(window::OVERLAY_LABEL, "capture://hotkey", ());
                        }
                        None => {}
                    }
                })
                .build(),
        )
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle();
            // Settings load first: tray creation and hotkey registration both
            // read the configured combos from the managed store. Stores live
            // here (not in AppState) because the path resolver needs the handle.
            let data_dir = app.path().app_data_dir()?;
            app.manage(settings::SettingsStore::load(data_dir.join("settings.json")));
            tray::create(handle)?;
            hotkey::register(handle);
            // Visual debug window — exists only when WIKILENS_DEBUG is truthy
            // at startup (env can't change mid-process, so existence always
            // agrees with the per-ask flag read in debug.rs).
            if debug::debug_enabled() {
                debug_window::create(handle)?;
            }
            // User-added wikis, persisted in the app-data dir.
            app.manage(UserWikiStore::load(data_dir.join("wikis.json")));
            Ok(())
        })
        .on_window_event(|win, event| {
            // Closing the window (e.g. Alt+F4) hides it instead of quitting; the
            // app only exits via the tray's Quit item. The overlay routes
            // through window::hide_overlay so this path fires overlay://hidden
            // like every other hide.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if win.label() == window::OVERLAY_LABEL {
                    window::hide_overlay(win.app_handle());
                } else {
                    let _ = win.hide();
                }
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
            commands::show_overlay,
            commands::debug_available,
            commands::toggle_debug_window,
            commands::get_settings,
            commands::set_hotkey,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::begin_capture,
            commands::finish_capture,
            commands::cancel_capture,
            commands::clear_capture,
            commands::ask,
            commands::cancel_ask,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
