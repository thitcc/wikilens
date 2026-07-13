//! Shared fixtures for the offline HTTP-mock test tier (wiremock; see
//! `vault/2026-07-13_rust-http-mock-integration-tests.md`). Compiled only
//! under `#[cfg(test)]` — the `mod` declaration in `lib.rs` is gated.

use crate::providers::{Provider, ProviderKind};
use crate::wiki::fetch::WikiPage;
use crate::wiki::games::GameWiki;

/// Leak a runtime string (a wiremock URI, minted per test on a random port)
/// into the `&'static str` the `Provider` registry struct requires. A few
/// bytes per test in a short-lived test binary — deliberately cheaper than
/// making the production endpoints non-static just for tests.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A `Provider` literal pointing at a mock server instead of a live host.
/// Kind decides the wire protocol under test; everything else is inert data.
pub fn mock_provider(kind: ProviderKind, endpoint: &str, models_endpoint: &str) -> Provider {
    Provider {
        id: "mock",
        name: "MockProv",
        kind,
        endpoint: leak(endpoint.to_string()),
        api_key_env: "WIKILENS_TEST_MOCK_KEY",
        model_env: "WIKILENS_TEST_MOCK_MODEL",
        default_model: "mock-model",
        models_endpoint: leak(models_endpoint.to_string()),
        models_need_key: false,
        curated_models: &[],
        extra_headers: &[],
    }
}

/// A `GameWiki` whose `api.php` lives on a mock server.
pub fn mock_wiki(server_uri: &str) -> GameWiki {
    GameWiki {
        id: "mock".to_string(),
        name: "Mock Game".to_string(),
        api_url: format!("{server_uri}/api.php"),
        page_url: format!("{server_uri}/wiki/"),
        search_namespace: None,
    }
}

/// A minimal wiki excerpt for LLM request bodies.
pub fn wiki_page(title: &str, text: &str) -> WikiPage {
    WikiPage {
        title: title.to_string(),
        text: text.to_string(),
        url: format!("https://example.com/{title}"),
    }
}

/// Join SSE lines into one response body with a trailing newline — the
/// streaming loop only parses lines completed by a `\n`.
pub fn sse_body(lines: &[&str]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// One OpenAI-compatible content-delta SSE line.
pub fn openai_delta(text: &str) -> String {
    format!(r#"data: {{"choices":[{{"delta":{{"content":"{text}"}},"finish_reason":null}}]}}"#)
}

/// One Anthropic `content_block_delta` / `text_delta` SSE line.
pub fn anthropic_delta(text: &str) -> String {
    format!(
        r#"data: {{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":"{text}"}}}}"#
    )
}
