//! Shared application state managed by Tauri (`app.manage(...)`).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::models::ModelInfo;

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
}

impl AppState {
    pub fn new() -> Self {
        // A build failure here is extremely unlikely (only if the platform TLS
        // backend is missing); fall back to a default client rather than panic.
        let http = reqwest::Client::builder()
            .user_agent(crate::wiki::USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http,
            ask_in_progress: AtomicBool::new(false),
            models_cache: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
