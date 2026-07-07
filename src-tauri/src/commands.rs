//! Tauri commands — the only surface the frontend can call. Each returns a
//! user-readable `String` on error so the UI can render it verbatim.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::capture::{self, CropRect};
use crate::error::AppError;
use crate::models::{self, ModelInfo, ModelSource};
use crate::state::AppState;
use crate::wiki::games::GameWiki;
use crate::wiki::user::UserWikiStore;
use crate::wiki::{fetch, games, probe, search};
use crate::{llm, providers, window};

/// A supported game, as sent to the frontend game picker.
#[derive(Debug, Clone, Serialize)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
    /// `true` for user-added wikis (removable); `false` for built-ins.
    pub custom: bool,
}

/// A supported LLM provider, as sent to the frontend. Carries the resolved
/// default model (env override applied) so the footer chip can label itself
/// before any model list is fetched. It still never reports which API keys
/// are configured.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub default_model: String,
    pub default_model_label: String,
}

/// A provider's model list plus where it came from. `"fallback"` means the
/// curated built-in list (no key, fetch failed, or offline) — the UI shows a
/// muted "offline list" note without learning why.
#[derive(Debug, Clone, Serialize)]
pub struct ModelList {
    pub models: Vec<ModelInfo>,
    pub source: ModelSource,
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

/// List the supported games: built-ins in curated order, then user-added
/// wikis (name-sorted by the store).
#[tauri::command]
pub fn list_games(store: State<'_, UserWikiStore>) -> Vec<GameInfo> {
    let builtin = games::GAMES.iter().map(|g| GameInfo {
        id: g.id.clone(),
        name: g.name.clone(),
        custom: false,
    });
    let custom = store
        .list()
        .into_iter()
        // A stored id can collide with a *later-shipped* built-in. The
        // built-in wins everywhere (run_ask resolves it first), so hide the
        // stale copy rather than showing two identical picker entries.
        .filter(|g| games::find_game(&g.id).is_none())
        .map(|g| GameInfo {
            id: g.id,
            name: g.name,
            custom: true,
        });
    builtin.chain(custom).collect()
}

/// Probe the common wiki hosts (wiki.gg, Fandom) for a game name, returning
/// only verified wikis. Sequential probes — may take a few seconds.
#[tauri::command]
pub async fn suggest_wikis(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<probe::WikiCandidate>, String> {
    if name.trim().is_empty() {
        return Err("Type a game name first.".to_string());
    }
    Ok(probe::suggest(&state.http, name.trim()).await)
}

/// Validate a wiki URL and save it as a user-added game. Always re-probes the
/// URL it receives — whether it came from a suggestion click or a manual
/// paste — so there is exactly one validation path, and the stored endpoints
/// are always derived from the wiki's own siteinfo.
#[tauri::command]
pub async fn add_game(
    state: State<'_, AppState>,
    store: State<'_, UserWikiStore>,
    name: String,
    url: String,
) -> Result<GameInfo, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Give the game a name first.".to_string());
    }
    if url.trim().is_empty() {
        return Err("Pick a suggestion or paste the wiki's address.".to_string());
    }
    let id = probe::slugify(&name);
    if id.is_empty() {
        return Err("The name needs at least one letter or digit.".to_string());
    }
    if games::find_game(&id).is_some() {
        return Err(format!(
            "WikiLens already includes {name} — to use a different wiki for it, give it a different name."
        ));
    }

    let candidate = probe::probe_base(&state.http, &url)
        .await
        .map_err(String::from)?;
    store
        .add(GameWiki {
            id: id.clone(),
            name: name.clone(),
            api_url: candidate.api_url,
            page_url: candidate.page_url,
            search_namespace: None,
        })
        .map_err(String::from)?;

    Ok(GameInfo {
        id,
        name,
        custom: true,
    })
}

/// Remove a user-added game. Built-ins are refused.
#[tauri::command]
pub fn remove_game(store: State<'_, UserWikiStore>, id: String) -> Result<(), String> {
    // Store first: a stored entry must always be removable, even when a
    // later release ships a built-in with the same id — checking built-ins
    // first would lock the stale entry in forever.
    match store.remove(&id) {
        Ok(()) => Ok(()),
        Err(AppError::UnknownGame(_)) if games::find_game(&id).is_some() => {
            Err("Built-in games can't be removed.".to_string())
        }
        Err(e) => Err(String::from(e)),
    }
}

/// List the supported LLM providers with their resolved default models.
#[tauri::command]
pub fn list_providers() -> Vec<ProviderInfo> {
    providers::PROVIDERS
        .iter()
        .map(|p| {
            let default_model = p.model();
            let default_model_label = p.model_label(&default_model).to_string();
            ProviderInfo {
                id: p.id.to_string(),
                name: p.name.to_string(),
                default_model,
                default_model_label,
            }
        })
        .collect()
}

/// List a provider's selectable models: the session-cached live list when one
/// exists, else a fresh fetch, else the curated fallback (see
/// `models::resolve_model_list`). Fallbacks are never cached, so a transient
/// failure retries on the next menu open.
#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ModelList, String> {
    let provider = providers::find_provider(&provider_id)
        .ok_or_else(|| String::from(AppError::UnknownProvider(provider_id.clone())))?;

    // Cache hit — only live lists are ever stored here. The guard clones and
    // drops the lock; it is never held across an await.
    let cached = state
        .models_cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(provider.id)
        .cloned();
    if let Some(models) = cached {
        return Ok(ModelList {
            models,
            source: ModelSource::Live,
        });
    }

    let api_key = provider.api_key();
    let live = if provider.models_need_key && api_key.is_none() {
        // The endpoint would 401 — skip the doomed round trip and degrade.
        Err(AppError::MissingApiKey {
            provider: provider.name,
            env_var: provider.api_key_env,
        })
    } else {
        // Keyless endpoints (OpenRouter) are always called unauthenticated —
        // never send a key where it isn't needed.
        let key = if provider.models_need_key {
            api_key.as_deref()
        } else {
            None
        };
        models::fetch_models(&state.http, provider, key).await
    };

    let (list, source) = models::resolve_model_list(live, provider.curated_models);
    if source == ModelSource::Live {
        state
            .models_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(provider.id, list.clone());
    }
    Ok(ModelList {
        models: list,
        source,
    })
}

/// Hide the overlay (used by the frontend `Esc` handler).
#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    window::hide_overlay(&app);
}

/// Start a region capture: hide the panel, freeze the monitor under the cursor,
/// and show the capture overlay. Rejected while an `ask` is running — attaching
/// a new image mid-request would race the attachment slot.
#[tauri::command]
pub async fn begin_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.ask_in_progress.load(Ordering::SeqCst) {
        return Err("Finish the current question before taking a screenshot.".to_string());
    }
    capture::begin(&app, &state).await.map_err(String::from)
}

/// Finish a capture: crop the frozen shot to the dragged region, store it as
/// the attachment, and restore the panel. Invoked by the capture webview, so
/// the result is delivered to the overlay via events rather than the return
/// value: `capture://attached` (an `AttachmentInfo`) on success, else
/// `capture://error` (a message) — the overlay owns the prompt strip and error
/// box, not the capture window.
#[tauri::command]
pub fn finish_capture(app: AppHandle, state: State<'_, AppState>, rect: CropRect) {
    match capture::finish(&app, &state, rect) {
        Ok(info) => {
            let _ = app.emit_to(window::OVERLAY_LABEL, "capture://attached", info);
        }
        Err(e) => {
            let _ = app.emit_to(window::OVERLAY_LABEL, "capture://error", String::from(e));
        }
    }
}

/// Cancel an in-progress capture (Esc / click-away / Alt-Tab in the capture
/// overlay): tear it down and restore the panel. Any existing attachment stays.
#[tauri::command]
pub fn cancel_capture(app: AppHandle, state: State<'_, AppState>) {
    capture::cancel(&app, &state);
}

/// Drop the attached screenshot (the prompt strip's "×").
#[tauri::command]
pub fn clear_capture(state: State<'_, AppState>) {
    capture::clear(&state);
}

/// Answer a question about a game using its wiki as the source of truth.
///
/// Emits progress events the UI listens for:
/// - `ask://status` — `"searching"` → optional `"retrying"` → `"reading"` → `"answering"`
/// - `ask://delta` — streamed answer text chunks
///
/// Only one `ask` runs at a time; a concurrent call is rejected.
///
/// `image_id` optionally names an attached screenshot (from `finish_capture`);
/// a mismatch with the stored attachment fails fast as "capture it again". The
/// attachment is cleared only after the model actually answers.
#[tauri::command]
pub async fn ask(
    app: AppHandle,
    state: State<'_, AppState>,
    store: State<'_, UserWikiStore>,
    game_id: String,
    provider_id: String,
    model: String,
    question: String,
    image_id: Option<String>,
) -> Result<AskResult, String> {
    // Concurrency guard: claim the slot, or reject if one is already running.
    if state.ask_in_progress.swap(true, Ordering::SeqCst) {
        return Err("A question is already in progress".to_string());
    }
    // Releases the slot on every exit path, including cancellation (drop).
    let _guard = AskGuard(&state.ask_in_progress);

    run_ask(
        &app,
        &state,
        &store,
        &game_id,
        &provider_id,
        &model,
        &question,
        image_id.as_deref(),
    )
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

#[allow(clippy::too_many_arguments)]
async fn run_ask(
    app: &AppHandle,
    state: &AppState,
    store: &UserWikiStore,
    game_id: &str,
    provider_id: &str,
    model: &str,
    question: &str,
    image_id: Option<&str>,
) -> Result<AskResult, AppError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(AppError::EmptyQuestion);
    }
    // Resolve any attached screenshot before any network work, so a stale id
    // fails instantly ("capture it again") rather than after searching. `None`
    // when no image is attached — the request then stays text-only.
    let image_png = capture::resolve_image(state, image_id)?;
    // Built-ins first, then the user store; the owned clone means a game
    // removed mid-ask can't be yanked out from under this run.
    let wiki = games::find_game(game_id)
        .cloned()
        .or_else(|| store.get(game_id))
        .ok_or_else(|| AppError::UnknownGame(game_id.to_string()))?;

    // Resolve provider → key → model up front, before any status event, so a
    // missing key fails instantly (no stuck "Searching…"). The AskGuard still
    // releases the concurrency slot on this early return.
    // The frontend always sends a provider, but fall back to the default if it
    // ever sends a blank one (commands are a trust boundary).
    let provider_id = if provider_id.trim().is_empty() {
        providers::DEFAULT_PROVIDER_ID
    } else {
        provider_id
    };
    let provider = providers::find_provider(provider_id)
        .ok_or_else(|| AppError::UnknownProvider(provider_id.to_string()))?;
    let api_key = provider.api_key().ok_or(AppError::MissingApiKey {
        provider: provider.name,
        env_var: provider.api_key_env,
    })?;
    let model = effective_model(model, provider.model());

    let _ = app.emit("ask://status", "searching");
    // The wiki search gets a keyword-stripped query; the LLM still receives the
    // original question below.
    let query = search::preprocess_query(question);
    let mut titles =
        search::search(&state.http, &wiki, &query, search::DEFAULT_SEARCH_LIMIT).await?;
    if titles.is_empty() {
        // One bounded retry with a harsher keyword pass — a single extra request,
        // only on the zero-hit path (MediaWiki etiquette).
        let simplified = search::simplify_query(question);
        if !simplified.is_empty() && simplified != query {
            let _ = app.emit("ask://status", "retrying");
            titles =
                search::search(&state.http, &wiki, &simplified, search::DEFAULT_SEARCH_LIMIT)
                    .await?;
        }
    }
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
    let pages = fetch::fetch_pages(&state.http, &wiki, &titles).await?;
    if pages.is_empty() {
        return Ok(AskResult {
            answer: "I found matching pages but couldn't read their contents. Please try again."
                .to_string(),
            sources: Vec::new(),
        });
    }

    let _ = app.emit("ask://status", "answering");
    let delta_app = app.clone();
    let answer = llm::answer_streaming(
        &state.http,
        provider,
        &model,
        &api_key,
        question,
        &pages,
        image_png.as_deref(),
        move |delta| {
            let _ = delta_app.emit("ask://delta", delta);
        },
    )
    .await?;

    // The model answered — consume the attachment (clears-on-success). Every
    // earlier `?`/return keeps it, so a failed or empty ask leaves the shot
    // attached for a retry (stays-on-error).
    capture::clear(state);

    let sources = pages
        .iter()
        .map(|p| Source {
            title: p.title.clone(),
            url: p.url.clone(),
        })
        .collect();

    Ok(AskResult { answer, sources })
}

/// The model an `ask` should use: the frontend's requested id, or the
/// provider's resolved default when blank. Deliberately NOT validated against
/// any model list — a cold cache or an env override would make that check
/// wrong, and the provider is the authoritative validator anyway (a stale id
/// surfaces as the `AppError::Llm` message in the error box).
fn effective_model(requested: &str, fallback: String) -> String {
    let trimmed = requested.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_model_prefers_trimmed_request() {
        assert_eq!(
            effective_model("  claude-sonnet-5  ", "default-model".to_string()),
            "claude-sonnet-5"
        );
    }

    #[test]
    fn effective_model_falls_back_when_blank() {
        assert_eq!(effective_model("", "default-model".to_string()), "default-model");
        assert_eq!(effective_model("   ", "default-model".to_string()), "default-model");
    }
}
