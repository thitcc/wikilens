//! Shared application state managed by Tauri (`app.manage(...)`).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::capture::{Attachment, PendingShot};
use crate::models::ModelInfo;
use crate::wiki::titles::TitleIndex;

/// Global app state. A single `reqwest::Client` is reused for every wiki and
/// LLM request (connection pooling + the shared wiki User-Agent), and a flag
/// enforces the "one ask at a time" concurrency guard.
pub struct AppState {
    pub http: reqwest::Client,
    /// `true` while an `ask` command is running. See `commands::ask`.
    pub ask_in_progress: AtomicBool,
    /// Session cache of live-fetched model lists, keyed by provider id.
    /// Live lists only — fallbacks are never cached, so a transient failure
    /// retries on the next menu open. A std `Mutex` is fine because it is
    /// never held across an await (`commands::list_models` locks, clones,
    /// drops, fetches, relocks).
    pub models_cache: Mutex<HashMap<&'static str, Vec<ModelInfo>>>,
    /// The monitor snapshot frozen while a region capture is in progress
    /// (`Some` between `begin_capture` and `finish_capture`/`cancel_capture`).
    /// Same never-across-await discipline as `models_cache`.
    pub pending_shot: Mutex<Option<PendingShot>>,
    /// The one screenshot currently attached to the prompt, if any. Consumed
    /// by a successful `ask`, replaced by a new capture, or dropped on clear.
    pub attachment: Mutex<Option<Attachment>>,
    /// Per-session title index per game, for the zero-hit fuzzy-match recovery
    /// (`wiki::titles`). Populated lazily on first zero-hit for a game and reused
    /// for the rest of the session. Same never-held-across-await discipline as
    /// `models_cache`.
    pub title_cache: Mutex<HashMap<String, Arc<TitleIndex>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // All client policy (User-Agent, redirect rules, timeouts) lives
            // in `crate::http` — every request in the app flows through it.
            http: crate::http::build_client(),
            ask_in_progress: AtomicBool::new(false),
            models_cache: Mutex::new(HashMap::new()),
            pending_shot: Mutex::new(None),
            attachment: Mutex::new(None),
            title_cache: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
