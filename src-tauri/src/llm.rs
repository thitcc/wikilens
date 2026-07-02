//! LLM streaming client for all providers.
//!
//! Two wire protocols are supported (see [`crate::providers::ProviderKind`]):
//! Anthropic's native Messages API, and the OpenAI-compatible Chat Completions
//! API shared by DeepSeek and OpenRouter. A single byte-buffered SSE loop drives
//! both; only request construction and per-line parsing branch on the kind.
//!
//! The wiki excerpts are the source of truth; the system prompt forbids the
//! model from answering beyond them. The API key is passed in from the command
//! layer and never stored or logged here.

use futures_util::StreamExt;

use crate::error::AppError;
use crate::providers::{Provider, ProviderKind};
use crate::wiki::fetch::WikiPage;

const MAX_TOKENS: u32 = 1024;

/// Verbatim system prompt from the scaffold spec (§4.6). Do not edit casually —
/// it is the guardrail that keeps answers grounded in the provided wiki text.
const SYSTEM_PROMPT: &str = "You are a game-wiki assistant embedded in an in-game overlay. Answer the player's question using ONLY the wiki excerpts provided below. If the excerpts do not contain the answer, say so plainly and suggest what to search instead. Be concise and practical — the player is mid-game. Use short markdown: bold key items, small lists when comparing options. Do not mention that you were given excerpts; just answer. Wiki excerpts follow, each with its page title.";

/// Outcome of parsing one SSE line, independent of provider. Richer than a bare
/// `Option<String>` so the OpenAI path can signal explicit termination (`[DONE]`)
/// and mid-stream errors (which arrive on an already-200 response).
#[derive(Debug, PartialEq)]
enum SseLine {
    /// Text to append to the answer and forward to `on_delta`.
    Delta(String),
    /// Explicit stream terminator; stop reading and return the answer so far.
    Done,
    /// A line with no answer text (comments, keep-alives, non-text events,
    /// role-only/finish chunks, null content, blanks).
    Ignore,
    /// Provider signalled a failure mid-stream; abort with the error body.
    Error(String),
}

/// Stream an answer from the given provider/model. Each text delta is handed to
/// `on_delta` as it arrives; the full accumulated answer is returned at the end.
pub async fn answer_streaming<F>(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    question: &str,
    pages: &[WikiPage],
    mut on_delta: F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    let request = match provider.kind {
        ProviderKind::Anthropic => {
            build_anthropic_request(client, provider, model, api_key, question, pages)
        }
        ProviderKind::OpenAiCompatible => {
            build_openai_request(client, provider, model, api_key, question, pages)
        }
    };

    let resp = request.send().await?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Llm {
            provider: provider.name,
            status,
            body,
        });
    }

    // Parse the SSE stream incrementally. Chunk boundaries align with neither
    // event nor UTF-8 char boundaries, so accumulate raw BYTES and only decode
    // whole lines (everything up to a `\n`, which is ASCII) — decoding a chunk
    // directly would split a multi-byte char and produce replacement chars.
    let mut answer = String::new();
    let mut buffer: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.extend_from_slice(&chunk);

        while let Some(newline) = buffer.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buffer.drain(..=newline).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            match parse_line(provider.kind, line.trim_end()) {
                SseLine::Delta(delta) => {
                    on_delta(&delta);
                    answer.push_str(&delta);
                }
                SseLine::Done => return Ok(answer),
                SseLine::Error(message) => {
                    // The stream returned HTTP 200 then failed mid-flight; 200 is
                    // the honest status to report alongside the error body.
                    return Err(AppError::Llm {
                        provider: provider.name,
                        status: 200,
                        body: message,
                    });
                }
                SseLine::Ignore => {}
            }
        }
    }

    // Anthropic ends by closing the stream (no `[DONE]`); OpenAI-compatible
    // providers return early via `SseLine::Done`.
    Ok(answer)
}

/// Anthropic Messages API request: `x-api-key` auth and a top-level `system`.
fn build_anthropic_request(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    question: &str,
    pages: &[WikiPage],
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": build_user_message(question, pages) }
        ]
    });
    client
        .post(provider.endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
}

/// OpenAI-compatible Chat Completions request (DeepSeek, OpenRouter): `Bearer`
/// auth, the system prompt as a `system` role message, plus any provider-specific
/// extra headers (e.g. OpenRouter attribution).
fn build_openai_request(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    question: &str,
    pages: &[WikiPage],
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": build_user_message(question, pages) }
        ]
    });
    let mut request = client
        .post(provider.endpoint)
        .header("Authorization", format!("Bearer {api_key}"));
    for (name, value) in provider.extra_headers {
        request = request.header(*name, *value);
    }
    request.json(&body)
}

fn parse_line(kind: ProviderKind, line: &str) -> SseLine {
    match kind {
        ProviderKind::Anthropic => parse_anthropic_sse_line(line),
        ProviderKind::OpenAiCompatible => parse_openai_sse_line(line),
    }
}

/// Format excerpts as `## {title}\n{text}` blocks followed by the question.
fn build_user_message(question: &str, pages: &[WikiPage]) -> String {
    let mut msg = String::new();
    for page in pages {
        msg.push_str("## ");
        msg.push_str(&page.title);
        msg.push('\n');
        msg.push_str(&page.text);
        msg.push_str("\n\n");
    }
    msg.push_str("Player question: ");
    msg.push_str(question);
    msg
}

/// Parse one Anthropic SSE line, yielding the text of a `content_block_delta`
/// `text_delta` and ignoring every other event (pings, non-text deltas, blanks).
fn parse_anthropic_sse_line(line: &str) -> SseLine {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return SseLine::Ignore;
    };
    if data.is_empty() || data == "[DONE]" {
        return SseLine::Ignore;
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
        return SseLine::Ignore;
    };
    if json.get("type").and_then(|t| t.as_str()) != Some("content_block_delta") {
        return SseLine::Ignore;
    }
    let Some(delta) = json.get("delta") else {
        return SseLine::Ignore;
    };
    if delta.get("type").and_then(|t| t.as_str()) != Some("text_delta") {
        return SseLine::Ignore;
    }
    match delta.get("text").and_then(|t| t.as_str()) {
        Some(text) => SseLine::Delta(text.to_string()),
        None => SseLine::Ignore,
    }
}

/// Parse one OpenAI-compatible SSE line (DeepSeek, OpenRouter).
///
/// Handles every shape these gateways emit: `:`-prefixed comment/keep-alive lines
/// (`: OPENROUTER PROCESSING`), the `[DONE]` terminator, a top-level `error`
/// object or a `finish_reason: "error"` chunk on an HTTP-200 stream, usage-only
/// chunks (`choices: []`), and the role-only first / finish chunks where
/// `delta.content` is null or absent.
fn parse_openai_sse_line(line: &str) -> SseLine {
    // Only `data:` lines carry payload. Rejecting everything else here is what
    // skips SSE comments/keep-alives (the shared loop hands us every line).
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return SseLine::Ignore;
    };
    if data.is_empty() {
        return SseLine::Ignore;
    }
    if data == "[DONE]" {
        return SseLine::Done;
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
        return SseLine::Ignore;
    };

    // Some gateways report a failure as a top-level error object mid-stream.
    if let Some(error) = json.get("error") {
        return SseLine::Error(error.to_string());
    }

    // Usage-only / metadata chunks carry an empty `choices` array.
    let Some(choice) = json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|choices| choices.first())
    else {
        return SseLine::Ignore;
    };

    // OpenRouter signals a mid-stream failure with finish_reason "error".
    if choice.get("finish_reason").and_then(|f| f.as_str()) == Some("error") {
        return SseLine::Error(json.to_string());
    }

    // `delta.content` is null/absent on the role-only first chunk and the finish
    // chunk; treat those (and empty strings) as nothing to emit.
    match choice
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
    {
        Some(text) if !text.is_empty() => SseLine::Delta(text.to_string()),
        _ => SseLine::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(title: &str, text: &str) -> WikiPage {
        WikiPage {
            title: title.to_string(),
            text: text.to_string(),
            url: format!("https://example.com/{title}"),
        }
    }

    #[test]
    fn user_message_formats_blocks_then_question() {
        let pages = vec![page("Winter", "Cold season."), page("Crops", "Grow food.")];
        let msg = build_user_message("best winter crops?", &pages);
        assert_eq!(
            msg,
            "## Winter\nCold season.\n\n## Crops\nGrow food.\n\nPlayer question: best winter crops?"
        );
    }

    // ---- Anthropic SSE ----

    #[test]
    fn anthropic_parses_text_delta() {
        let line = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        assert_eq!(parse_anthropic_sse_line(line), SseLine::Delta("Hello".to_string()));
    }

    #[test]
    fn anthropic_ignores_non_text_events() {
        assert_eq!(parse_anthropic_sse_line("event: content_block_delta"), SseLine::Ignore);
        assert_eq!(
            parse_anthropic_sse_line(r#"data: {"type":"message_start","message":{}}"#),
            SseLine::Ignore
        );
        assert_eq!(
            parse_anthropic_sse_line(r#"data: {"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{}"}}"#),
            SseLine::Ignore
        );
        assert_eq!(parse_anthropic_sse_line(""), SseLine::Ignore);
        assert_eq!(parse_anthropic_sse_line("data: [DONE]"), SseLine::Ignore);
    }

    // ---- OpenAI-compatible SSE (DeepSeek / OpenRouter) ----

    #[test]
    fn openai_parses_content_delta() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}"#;
        assert_eq!(parse_openai_sse_line(line), SseLine::Delta("Hi".to_string()));
    }

    #[test]
    fn openai_done_terminates() {
        assert_eq!(parse_openai_sse_line("data: [DONE]"), SseLine::Done);
    }

    #[test]
    fn openai_skips_comment_and_keepalive_lines() {
        assert_eq!(parse_openai_sse_line(": OPENROUTER PROCESSING"), SseLine::Ignore);
        assert_eq!(parse_openai_sse_line(":"), SseLine::Ignore);
        assert_eq!(parse_openai_sse_line(""), SseLine::Ignore);
    }

    #[test]
    fn openai_ignores_role_only_and_null_content() {
        assert_eq!(
            parse_openai_sse_line(r#"data: {"choices":[{"delta":{"role":"assistant"},"finish_reason":null}]}"#),
            SseLine::Ignore
        );
        assert_eq!(
            parse_openai_sse_line(r#"data: {"choices":[{"delta":{"content":null}}]}"#),
            SseLine::Ignore
        );
    }

    #[test]
    fn openai_ignores_usage_only_chunk() {
        assert_eq!(
            parse_openai_sse_line(r#"data: {"choices":[],"usage":{"total_tokens":5}}"#),
            SseLine::Ignore
        );
    }

    #[test]
    fn openai_finish_reason_error_aborts() {
        let line = r#"data: {"choices":[{"delta":{},"finish_reason":"error"}]}"#;
        assert!(matches!(parse_openai_sse_line(line), SseLine::Error(_)));
    }

    #[test]
    fn openai_top_level_error_aborts() {
        let line = r#"data: {"error":{"message":"rate limited","code":429}}"#;
        assert!(matches!(parse_openai_sse_line(line), SseLine::Error(_)));
    }
}
