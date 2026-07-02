//! Library error type. Internal modules use `AppError`; `#[tauri::command]`
//! functions convert it to a user-readable `String` at the IPC boundary.

/// All fallible operations in the wiki/LLM layers return this.
///
/// Every `Display` message is written to be shown directly to the user, so the
/// frontend can render `AppError`-derived strings verbatim.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Network request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse wiki response: {0}")]
    Parse(String),

    #[error("Unknown game: {0}")]
    UnknownGame(String),

    #[error("Please type a question first.")]
    EmptyQuestion,

    #[error("Anthropic API key not found. Set the ANTHROPIC_API_KEY environment variable, then restart WikiLens.")]
    MissingApiKey,

    #[error("The Anthropic API returned an error ({status}): {body}")]
    Anthropic { status: u16, body: String },
}

/// Lets `#[tauri::command] -> Result<T, String>` use `?` on `AppError` values.
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
