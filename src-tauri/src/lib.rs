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
mod keys;
mod llm;
mod models;
mod providers;
mod settings;
mod state;
mod target;
#[cfg(test)]
mod test_support;
mod tray;
mod window;
mod wiki;

use tauri::{Emitter, Manager, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use state::AppState;
use wiki::user::UserWikiStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load a `.env` (dev convenience) before anything reads a provider key. Real
    // OS env vars are NOT overwritten, so exported/`setx` vars take precedence.
    // `.ok()` ignores "no .env found". See CLAUDE.md for the packaged-app caveat.
    dotenvy::dotenv().ok();
    // Returning-dev nudge: name any env var this build no longer reads
    // (vendor keys → the DPAPI store; rewrite pins → the picked model).
    target::warn_legacy_env();

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
        // The overlay's reported panel height (window.rs) — window-geometry
        // state, deliberately outside session-scoped AppState.
        .manage(window::OverlayHeight::default())
        .setup(|app| {
            let handle = app.handle();
            // Settings load first: tray creation and hotkey registration both
            // read the configured combos from the managed store. Stores live
            // here (not in AppState) because the path resolver needs the handle.
            let data_dir = app.path().app_data_dir()?;
            let settings = settings::SettingsStore::load(data_dir.join("settings.json"));
            // First-launch mode auto-sense: a complete WIKILENS_DEFAULT_* env
            // selects Default once; the stored choice wins forever after.
            // Best-effort — a read-only settings file must never abort
            // startup (both sides then degrade to Custom).
            if settings.mode().is_none() {
                let mode = if target::sense_default_targets().is_ok() {
                    settings::Mode::Default
                } else {
                    settings::Mode::Custom
                };
                if let Err(e) = settings.set_mode(mode) {
                    eprintln!("wikilens: couldn't persist the auto-sensed mode: {e}");
                }
            }
            app.manage(settings);
            // User-added wikis, persisted in the app-data dir.
            app.manage(UserWikiStore::load(data_dir.join("wikis.json")));
            // Panel-entered API keys, DPAPI-encrypted per Windows user
            // (keys.rs) — read by the key commands and at ask/model-list
            // time. Store-only: the vendor env keys are gone
            // (vault/2026-07-26_default-mode-and-byo-api-keys.md, phase 2).
            app.manage(keys::DpapiKeyStore::load(data_dir.join("keys.json")));
            // Only now do the overlay/capture webviews get built: both are
            // `"create": false` in tauri.conf.json because Tauri creates
            // `create: true` config windows BEFORE this hook runs, and the
            // frontend's boot invokes race setup's manage() calls — a
            // packaged build loads its bundled assets fast enough to win
            // ("state not managed for field `keys`"), while dev's slower
            // Vite loads always lost, hiding the race. Stores first, webviews
            // after — pinned in config_guardrails.rs; story in
            // vault/2026-08-02_packaged-boot-state-race.md.
            for config in app.config().app.windows.iter().filter(|w| !w.create) {
                let win = WebviewWindowBuilder::from_config(handle, config)?.build()?;
                // Created hidden; without this the first show gets DWM's
                // one-time open transition (fade + rise) layered over the
                // app's own entrance — the first summon after launch animated
                // differently from every later one. The debug window does the
                // same in its create().
                window::disable_os_open_transition(&win);
            }
            tray::create(handle)?;
            hotkey::register(handle);
            // Visual debug window — exists only when WIKILENS_DEBUG is truthy
            // at startup (env can't change mid-process, so existence always
            // agrees with the per-ask flag read in debug.rs).
            if debug::debug_enabled() {
                debug_window::create(handle)?;
            }
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
            commands::set_overlay_height,
            commands::debug_available,
            commands::toggle_debug_window,
            commands::get_settings,
            commands::set_hotkey,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::list_key_status,
            commands::set_api_key,
            commands::remove_api_key,
            commands::set_mode,
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
