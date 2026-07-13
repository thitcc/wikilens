//! Library error type. Internal modules use `AppError`; `#[tauri::command]`
//! functions convert it to a user-readable `String` at the IPC boundary.

/// All fallible operations in the wiki/LLM layers return this.
///
/// Every `Display` message is written to be shown directly to the user, so the
/// frontend can render `AppError`-derived strings verbatim.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Network request failed: {}", error_chain(.0))]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    Parse(String),

    #[error("Unknown game: {0}")]
    UnknownGame(String),

    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    #[error("Please type a question first.")]
    EmptyQuestion,

    #[error("{provider} API key not found. Set the {env_var} environment variable (or add it to a .env file), then restart WikiLens.")]
    MissingApiKey {
        provider: &'static str,
        env_var: &'static str,
    },

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

/// Lets `#[tauri::command] -> Result<T, String>` use `?` on `AppError` values.
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
