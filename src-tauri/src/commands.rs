//! Tauri commands — the only surface the frontend can call. Each returns a
//! user-readable `String` on error so the UI can render it verbatim.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::state::AppState;
use crate::wiki::{fetch, games, search};
use crate::{llm, window};

/// A supported game, as sent to the frontend game picker.
#[derive(Debug, Clone, Serialize)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
}

/// A wiki page used as a source for an answer.
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub title: String,
    pub url: String,
}

/// Final result of an `ask`: the full answer plus the pages it drew from.
#[derive(Debug, Clone, Serialize)]
pub struct AskResult {
    pub answer: String,
    pub sources: Vec<Source>,
}

/// List the supported games (id + display name).
#[tauri::command]
pub fn list_games() -> Vec<GameInfo> {
    games::GAMES
        .iter()
        .map(|g| GameInfo {
            id: g.id.to_string(),
            name: g.name.to_string(),
        })
        .collect()
}

/// Hide the overlay (used by the frontend `Esc` handler).
#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    window::hide_overlay(&app);
}

/// Answer a question about a game using its wiki as the source of truth.
///
/// Emits progress events the UI listens for:
/// - `ask://status` — `"searching"` → `"reading"` → `"answering"`
/// - `ask://delta` — streamed answer text chunks
///
/// Only one `ask` runs at a time; a concurrent call is rejected.
#[tauri::command]
pub async fn ask(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
    question: String,
) -> Result<AskResult, String> {
    // Concurrency guard: claim the slot, or reject if one is already running.
    if state.ask_in_progress.swap(true, Ordering::SeqCst) {
        return Err("A question is already in progress".to_string());
    }
    // Releases the slot on every exit path, including cancellation (drop).
    let _guard = AskGuard(&state.ask_in_progress);

    run_ask(&app, &state, &game_id, &question)
        .await
        .map_err(String::from)
}

/// Resets `ask_in_progress` when dropped, so a panic or a cancelled future can't
/// leave the guard stuck.
struct AskGuard<'a>(&'a AtomicBool);

impl Drop for AskGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn run_ask(
    app: &AppHandle,
    state: &AppState,
    game_id: &str,
    question: &str,
) -> Result<AskResult, AppError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(AppError::EmptyQuestion);
    }
    let wiki = games::find_game(game_id).ok_or_else(|| AppError::UnknownGame(game_id.to_string()))?;
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or(AppError::MissingApiKey)?;

    let _ = app.emit("ask://status", "searching");
    let titles = search::search(&state.http, wiki, question, search::DEFAULT_SEARCH_LIMIT).await?;
    if titles.is_empty() {
        return Ok(AskResult {
            answer: format!(
                "I couldn't find anything on the {} wiki for that. Try rephrasing with different keywords.",
                wiki.name
            ),
            sources: Vec::new(),
        });
    }

    let _ = app.emit("ask://status", "reading");
    let pages = fetch::fetch_pages(&state.http, wiki, &titles).await?;
    if pages.is_empty() {
        return Ok(AskResult {
            answer: "I found matching pages but couldn't read their contents. Please try again."
                .to_string(),
            sources: Vec::new(),
        });
    }

    let _ = app.emit("ask://status", "answering");
    let delta_app = app.clone();
    let answer = llm::answer_streaming(&state.http, &api_key, question, &pages, move |delta| {
        let _ = delta_app.emit("ask://delta", delta);
    })
    .await?;

    let sources = pages
        .iter()
        .map(|p| Source {
            title: p.title.clone(),
            url: p.url.clone(),
        })
        .collect();

    Ok(AskResult { answer, sources })
}
