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
}

/// Lets `#[tauri::command] -> Result<T, String>` use `?` on `AppError` values.
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
