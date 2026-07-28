//! Tauri commands — the only surface the frontend can call. Each returns a
//! user-readable `String` on error so the UI can render it verbatim.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::capture::{self, CropRect};
use crate::debug::DebugReport;
use crate::error::AppError;
use crate::keys::{DpapiKeyStore, KeyStore};
use crate::models::{self, ModelInfo, ModelSource};
use crate::settings::{HotkeyRole, Mode, SettingsStore};
use crate::target::{self, AskTargets, LlmTarget};
use crate::state::AppState;
use crate::wiki::games::GameWiki;
use crate::wiki::user::UserWikiStore;
use crate::wiki::{fetch, games, probe, search, titles};
use crate::{hotkey, llm, providers, window};

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
/// before any model list is fetched. Key *presence* crosses IPC separately
/// (`KeyStatus`); key material never does.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub default_model: String,
    pub default_model_label: String,
    /// Whether the resolved default model accepts image input — lets the
    /// guardrails plan resolve the never-opened-menu case with no fetch.
    pub default_model_vision: bool,
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

/// One configurable shortcut, as sent to the frontend settings popover. The
/// default rides along so the UI can offer "Reset" with no extra command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyInfo {
    /// Canonical accelerator form (`hotkey::to_accelerator`) — the identity
    /// the recorder compares against.
    pub accelerator: String,
    /// Player-facing label, e.g. "Ctrl+`".
    pub label: String,
    pub default_accelerator: String,
    pub default_label: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotkeysInfo {
    pub summon: HotkeyInfo,
    pub capture: HotkeyInfo,
}

/// One provider's key *presence*, for the settings panel. Never key material —
/// the guardrails pin the field set and sentinel-check the serialization.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub id: String,
    pub name: String,
    pub has_key: bool,
}

/// Whether the packaged Default model source is configured in this
/// environment (`WIKILENS_DEFAULT_*` — configured means the resolver
/// succeeds), and whether it reads images. Sensed per command call.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DefaultModeInfo {
    pub configured: bool,
    pub vision: bool,
}

/// Extensible settings envelope — future config-panel tenants join here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInfo {
    pub hotkeys: HotkeysInfo,
    /// The persisted model-source choice; `None` (→ JSON null) = never chosen.
    pub mode: Option<Mode>,
    pub default_mode: DefaultModeInfo,
}

fn hotkey_info(settings: &SettingsStore, role: HotkeyRole) -> HotkeyInfo {
    let current = settings.shortcut(role);
    let default = hotkey::default_for(role);
    HotkeyInfo {
        accelerator: hotkey::to_accelerator(&current),
        label: hotkey::display_label(&current),
        default_accelerator: hotkey::to_accelerator(&default),
        default_label: hotkey::display_label(&default),
        is_default: current == default,
    }
}

/// Pure over its inputs (the guardrail pins call it with a fixed
/// `DefaultModeInfo`); commands pass `sense_default_mode()`.
pub(crate) fn settings_info(settings: &SettingsStore, default_mode: DefaultModeInfo) -> SettingsInfo {
    SettingsInfo {
        hotkeys: HotkeysInfo {
            summon: hotkey_info(settings, HotkeyRole::Summon),
            capture: hotkey_info(settings, HotkeyRole::Capture),
        },
        mode: settings.mode(),
        default_mode,
    }
}

/// Pure over an injected env lookup — the multi-threaded suite never mutates
/// env, so tests drive this with closures. `configured` is resolver success
/// (not mere var presence), so the panel's "isn't set up" note and the ask
/// path's error can never disagree — an invalid `_API_PROVIDER` counts as
/// unconfigured here too.
fn default_mode_info(env: impl Fn(&str) -> Option<String>) -> DefaultModeInfo {
    DefaultModeInfo {
        configured: crate::target::resolve_default_targets(&env).is_ok(),
        // Vision is opt-in (no id heuristic exists for an arbitrary target);
        // same truthy semantics as WIKILENS_DEBUG.
        vision: env("WIKILENS_DEFAULT_VISION").is_some_and(|v| crate::debug::is_truthy(&v)),
    }
}

/// The impure half: real env reads (`env_nonempty` trims and drops blanks).
fn sense_default_mode() -> DefaultModeInfo {
    default_mode_info(providers::env_nonempty)
}

/// One row per registry provider, in registry order. Presence only — reading
/// `has_key` never decrypts (keys.rs).
pub(crate) fn key_status(keys: &impl KeyStore) -> Vec<KeyStatus> {
    providers::PROVIDERS
        .iter()
        .map(|p| KeyStatus {
            id: p.id.to_string(),
            name: p.name.to_string(),
            has_key: keys.has_key(p.id),
        })
        .collect()
}

fn set_key(keys: &impl KeyStore, provider_id: &str, key: &str) -> Result<Vec<KeyStatus>, String> {
    let provider = providers::find_provider(provider_id)
        .ok_or_else(|| String::from(AppError::UnknownProvider(provider_id.to_string())))?;
    // Trim here: a pasted trailing newline would poison the auth header at
    // ask time. Only the trimmed key is stored.
    let key = key.trim();
    if key.is_empty() {
        return Err("Paste an API key first.".to_string());
    }
    keys.set(provider.id, key).map_err(String::from)?;
    Ok(key_status(keys))
}

fn remove_key(keys: &impl KeyStore, provider_id: &str) -> Result<Vec<KeyStatus>, String> {
    let provider = providers::find_provider(provider_id)
        .ok_or_else(|| String::from(AppError::UnknownProvider(provider_id.to_string())))?;
    // An absent key is the trait's documented no-op leg — a double-clicked
    // Remove stays quiet.
    keys.remove(provider.id).map_err(String::from)?;
    Ok(key_status(keys))
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

/// The keyed providers, in registry order — Custom mode's menu shows only
/// providers the player can actually use. Presence check only; never decrypts.
pub(crate) fn provider_infos(keys: &impl KeyStore) -> Vec<ProviderInfo> {
    providers::PROVIDERS
        .iter()
        .filter(|p| keys.has_key(p.id))
        .map(|p| {
            let default_model = p.model();
            let default_model_label = p.model_label(&default_model).to_string();
            // Curated lookup; an unknown env-override id defaults to vision only
            // for Anthropic (its whole catalog is vision), false elsewhere.
            let default_model_vision = p
                .model_vision(&default_model)
                .unwrap_or(p.kind == providers::ProviderKind::Anthropic);
            ProviderInfo {
                id: p.id.to_string(),
                name: p.name.to_string(),
                default_model,
                default_model_label,
                default_model_vision,
            }
        })
        .collect()
}

/// List the LLM providers with a stored key (Custom mode's picker source).
#[tauri::command]
pub fn list_providers(keys: State<'_, DpapiKeyStore>) -> Vec<ProviderInfo> {
    provider_infos(&*keys)
}

/// List a provider's selectable models: the session-cached live list when one
/// exists, else a fresh fetch, else the curated fallback (see
/// `models::resolve_model_list`). Fallbacks are never cached, so a transient
/// failure retries on the next menu open.
#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
    keys: State<'_, DpapiKeyStore>,
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

    let api_key = keys.get(provider.id);
    let live = if provider.models_need_key && api_key.is_none() {
        // The endpoint would 401 — skip the doomed round trip and degrade.
        Err(AppError::MissingApiKey {
            provider: provider.name,
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

/// Show the overlay if it's hidden (a visible panel is left untouched — no
/// `overlay://shown` re-fire, no select-all on a draft). Used by the frontend
/// when the capture hotkey fires against a text-only model: the panel must
/// appear to carry the explanation, or the hotkey fails silently.
#[tauri::command]
pub fn show_overlay(app: AppHandle) {
    window::show_overlay_if_hidden(&app);
}

/// Whether the visual debug window exists this session (`WIKILENS_DEBUG` was
/// truthy at startup) — the overlay renders its footer Debug chip only then.
/// Reveals a single bool about the local environment, never key material.
#[tauri::command]
pub fn debug_available() -> bool {
    crate::debug::debug_enabled()
}

/// Show/hide the debug window (the overlay footer's Debug chip). A no-op when
/// the window doesn't exist (flag off — the chip isn't rendered then anyway).
#[tauri::command]
pub fn toggle_debug_window(app: AppHandle) {
    crate::debug_window::toggle(&app);
}

/// Current settings, for the popover and the overlay's dynamic copy (prompt
/// placeholder, capture chip title).
#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsStore>) -> SettingsInfo {
    settings_info(&settings, sense_default_mode())
}

/// Change one shortcut: parse, refuse the other role's combo, prove the OS
/// will grant it, persist-then-commit, refresh the tray tooltip. Async
/// because the plugin's register/unregister hop to the main thread.
#[tauri::command]
pub async fn set_hotkey(
    app: AppHandle,
    settings: State<'_, SettingsStore>,
    role: HotkeyRole,
    accelerator: String,
) -> Result<SettingsInfo, String> {
    let new = hotkey::parse_accelerator(&accelerator).map_err(String::from)?;
    let current = settings.shortcut(role);
    if new == current {
        return Ok(settings_info(&settings, sense_default_mode()));
    }
    let other = match role {
        HotkeyRole::Summon => HotkeyRole::Capture,
        HotkeyRole::Capture => HotkeyRole::Summon,
    };
    if new == settings.shortcut(other) {
        let name = match other {
            HotkeyRole::Summon => "Summon",
            HotkeyRole::Capture => "Capture",
        };
        return Err(format!(
            "That's already your {name} shortcut — pick a different combo."
        ));
    }
    // Writability first: never touch a working registration for a change
    // that can't be persisted anyway.
    settings.writable().map_err(String::from)?;
    if settings.is_suspended() {
        // Recorder armed (the normal path): registrations are down, so just
        // prove the OS will grant the combo — `resume_hotkeys` registers it.
        hotkey::probe(&app, new).map_err(String::from)?;
        settings.set_hotkey(role, new).map_err(String::from)?;
    } else {
        // Reset / non-recorder path: swap live; roll back if persist fails.
        hotkey::apply_change(&app, current, new).map_err(String::from)?;
        if let Err(e) = settings.set_hotkey(role, new) {
            let _ = hotkey::apply_change(&app, new, current);
            return Err(String::from(e));
        }
    }
    crate::tray::update_summon_tooltip(&app);
    Ok(settings_info(&settings, sense_default_mode()))
}

/// Drop the OS hotkey registrations while the settings recorder is armed —
/// otherwise pressing the current combo mid-recording would toggle the
/// overlay. Idempotent; failures are logged Rust-side only (the frontend's
/// `overlay://hidden` disarm path contains a leftover registration).
#[tauri::command]
pub async fn suspend_hotkeys(
    app: AppHandle,
    settings: State<'_, SettingsStore>,
) -> Result<(), String> {
    if !settings.suspend() {
        hotkey::unregister_all(&app);
    }
    Ok(())
}

/// Restore the configured registrations after recording. Idempotent; a combo
/// another app grabbed meanwhile surfaces as a user-readable error.
#[tauri::command]
pub async fn resume_hotkeys(
    app: AppHandle,
    settings: State<'_, SettingsStore>,
) -> Result<(), String> {
    if settings.resume() {
        hotkey::register_all(&app).map_err(String::from)?;
    }
    Ok(())
}

/// Key presence per provider, for the settings panel's "Answers come from"
/// list (one row per provider; presence decides keyed vs. needs-a-key).
#[tauri::command]
pub fn list_key_status(keys: State<'_, DpapiKeyStore>) -> Vec<KeyStatus> {
    key_status(&*keys)
}

/// Store (or replace) a provider's API key — **the single place key material
/// crosses IPC, and only webview → Rust**. No response, event, or error ever
/// carries it back (pinned in `config_guardrails.rs`). Resolves with fresh
/// statuses so the panel updates in one round trip.
#[tauri::command]
pub fn set_api_key(
    keys: State<'_, DpapiKeyStore>,
    provider_id: String,
    key: String,
) -> Result<Vec<KeyStatus>, String> {
    set_key(&*keys, &provider_id, &key)
}

/// Drop a provider's stored key. The only action offered on a set key — keys
/// are never displayed back in any form.
#[tauri::command]
pub fn remove_api_key(
    keys: State<'_, DpapiKeyStore>,
    provider_id: String,
) -> Result<Vec<KeyStatus>, String> {
    remove_key(&*keys, &provider_id)
}

/// Persist the model-source choice — `run_ask` snapshots it per ask and the
/// footer follows it (vault/2026-07-26_default-mode-and-byo-api-keys.md).
/// Writable gate + persist-then-commit live in `SettingsStore::set_mode`
/// (the `set_hotkey` shape).
#[tauri::command]
pub fn set_mode(settings: State<'_, SettingsStore>, mode: Mode) -> Result<SettingsInfo, String> {
    settings.set_mode(mode).map_err(String::from)?;
    Ok(settings_info(&settings, sense_default_mode()))
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

/// The `ask` command's settle value after `cancel_ask` wins the race — the one
/// deliberately machine-readable command error (every other message is
/// user-readable prose). The frontend compares against its mirror constant in
/// `src/api.ts` and quietly resets instead of rendering an error box.
pub const ASK_CANCELLED: &str = "wikilens::ask-cancelled";

/// Answer a question about a game using its wiki as the source of truth.
///
/// Emits progress events the UI listens for:
/// - `ask://status` — `"searching"` → optional `"understanding"` (only while the
///   rewrite's candidate searches run) → optional `"retrying"` → `"reading"` →
///   `"answering"`
/// - `ask://delta` — streamed answer text chunks
///
/// Only one `ask` runs at a time; a concurrent call is rejected. `cancel_ask`
/// aborts the running one: the ask future is dropped at whatever await point it
/// had reached and this command settles with [`ASK_CANCELLED`].
///
/// `image_id` optionally names an attached screenshot (from `finish_capture`);
/// a mismatch with the stored attachment fails fast as "capture it again". The
/// attachment is cleared only after the model actually answers.
// The arg list is the IPC contract: five managed handles + one arg per
// frontend payload field. Bundling them into a struct would only move the count.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn ask(
    app: AppHandle,
    state: State<'_, AppState>,
    store: State<'_, UserWikiStore>,
    keys: State<'_, DpapiKeyStore>,
    settings: State<'_, SettingsStore>,
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
    // Releases the slot (and disarms the cancel token) on every exit path,
    // including cancellation (drop).
    let _guard = AskGuard(&state);

    // Arm this ask's cancel token. Per-ask and disarmed by the guard's Drop,
    // so a stale `cancel_ask` that lands after this ask settles finds an empty
    // slot instead of the next ask's token.
    let cancel = CancellationToken::new();
    *state
        .cancel_ask
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cancel.clone());

    let run = run_ask(
        &app,
        &state,
        &store,
        &*keys,
        &settings,
        &game_id,
        &provider_id,
        &model,
        &question,
        image_id.as_deref(),
    );
    match race_cancel(run, cancel).await {
        Some(result) => result.map_err(String::from),
        None => Err(ASK_CANCELLED.to_string()),
    }
}

/// Race a future against its cancel token: `Some(output)` when the future
/// finishes, `None` when the token fires first. Cancellation works by dropping
/// the future at whatever await point it had reached — reqwest aborts the
/// in-flight request, and the ask's `DebugReport` still emits its
/// finished/aborted row from `Drop`. The run future is polled first, so a
/// completed answer beats a simultaneous cancel.
async fn race_cancel<F: std::future::Future>(
    run: F,
    cancel: CancellationToken,
) -> Option<F::Output> {
    use futures_util::future::{select, Either};
    let cancelled = cancel.cancelled();
    futures_util::pin_mut!(run, cancelled);
    match select(run, cancelled).await {
        Either::Left((output, _)) => Some(output),
        Either::Right(((), _)) => None,
    }
}

/// Cancel the in-flight `ask`, if any (the status row's Stop action). Fires the
/// stored token; `ask` then settles with [`ASK_CANCELLED`]. Always `Ok`: with
/// nothing in flight the slot is empty and this is a no-op, and
/// `CancellationToken::cancel` is idempotent, so a double Stop is safe too.
#[tauri::command]
pub fn cancel_ask(state: State<'_, AppState>) {
    if let Some(token) = state
        .cancel_ask
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        token.cancel();
    }
}

/// Resets `ask_in_progress` and disarms the cancel token when dropped, so a
/// panic or a cancelled future can't leave the guard stuck — and an idle-time
/// `cancel_ask` finds nothing to fire.
struct AskGuard<'a>(&'a AppState);

impl Drop for AskGuard<'_> {
    fn drop(&mut self) {
        self.0
            .cancel_ask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.0.ask_in_progress.store(false, Ordering::SeqCst);
    }
}

/// Custom-mode target resolution: registry provider + stored key + the picked
/// model, which drives both the answer and the pre-search rewrite.
fn resolve_custom_targets(
    keys: &dyn KeyStore,
    provider_id: &str,
    model: &str,
) -> Result<AskTargets, AppError> {
    // The frontend always sends a provider, but fall back to the default if it
    // ever sends a blank one (commands are a trust boundary).
    let provider_id = if provider_id.trim().is_empty() {
        providers::DEFAULT_PROVIDER_ID
    } else {
        provider_id
    };
    let provider = providers::find_provider(provider_id)
        .ok_or_else(|| AppError::UnknownProvider(provider_id.to_string()))?;
    // Store-only key resolution (no env fallback). Known quirk, accepted by
    // the storage ADR: a ghost blob from another machine reads as "Key added"
    // in the panel (`has_key` never decrypts) but as missing here — re-pasting
    // the key recovers.
    let api_key = keys.get(provider.id).ok_or(AppError::MissingApiKey {
        provider: provider.name,
    })?;
    let model = effective_model(model, provider.model());
    // The picked model drives both the answer and the pre-search rewrite —
    // one model choice, one destination for player text. (Default mode has
    // its own optional WIKILENS_DEFAULT_REWRITE_MODEL.)
    // A known-Reasoning model makes the rewrite a call we *know* fails: its
    // reply lands in `reasoning_content`, the parser reads `content` → zero
    // candidates after a multi-second think. Skip it outright (zero latency,
    // zero cost); unknown models stay eligible — the circuit breaker bounds
    // their worst case (`vault/2026-07-10_reasoning-skip-and-capability-tags.md`).
    let rewrite_skip_reasoning = provider
        .model_reasoning(&model)
        .or_else(|| models::reasoning_from_id(&model))
        == Some(true);
    // Construct-twice (LlmTarget isn't Clone): identical debug_id + model
    // strings keep the debug table's rewrite-row suppression working.
    Ok(AskTargets {
        answer: LlmTarget::from_provider(provider, api_key.clone(), model.clone()),
        rewrite: LlmTarget::from_provider(provider, api_key, model),
        rewrite_skip_reasoning,
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_ask(
    app: &AppHandle,
    state: &AppState,
    store: &UserWikiStore,
    keys: &dyn KeyStore,
    settings: &SettingsStore,
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

    // Resolve the ask's targets up front, before any status event, so a
    // missing key / unconfigured Default fails instantly (no stuck
    // "Searching…"). The AskGuard still releases the slot on this early
    // return. The mode is snapshotted ONCE here — a mid-ask switch can't tear
    // the pair (the gear is disabled while busy anyway; this is the backstop).
    let mode = settings.mode().unwrap_or(Mode::Custom);
    let targets = match mode {
        // Default mode: one env-configured target; the request's
        // provider_id/model args are deliberately ignored.
        Mode::Default => target::sense_default_targets().map_err(AppError::DefaultMode)?,
        Mode::Custom => resolve_custom_targets(keys, provider_id, model)?,
    };
    let rewrite_skip_reasoning = targets.rewrite_skip_reasoning;

    let _ = app.emit("ask://status", "searching");
    // The wiki search gets a keyword-stripped query; the LLM still receives the
    // original question below.
    let query = search::preprocess_query(question);
    // WIKILENS_DEBUG collector — filled below, prints itself on every exit path
    // (including `?` errors) via Drop. Created only now because the header rows
    // need the resolved models; the pre-flight failures above print no table.
    // Ids and models come from the targets on BOTH sides: the pair-equality
    // check in debug.rs suppresses the rewrite row only when the strings
    // match, and identical targets carry identical debug_ids by construction.
    let mut report = DebugReport::new(
        &wiki.name,
        game_id,
        question,
        &query,
        targets.answer.debug_id,
        &targets.answer.model,
        targets.rewrite.debug_id,
        &targets.rewrite.model,
    );
    // Mirror the report into the debug window when it exists (flag on at
    // startup); every setter below then also emits its debug:// event.
    if let Some(sink) = crate::debug_window::sink(app) {
        report.attach_sink(sink);
    }
    let search_start = std::time::Instant::now();
    let tracing = std::env::var_os("WIKILENS_TRACE_RETRIEVAL").is_some();
    let rewrite_on = stage_enabled("WIKILENS_QUERY_REWRITE");

    // Eager query understanding: run the raw keyword search and an LLM rewrite of the
    // question *concurrently*. The rewrite hits the model provider — a different host —
    // so it overlaps the wiki search instead of adding latency. Eager (every query)
    // because the search often returns wrong-but-nonzero pages a zero-hit-only rewrite
    // would never reach. The rewrite yields no candidates on any failure (graceful).
    // Each future times itself and returns data (never touches `report` — two
    // concurrent futures can't share a `&mut`); the phase rows are recorded
    // after the join. The rewrite future's extra fields: usage is `Some` iff
    // the request was actually sent, elapsed is `None` when the stage was
    // disabled, and the last field carries an error message when the call failed.
    let raw_fut = async {
        let timer = std::time::Instant::now();
        let result =
            search::search_full(&state.http, &wiki, &query, search::DEFAULT_SEARCH_LIMIT).await;
        (result, timer.elapsed())
    };
    let rewrite_fut = async {
        if !rewrite_on {
            return (Vec::new(), None, None, None);
        }
        // Known-Reasoning model: the call is doomed (see above). Same all-None
        // tuple as the off-switch — never made, never a breaker strike.
        if rewrite_skip_reasoning {
            return (Vec::new(), None, None, None);
        }
        // Session circuit breaker: a rewrite model that never yields candidates
        // (see AppState::record_rewrite_outcome) stops costing every ask its
        // full timeout budget. Same all-None tuple as the off-switch — the
        // call was never made. Skips never touch the counter.
        if state.rewrite_breaker_tripped() {
            return (Vec::new(), None, None, None);
        }
        let timer = std::time::Instant::now();
        match llm::rewrite_query(&state.http, &targets.rewrite, &wiki.name, question).await {
            Ok(outcome) => {
                if tracing {
                    eprintln!("wikilens.rewrite candidates={:?}", outcome.queries);
                }
                // An empty parse counts as a strike: the reasoning-only-model
                // failure mode is a successful call with an unusable body.
                state.record_rewrite_outcome(!outcome.queries.is_empty());
                (
                    outcome.queries,
                    Some(outcome.usage),
                    Some(timer.elapsed()),
                    None,
                )
            }
            Err(e) => {
                if tracing {
                    eprintln!("wikilens.rewrite error={e}");
                }
                state.record_rewrite_outcome(false);
                (
                    Vec::new(),
                    Some(llm::TokenUsage::default()),
                    Some(timer.elapsed()),
                    Some(e.to_string()),
                )
            }
        }
    };
    let ((raw_result, raw_elapsed), (rewrite_candidates, rewrite_usage, rewrite_elapsed, rewrite_error)) =
        futures_util::future::join(raw_fut, rewrite_fut).await;

    match rewrite_elapsed {
        Some(elapsed) => {
            let detail = match &rewrite_error {
                Some(e) => format!("error: {}", truncate_detail(e)),
                None => format!("{} candidates", rewrite_candidates.len()),
            };
            report.phase("rewrite", elapsed, detail);
        }
        // `rewrite_elapsed` is `None` only when an early return fired; the
        // guards mirror the closure's order (off-switch → reasoning-skip →
        // breaker), so what's left in the last arm is the breaker. Derived
        // from the local flags rather than re-reading the counter — race-free
        // even though this ask's own failure may have just tripped it.
        None if !rewrite_on => report.phase_skipped("rewrite", "disabled (WIKILENS_QUERY_REWRITE)"),
        None if rewrite_skip_reasoning => {
            report.phase_skipped("rewrite", "skipped (reasoning model)")
        }
        None => report.phase_skipped("rewrite", "skipped (circuit breaker)"),
    }
    if let Some(usage) = rewrite_usage {
        report.set_rewrite_usage(usage);
    }
    report.set_candidates(&rewrite_candidates);
    // Recorded before the `?` so a raw-search failure still shows its row.
    let raw_detail = match &raw_result {
        Ok((titles, _)) => format!("{} hits", titles.len()),
        Err(e) => format!("error: {}", truncate_detail(&e.to_string())),
    };
    report.phase("raw search", raw_elapsed, raw_detail);
    let (raw_titles, suggestion) = raw_result?;

    // Search the top rewrite candidates (concurrent, bounded) and merge with the raw
    // hits: consensus (in both) first, then entity hits, then keyword hits.
    // Candidates that merely echo the raw query would return the exact same hits —
    // drop them *before* the take() so a surviving second candidate still gets
    // searched. Dropped duplicates already counted as rewrite success for the
    // circuit breaker (the model parsed fine), and the debug table still shows
    // the full candidate list.
    let to_search: Vec<&String> = rewrite_candidates
        .iter()
        .filter(|rq| {
            let dup = rq.eq_ignore_ascii_case(&query);
            if dup && tracing {
                eprintln!("wikilens.rewrite dropped duplicate-of-raw candidate={rq:?}");
            }
            !dup
        })
        .take(REWRITE_SEARCH_LIMIT)
        .collect();
    let mut retries: Vec<(&str, String)> = Vec::new();
    let mut rewrite_hits: Vec<String> = Vec::new();
    if !to_search.is_empty() {
        // Emitted only when candidate searches actually run — the skip/empty
        // paths (rewrite off, reasoning skip, breaker, all-duplicate) must
        // never flash this status.
        let _ = app.emit("ask://status", "understanding");
        let cand_timer = std::time::Instant::now();
        // join_all preserves input order, so zipping back keeps `rewrite_hits`
        // in candidate order — the merge below ranks by list order.
        let results = futures_util::future::join_all(
            to_search
                .iter()
                .map(|rq| search::search(&state.http, &wiki, rq, search::DEFAULT_SEARCH_LIMIT)),
        )
        .await;
        for (rq, hits) in to_search.iter().zip(results) {
            let hits = hits.unwrap_or_default();
            if !hits.is_empty() {
                retries.push(("rewrite", (*rq).clone()));
            }
            rewrite_hits.extend(hits);
        }
        report.phase(
            "cand search",
            cand_timer.elapsed(),
            format!("{} queries -> {} hits", to_search.len(), rewrite_hits.len()),
        );
    }
    let mut titles = merge_hits(&raw_titles, &rewrite_hits, search::DEFAULT_SEARCH_LIMIT as usize);

    // Deterministic net — only when raw + rewrite both came up empty. Cheapest first,
    // each firing only while still empty (MediaWiki etiquette): the wiki's own "did
    // you mean" suggestion, a harsher keyword pass, then the local title index.
    if titles.is_empty() {
        let sugg = suggestion.as_deref().unwrap_or("");
        if !sugg.is_empty() && sugg != query.as_str() {
            let _ = app.emit("ask://status", "retrying");
            let timer = std::time::Instant::now();
            let result = search::search(&state.http, &wiki, sugg, search::DEFAULT_SEARCH_LIMIT).await;
            report.phase("retry:suggestion", timer.elapsed(), retry_detail(sugg, &result));
            titles = result?;
            retries.push(("suggestion", sugg.to_string()));
        }
    }
    if titles.is_empty() {
        let simplified = search::simplify_query(question);
        if !simplified.is_empty() && simplified != query {
            let _ = app.emit("ask://status", "retrying");
            let timer = std::time::Instant::now();
            let result =
                search::search(&state.http, &wiki, &simplified, search::DEFAULT_SEARCH_LIMIT)
                    .await;
            report.phase("retry:simplify", timer.elapsed(), retry_detail(&simplified, &result));
            titles = result?;
            retries.push(("simplify", simplified));
        }
    }
    // Deterministic typo→title fallback: fuzzy-match the query against the game's real
    // page titles (fetched once per session), no model call, can't hallucinate. Off
    // with `WIKILENS_TITLE_INDEX=0`. Skipped for long queries — `best_match` compares
    // the whole query to whole titles, so a many-word question can never match a short
    // title (the length guard rejects it); fetching titles for it would just stall.
    if titles.is_empty()
        && stage_enabled("WIKILENS_TITLE_INDEX")
        && query.split_whitespace().count() <= TITLE_INDEX_MAX_WORDS
    {
        // Emit before the (possibly multi-second, first-time-per-session) fetch so
        // the UI isn't stalled on a stale status.
        let _ = app.emit("ask://status", "retrying");
        let timer = std::time::Instant::now();
        match titles::get_or_fetch(&state.title_cache, &state.http, &wiki).await {
            Some(index) => match index.best_match(&query) {
                Some(matched) => {
                    report.phase(
                        "retry:title-index",
                        timer.elapsed(),
                        format!("\"{matched}\" -> 1 hit"),
                    );
                    retries.push(("title-index", matched.clone()));
                    titles = vec![matched];
                }
                // Record the fired-but-missed round too (trace fidelity).
                None => {
                    report.phase("retry:title-index", timer.elapsed(), "(no match)".to_string());
                    retries.push(("title-index", "(no match)".to_string()));
                }
            },
            None => {
                report.phase("retry:title-index", timer.elapsed(), "index unavailable".to_string());
            }
        }
    }

    // Opt-in retrieval trace for offline eval scoring (Phase 2 of the
    // retrieval-quality plan). Off unless `WIKILENS_TRACE_RETRIEVAL` is set; records
    // each rewrite candidate / recovery stage that contributed (the eager rewrite's
    // full candidate list prints on the separate `wikilens.rewrite` line).
    trace_retrieval(&wiki, question, &query, &retries, &titles, search_start.elapsed());

    if titles.is_empty() {
        report.finish("no results");
        return Ok(AskResult {
            answer: format!(
                "I couldn't find anything on the {} wiki for that. Try rephrasing with different keywords.",
                wiki.name
            ),
            sources: Vec::new(),
        });
    }

    let _ = app.emit("ask://status", "reading");
    let fetch_timer = std::time::Instant::now();
    let pages_result = fetch::fetch_pages(&state.http, &wiki, &titles).await;
    // Recorded before the `?` so a fetch failure still shows its row. The table
    // gets titles and char counts only — never the page text.
    match &pages_result {
        Ok(pages) => {
            let sizes: Vec<(String, usize)> = pages
                .iter()
                .map(|p| (p.title.clone(), p.text.chars().count()))
                .collect();
            let total: usize = sizes.iter().map(|(_, chars)| chars).sum();
            report.phase(
                "fetch",
                fetch_timer.elapsed(),
                format!("{} pages, {total} chars", sizes.len()),
            );
            report.set_pages(sizes);
        }
        Err(e) => report.phase(
            "fetch",
            fetch_timer.elapsed(),
            format!("error: {}", truncate_detail(&e.to_string())),
        ),
    }
    let pages = pages_result?;
    if pages.is_empty() {
        report.finish("pages unreadable");
        return Ok(AskResult {
            answer: "I found matching pages but couldn't read their contents. Please try again."
                .to_string(),
            sources: Vec::new(),
        });
    }

    let _ = app.emit("ask://status", "answering");
    let delta_app = app.clone();
    let answer_timer = std::time::Instant::now();
    let streamed = llm::answer_streaming(
        &state.http,
        &targets.answer,
        question,
        &pages,
        image_png.as_deref(),
        move |delta| {
            let _ = delta_app.emit("ask://delta", delta);
        },
    )
    .await;
    let answer_detail = match &streamed {
        Ok(s) => match s.ttft {
            Some(ttft) => format!("first token {} ms", ttft.as_millis()),
            None => "no first token".to_string(),
        },
        Err(e) => format!("error: {}", truncate_detail(&e.to_string())),
    };
    report.phase("answer", answer_timer.elapsed(), answer_detail);
    let streamed = streamed?;
    report.set_answer_usage(streamed.usage);
    report.finish("answered");
    let answer = streamed.text;

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

/// How many of the rewrite's candidate queries to actually search per ask. Bounds
/// the extra wiki round-trips the eager rewrite adds (searched concurrently, so
/// the phase costs the slowest query, not the sum).
const REWRITE_SEARCH_LIMIT: usize = 2;

/// Max preprocessed-query word count for the title-index fallback to bother
/// fetching. `best_match` scores whole-query-vs-whole-title, so a longer query
/// can't match a short entity title anyway — skip the (multi-second) allpages walk.
const TITLE_INDEX_MAX_WORDS: usize = 4;

/// Merge the raw-search and rewrite-search hit lists into the final ≤`limit` titles,
/// ranked: consensus (a title in *both* lists — the strongest signal) first, then
/// rewrite-only hits (the targeted entity), then raw-only hits (keyword match).
/// Case-insensitive dedup, order-preserving; consensus keeps the raw/canonical casing.
///
/// Guarantee: raw's top hit is always *included* (a slot is reserved for it, so a
/// confident-but-wrong rewrite can't flood the cap and silently evict the correct
/// page). Inclusion, not rank — every merged page is fetched and handed to the
/// model, and pinning raw[0] to #1 would undo the entity-injection win.
fn merge_hits(raw: &[String], rewrite: &[String], limit: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // 1) consensus — titles in both lists, using the raw (canonical) casing.
    for t in rewrite {
        if let Some(canonical) = raw.iter().find(|r| r.eq_ignore_ascii_case(t)) {
            push_unique(&mut out, canonical, limit);
        }
    }
    // 2) remaining rewrite hits — capped one short of `limit` while raw[0] still
    //    needs its reserved slot (consensus already admitted it iff any rewrite
    //    hit case-matches it).
    let reserve = raw
        .first()
        .is_some_and(|r0| !out.iter().any(|e| e.eq_ignore_ascii_case(r0)));
    let rewrite_cap = if reserve { limit.saturating_sub(1) } else { limit };
    for t in rewrite {
        push_unique(&mut out, t, rewrite_cap);
    }
    // 3) remaining raw hits — raw[0] first, filling its reserved slot.
    for t in raw {
        push_unique(&mut out, t, limit);
    }
    out
}

/// Append `title` to `out` if there is room (`limit`) and no case-insensitive dup.
fn push_unique(out: &mut Vec<String>, title: &str, limit: usize) {
    if out.len() < limit && !out.iter().any(|e| e.eq_ignore_ascii_case(title)) {
        out.push(title.to_string());
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn ranks_consensus_then_rewrite_then_raw() {
        let raw = vec![
            "Trinity".to_string(),
            "Sirius & Orion".to_string(),
            "Mag".to_string(),
        ];
        let rewrite = vec!["Sirius & Orion".to_string(), "Wisp".to_string()];
        // Consensus (Sirius & Orion) first, then rewrite-only (Wisp), then raw-only.
        assert_eq!(
            merge_hits(&raw, &rewrite, 4),
            vec![
                "Sirius & Orion".to_string(),
                "Wisp".to_string(),
                "Trinity".to_string(),
                "Mag".to_string(),
            ]
        );
    }

    #[test]
    fn injects_the_entity_when_raw_is_junk() {
        let raw = vec!["Version History".to_string()];
        let rewrite = vec!["Wine".to_string()];
        assert_eq!(
            merge_hits(&raw, &rewrite, 4),
            vec!["Wine".to_string(), "Version History".to_string()]
        );
    }

    #[test]
    fn dedupes_case_insensitively_keeps_canonical_and_truncates() {
        let raw = vec!["Wood".to_string(), "Stone".to_string()];
        let rewrite = vec!["wood".to_string(), "Clay".to_string()];
        // "wood"/"Wood" collapse to the raw casing (consensus); limit caps the rest.
        assert_eq!(
            merge_hits(&raw, &rewrite, 2),
            vec!["Wood".to_string(), "Clay".to_string()]
        );
    }

    #[test]
    fn handles_empty_inputs() {
        let raw = vec!["A".to_string()];
        assert_eq!(merge_hits(&raw, &[], 4), vec!["A".to_string()]);
        assert_eq!(merge_hits(&[], &raw, 4), vec!["A".to_string()]);
        assert!(merge_hits(&[], &[], 4).is_empty());
    }

    #[test]
    fn raw_first_survives_a_rewrite_flood() {
        // A confident-but-wrong rewrite floods the cap; raw's top hit must
        // still be included (last is fine — inclusion matters, not rank).
        let raw = vec!["R1".to_string(), "R2".to_string()];
        let rewrite = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ];
        assert_eq!(
            merge_hits(&raw, &rewrite, 4),
            vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "R1".to_string(),
            ]
        );
    }

    #[test]
    fn consensus_on_raw_first_frees_the_reserved_slot() {
        // raw[0] already entered via consensus — rewrite-only hits may fill
        // every remaining slot, no slot held back.
        let raw = vec!["R1".to_string(), "R2".to_string()];
        let rewrite = vec![
            "R1".to_string(),
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
        ];
        assert_eq!(
            merge_hits(&raw, &rewrite, 4),
            vec![
                "R1".to_string(),
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
            ]
        );
    }

    #[test]
    fn limit_one_still_keeps_raw_first() {
        // The saturating_sub edge: limit 1 leaves zero rewrite-only slots.
        let raw = vec!["R1".to_string()];
        let rewrite = vec!["A".to_string()];
        assert_eq!(merge_hits(&raw, &rewrite, 1), vec!["R1".to_string()]);
    }
}

/// Detail cell for a retry-ladder row in the debug table: the retried query and
/// its hit count, or the (capped) error when the retry search itself failed.
fn retry_detail(retry_query: &str, result: &Result<Vec<String>, AppError>) -> String {
    match result {
        Ok(hits) => format!("\"{retry_query}\" -> {} hits", hits.len()),
        Err(e) => format!("error: {}", truncate_detail(&e.to_string())),
    }
}

/// Cap an error string for a debug-table detail cell — LLM/wiki error bodies
/// can be whole JSON documents.
fn truncate_detail(s: &str) -> String {
    const MAX_DETAIL_CHARS: usize = 120;
    if s.chars().count() <= MAX_DETAIL_CHARS {
        s.to_string()
    } else {
        let capped: String = s.chars().take(MAX_DETAIL_CHARS).collect();
        format!("{capped}…")
    }
}

/// An optional retrieval stage (the zero-hit title index, the eager LLM rewrite)
/// is on unless its env var is explicitly falsey (`0`/`false`/`off`) — an
/// off-switch for the two unvalidated stages that needs no rebuild.
fn stage_enabled(var: &str) -> bool {
    match std::env::var(var) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => true,
    }
}

/// Opt-in, one-line JSON trace of a retrieval round for offline eval scoring
/// (Phase 2 of the retrieval-quality plan). Silent unless `WIKILENS_TRACE_RETRIEVAL`
/// is set. Emits only public wiki data — game id, the question, the search query,
/// which retry (if any) fired, the resulting titles, and search latency — to
/// stderr, so `npm run tauri dev 2> eval.jsonl` collects a scorable dataset. Never
/// touches API keys or answer text.
fn trace_retrieval(
    wiki: &GameWiki,
    question: &str,
    query: &str,
    retries: &[(&str, String)],
    titles: &[String],
    elapsed: std::time::Duration,
) {
    if std::env::var_os("WIKILENS_TRACE_RETRIEVAL").is_none() {
        return;
    }
    let record = serde_json::json!({
        "game": wiki.id,
        "question": question,
        "query": query,
        // Every retry stage that fired, in order (`search_ms` covers them all).
        "retries": retries
            .iter()
            .map(|(kind, q)| serde_json::json!({ "kind": kind, "query": q }))
            .collect::<Vec<_>>(),
        "titles": titles,
        "hits": titles.len(),
        "search_ms": elapsed.as_millis(),
    });
    eprintln!("wikilens.retrieval {record}");
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{Duration, Instant};

    use crate::keys::InMemoryKeyStore;

    #[test]
    fn provider_infos_lists_only_keyed_providers_in_registry_order() {
        let keys = InMemoryKeyStore::default();
        assert!(provider_infos(&keys).is_empty(), "no keys → empty menu");

        keys.set("deepseek", "sk-1").unwrap();
        keys.set("anthropic", "sk-2").unwrap();
        let ids: Vec<String> = provider_infos(&keys).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["anthropic", "deepseek"], "registry order, keyed only");
    }

    #[test]
    fn resolve_custom_targets_pairs_answer_and_rewrite_on_the_picked_model() {
        let keys = InMemoryKeyStore::default();
        keys.set("anthropic", "sk-1").unwrap();
        let targets = resolve_custom_targets(&keys, "anthropic", "claude-haiku-4-5-20251001")
            .expect("keyed provider resolves");
        // One model choice drives both calls — identical strings keep the
        // debug table's rewrite-row suppression working.
        assert_eq!(targets.answer.model, targets.rewrite.model);
        assert_eq!(targets.answer.debug_id, targets.rewrite.debug_id);
        assert_eq!(targets.answer.api_key, targets.rewrite.api_key);
        assert!(!targets.rewrite_skip_reasoning, "curated non-reasoning model");
    }

    #[test]
    fn resolve_custom_targets_skips_rewrite_for_a_reasoning_model() {
        let keys = InMemoryKeyStore::default();
        keys.set("deepseek", "sk-1").unwrap();
        let targets = resolve_custom_targets(&keys, "deepseek", "deepseek-v4-flash")
            .expect("keyed provider resolves");
        assert!(targets.rewrite_skip_reasoning, "curated reasoning model");
    }

    #[test]
    fn resolve_custom_targets_requires_a_stored_key() {
        let keys = InMemoryKeyStore::default();
        assert!(matches!(
            resolve_custom_targets(&keys, "anthropic", ""),
            Err(AppError::MissingApiKey { .. })
        ));
    }

    #[test]
    fn resolve_custom_targets_falls_back_on_a_blank_provider() {
        let keys = InMemoryKeyStore::default();
        keys.set("anthropic", "sk-1").unwrap();
        let targets = resolve_custom_targets(&keys, "  ", "").expect("default provider");
        assert_eq!(targets.answer.debug_id, "anthropic");
        assert!(!targets.answer.model.is_empty(), "provider default model");
    }

    #[test]
    fn key_status_lists_every_provider_in_registry_order() {
        let keys = InMemoryKeyStore::default();
        let rows = key_status(&keys);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["anthropic", "deepseek", "openrouter"]);
        assert!(rows.iter().all(|r| !r.has_key));

        keys.set("deepseek", "sk-ds").unwrap();
        let rows = key_status(&keys);
        assert!(rows.iter().all(|r| r.has_key == (r.id == "deepseek")));
    }

    #[test]
    fn set_key_trims_stores_and_reports_fresh_statuses() {
        let keys = InMemoryKeyStore::default();
        let rows = set_key(&keys, "anthropic", "  sk-ant-1\n").unwrap();
        assert_eq!(keys.get("anthropic").as_deref(), Some("sk-ant-1"));
        assert!(rows.iter().any(|r| r.id == "anthropic" && r.has_key));
    }

    #[test]
    fn set_key_rejects_unknown_provider() {
        let keys = InMemoryKeyStore::default();
        let err = set_key(&keys, "netscape", "sk-1").unwrap_err();
        assert!(err.contains("Unknown provider"), "was: {err}");
    }

    #[test]
    fn set_key_rejects_a_blank_key_with_friendly_copy() {
        let keys = InMemoryKeyStore::default();
        assert_eq!(
            set_key(&keys, "anthropic", "   ").unwrap_err(),
            "Paste an API key first."
        );
        assert!(!keys.has_key("anthropic"));
    }

    #[test]
    fn remove_key_clears_and_reports_fresh_statuses() {
        let keys = InMemoryKeyStore::default();
        keys.set("anthropic", "sk-1").unwrap();
        let rows = remove_key(&keys, "anthropic").unwrap();
        assert!(rows.iter().all(|r| !r.has_key));
        // Removing an absent key is the trait's no-op leg — still Ok.
        assert!(remove_key(&keys, "anthropic").is_ok());
    }

    #[test]
    fn remove_key_rejects_unknown_provider() {
        let keys = InMemoryKeyStore::default();
        let err = remove_key(&keys, "netscape").unwrap_err();
        assert!(err.contains("Unknown provider"), "was: {err}");
    }

    #[test]
    fn default_mode_requires_all_four_vars_and_vision_is_opt_in() {
        let full = |name: &str| match name {
            "WIKILENS_DEFAULT_API_KEY" => Some("k".to_string()),
            "WIKILENS_DEFAULT_API_PROVIDER" => Some("anthropic".to_string()),
            "WIKILENS_DEFAULT_API_URL" => Some("https://proxy.example/v1".to_string()),
            "WIKILENS_DEFAULT_ANSWER_MODEL" => Some("some-model".to_string()),
            _ => None,
        };
        let info = default_mode_info(full);
        assert!(info.configured);
        assert!(!info.vision, "vision defaults off");

        // Configured means resolver success, not mere presence: an invalid
        // protocol word (e.g. a vendor id) must read as unconfigured, so the
        // panel's note and the ask-path error can't disagree.
        let info = default_mode_info(|name| {
            if name == "WIKILENS_DEFAULT_API_PROVIDER" {
                Some("deepseek".to_string())
            } else {
                full(name)
            }
        });
        assert!(!info.configured, "vendor id is not a protocol word");

        // Any one required var missing → unconfigured.
        for missing in [
            "WIKILENS_DEFAULT_API_KEY",
            "WIKILENS_DEFAULT_API_PROVIDER",
            "WIKILENS_DEFAULT_API_URL",
            "WIKILENS_DEFAULT_ANSWER_MODEL",
        ] {
            let info = default_mode_info(|name| if name == missing { None } else { full(name) });
            assert!(!info.configured, "should be unconfigured without {missing}");
        }

        // Vision is opt-in with the WIKILENS_DEBUG truthy semantics.
        for (value, expected) in [("1", true), ("true", true), ("0", false), ("off", false)] {
            let info = default_mode_info(|name| {
                if name == "WIKILENS_DEFAULT_VISION" {
                    Some(value.to_string())
                } else {
                    full(name)
                }
            });
            assert_eq!(info.vision, expected, "WIKILENS_DEFAULT_VISION={value}");
        }
    }

    #[tokio::test]
    async fn race_cancel_pre_cancelled_token_beats_a_pending_future() {
        let token = CancellationToken::new();
        token.cancel();
        // Level-triggered: a cancel that landed before the race still fires.
        let out = race_cancel(std::future::pending::<u8>(), token).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn race_cancel_lets_a_ready_future_beat_a_cancelled_token() {
        let token = CancellationToken::new();
        token.cancel();
        // The run future is polled first: a completed answer wins over a
        // simultaneous Stop.
        let out = race_cancel(std::future::ready(7u8), token).await;
        assert_eq!(out, Some(7));
    }

    #[tokio::test]
    async fn race_cancel_interrupts_mid_await() {
        let token = CancellationToken::new();
        let trigger = token.clone();
        let started = Instant::now();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            trigger.cancel();
        });
        let out = race_cancel(tokio::time::sleep(Duration::from_secs(30)), token).await;
        assert!(out.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "cancel must interrupt the await promptly, not wait it out"
        );
    }

    #[tokio::test]
    async fn race_cancel_aborts_a_hung_http_request() {
        // The motivating case: a wiki (or LLM) socket that answers slowly.
        // Cancellation must land mid-request by dropping the future — reqwest
        // aborts the connection — not wait out the response.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_delay(Duration::from_secs(30)),
            )
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let token = CancellationToken::new();
        let trigger = token.clone();
        let started = Instant::now();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            trigger.cancel();
        });
        let request = async { client.get(server.uri()).send().await };
        let out = race_cancel(request, token).await;
        assert!(out.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "cancel must abort the in-flight request promptly"
        );
    }

    #[test]
    fn ask_guard_drop_disarms_the_cancel_token_slot() {
        let state = AppState::new();
        state.ask_in_progress.store(true, Ordering::SeqCst);
        *state
            .cancel_ask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(CancellationToken::new());

        drop(AskGuard(&state));

        assert!(!state.ask_in_progress.load(Ordering::SeqCst));
        // The slot is empty, so a stale cancel_ask after settle is a no-op.
        assert!(state
            .cancel_ask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none());
    }

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
