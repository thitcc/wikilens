//! Wiki layer: a static registry of supported game wikis plus MediaWiki
//! search + plaintext-extract clients. No caching yet (see roadmap).

pub mod fetch;
pub mod games;
pub mod search;

/// Sent on every wiki request. Fandom/MediaWiki etiquette asks for a
/// descriptive, contactable User-Agent — keep it set on the shared client.
pub const USER_AGENT: &str = "wikilens/0.1 (game overlay; contact: none)";
