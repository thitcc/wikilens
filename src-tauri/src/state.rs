//! Shared application state managed by Tauri (`app.manage(...)`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::capture::{Attachment, PendingShot};
use crate::models::ModelInfo;
use crate::wiki::titles::TitleIndex;

/// Consecutive zero-candidate rewrite outcomes that trip the session circuit
/// breaker. Two, not one: a single failure can be a transient 5xx/timeout; two
/// in a row on a path that should basically always parse is a config problem.
pub const REWRITE_BREAKER_LIMIT: u32 = 2;

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
    /// Consecutive rewrite attempts that yielded zero candidates (an error or
    /// an empty parse — the reasoning-only-model failure mode is a successful
    /// HTTP call with an unusable body). Any success resets it; reaching
    /// [`REWRITE_BREAKER_LIMIT`] disables rewrites for the rest of the session
    /// (see `rewrite_breaker_tripped`). A restart is the reset lever.
    pub rewrite_failures: AtomicU32,
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
            rewrite_failures: AtomicU32::new(0),
        }
    }

    /// `true` once the session circuit breaker has tripped: rewrites are
    /// skipped for the rest of the session (`commands::run_ask` checks this
    /// before each attempt). Skips never touch the counter, so a tripped
    /// breaker stays tripped until restart.
    pub fn rewrite_breaker_tripped(&self) -> bool {
        self.rewrite_failures.load(Ordering::SeqCst) >= REWRITE_BREAKER_LIMIT
    }

    /// Record the outcome of a rewrite *attempt* (never call on a skip).
    /// A success fully resets the counter — transient blips don't trip the
    /// breaker. On the exact crossing to the limit, print one loud stderr
    /// notice with the likely causes and the fixes; deliberately not gated by
    /// `WIKILENS_DEBUG`/`WIKILENS_TRACE_RETRIEVAL` — the whole point is making
    /// an otherwise-invisible session-long failure visible once.
    pub fn record_rewrite_outcome(&self, got_candidates: bool) {
        if got_candidates {
            self.rewrite_failures.store(0, Ordering::SeqCst);
            return;
        }
        let prev = self.rewrite_failures.fetch_add(1, Ordering::SeqCst);
        if prev + 1 == REWRITE_BREAKER_LIMIT {
            eprintln!(
                "wikilens: query rewrite disabled for the rest of this session — \
                 {REWRITE_BREAKER_LIMIT} consecutive attempts produced no usable candidates. \
                 Likely causes: a reasoning-only rewrite model (its reply lands outside the \
                 parsed content), a misconfigured WIKILENS_REWRITE_MODEL, or an unreachable \
                 provider host. Fix: point WIKILENS_REWRITE_MODEL (and, if needed, \
                 WIKILENS_REWRITE_PROVIDER) at a fast non-reasoning model, then restart \
                 WikiLens to re-enable rewrites. Asks still work — they just skip the rewrite."
            );
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_not_tripped() {
        let state = AppState::new();
        assert!(!state.rewrite_breaker_tripped());
    }

    #[test]
    fn two_consecutive_failures_trip_the_breaker() {
        let state = AppState::new();
        state.record_rewrite_outcome(false);
        assert!(!state.rewrite_breaker_tripped(), "one strike must not trip");
        state.record_rewrite_outcome(false);
        assert!(state.rewrite_breaker_tripped());
    }

    #[test]
    fn a_success_fully_resets_the_counter() {
        let state = AppState::new();
        state.record_rewrite_outcome(false);
        state.record_rewrite_outcome(true);
        state.record_rewrite_outcome(false);
        assert!(
            !state.rewrite_breaker_tripped(),
            "non-consecutive failures must not trip"
        );
    }

    #[test]
    fn tripped_stays_tripped() {
        let state = AppState::new();
        state.record_rewrite_outcome(false);
        state.record_rewrite_outcome(false);
        // Attempts shouldn't happen after the trip, but even if one slipped
        // through and failed, the breaker must not un-trip.
        state.record_rewrite_outcome(false);
        assert!(state.rewrite_breaker_tripped());
    }
}
