//! Library error type. Internal modules use `AppError`; `#[tauri::command]`
//! functions convert it to a user-readable `String` at the IPC boundary.

/// All fallible operations in the wiki/LLM layers return this.
///
/// Every `Display` message is written to be shown directly to the user, so the
/// frontend can render `AppError`-derived strings verbatim.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{}", http_message(.0))]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    Parse(String),

    #[error("Unknown game: {0}")]
    UnknownGame(String),

    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    #[error("Please type a question first.")]
    EmptyQuestion,

    /// No stored key for the provider an ask or model-list call needs. The
    /// field is `&'static str` by design: a runtime key value structurally
    /// cannot be embedded here (pinned in `config_guardrails.rs`). Keys are
    /// read at ask time, so no restart is needed after adding one.
    #[error("No API key for {provider} yet — add one in Settings → API keys.")]
    MissingApiKey { provider: &'static str },

    #[error("The {provider} API returned an error ({status}): {body}")]
    Llm {
        provider: &'static str,
        status: u16,
        body: String,
    },

    /// Screenshot capture failed (no monitor under the cursor, the OS refused
    /// the grab on a secure/DRM surface, a too-small selection, …). The message
    /// is already complete user-facing text shown verbatim in the error box.
    #[error("{0}")]
    Capture(String),

    /// An image was sent to a model that can't read images, translated from the
    /// provider's raw rejection into plain language (see `llm::friendly_image_error`).
    /// The message is already complete user-facing text.
    #[error("{0}")]
    VisionUnsupported(String),

    /// Probe validation failed for a user-added wiki. The message is already
    /// complete user-facing text ("Couldn't find a MediaWiki API at …").
    #[error("{0}")]
    Probe(String),

    /// A user-added game request that is wrong before any network I/O
    /// (duplicate name, removing a built-in, …). Message shown verbatim.
    #[error("{0}")]
    InvalidGame(String),

    #[error("Couldn't save your games: {0}")]
    Storage(String),

    #[error("Couldn't save your settings: {0}")]
    Settings(String),

    #[error("Couldn't save your API keys: {0}")]
    Keys(String),

    /// A hotkey change that can't be applied (unparseable combo, conflict with
    /// the other shortcut, another app owns the combo). The message is already
    /// complete user-facing text shown in the settings popover.
    #[error("{0}")]
    Hotkey(String),

    /// A response body or stream exceeded its byte cap and was abandoned
    /// (see `crate::http`). The message is already complete user-facing text.
    #[error("{0}")]
    BodyTooLarge(String),
}

/// reqwest 0.12 (hyper 1.x) stopped inlining error sources in `Display`, so a
/// bare `{0}` renders a redirect-policy refusal as just "error following
/// redirect for url (…)" and a timeout without its cause — the actual reason
/// lives in `.source()`. Walk the chain so the user sees why.
fn error_chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut source = err.source();
    while let Some(s) = source {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    msg
}

/// User-facing copy for `AppError::Http`. A 403 is a deliberate refusal —
/// WAF-guarded wikis (e.g. wiki.guildwars2.com) block all non-browser clients,
/// and no header or retry fixes it — so say what's happening instead of echoing
/// reqwest's raw chain. Status-bearing errors only come from
/// `error_for_status()` on the wiki paths (search/fetch/titles); the
/// LLM/model/probe paths never build one, so "This wiki" is accurate.
fn http_message(err: &reqwest::Error) -> String {
    if err.status() == Some(reqwest::StatusCode::FORBIDDEN) {
        return "This wiki refused WikiLens's request (403 Forbidden) — it looks like it \
                blocks automated access. Its pages still open normally in a browser."
            .to_string();
    }
    format!("Network request failed: {}", error_chain(err))
}

/// Lets `#[tauri::command] -> Result<T, String>` use `?` on `AppError` values.
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
