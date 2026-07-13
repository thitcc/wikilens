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

use base64::Engine as _;
use futures_util::StreamExt;

use crate::error::AppError;
use crate::providers::{Provider, ProviderKind};
use crate::wiki::fetch::WikiPage;

const MAX_TOKENS: u32 = 1024;

/// Verbatim system prompt from the scaffold spec (§4.6). Do not edit casually —
/// it is the guardrail that keeps answers grounded in the provided wiki text.
const SYSTEM_PROMPT: &str = "You are a game-wiki assistant embedded in an in-game overlay. Answer the player's question using ONLY the wiki excerpts provided below. If the excerpts do not contain the answer, say so plainly and suggest what to search instead. Be concise and practical — the player is mid-game. Use short markdown: bold key items, small lists when comparing options. Do not mention that you were given excerpts; just answer. Wiki excerpts follow, each with its page title.";

/// Appended to `SYSTEM_PROMPT` only when a screenshot is attached — so every
/// text-only ask still sends the byte-identical prompt it always has. It keeps
/// the wiki-grounded guardrail intact while telling the model the image is for
/// identifying *what* the question is about, not a new source of facts.
const SCREENSHOT_ADDENDUM: &str = "The player has attached a screenshot of their game. Use it only to identify what the question is about — the item, enemy, location, or situation shown — and then answer from the wiki excerpts as usual. The excerpts remain your only source of facts. If the screenshot shows something the excerpts do not cover, say plainly that the wiki text provided doesn't cover what's on screen, and suggest what to search instead. Do not describe the screenshot back to the player unless they ask.";

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
#[allow(clippy::too_many_arguments)] // one arg per request ingredient; callers pass them all anyway
pub async fn answer_streaming<F>(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    question: &str,
    pages: &[WikiPage],
    image_png: Option<&[u8]>,
    mut on_delta: F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    let request = match provider.kind {
        ProviderKind::Anthropic => {
            build_anthropic_request(client, provider, model, api_key, question, pages, image_png)
        }
        ProviderKind::OpenAiCompatible => {
            build_openai_request(client, provider, model, api_key, question, pages, image_png)
        }
    };

    let resp = request.send().await?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        // With an image attached, translate the two known non-vision rejections
        // into plain language; everything else keeps the verbatim error body.
        if image_png.is_some() {
            if let Some(message) = friendly_image_error(provider, model, status, &body) {
                return Err(AppError::VisionUnsupported(message));
            }
        }
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
    image_png: Option<&[u8]>,
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": system_prompt(image_png.is_some()),
        "messages": [
            { "role": "user", "content": build_user_content(provider.kind, question, pages, image_png) }
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
    image_png: Option<&[u8]>,
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "messages": [
            { "role": "system", "content": system_prompt(image_png.is_some()) },
            { "role": "user", "content": build_user_content(provider.kind, question, pages, image_png) }
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

/// Translate the two known non-vision rejection bodies into plain language.
/// Called only when an image was attached, so it never touches text-only asks;
/// `None` for anything else, leaving the verbatim `AppError::Llm` backstop in
/// place. Substring matching is brittle by nature — acceptable because it's
/// image-conditional and the fallback stays reachable (see
/// `vault/2026-07-06_image-attach-guardrails.md`).
fn friendly_image_error(provider: &Provider, model: &str, status: u16, body: &str) -> Option<String> {
    // OpenAI-compatible endpoints reject the `image_url` content part with a raw
    // serde error (DeepSeek, live-captured 400 — the only text-only provider).
    if body.contains("unknown variant `image_url`") {
        return Some(format!(
            "{} models can't read images. Remove the screenshot or switch providers.",
            provider.name
        ));
    }
    // OpenRouter's routing-time rejection when no endpoint supports image input.
    if status == 404 && body.contains("support image input") {
        return Some(format!(
            "{model} on OpenRouter can't read images. Remove the screenshot or pick a model with the Image badge."
        ));
    }
    None
}

fn parse_line(kind: ProviderKind, line: &str) -> SseLine {
    match kind {
        ProviderKind::Anthropic => parse_anthropic_sse_line(line),
        ProviderKind::OpenAiCompatible => parse_openai_sse_line(line),
    }
}

/// The system prompt for this request: `SYSTEM_PROMPT` verbatim, plus the
/// screenshot addendum when (and only when) an image is attached. A text-only
/// ask therefore sends exactly the prompt it always has.
fn system_prompt(has_image: bool) -> String {
    if has_image {
        format!("{SYSTEM_PROMPT}\n\n{SCREENSHOT_ADDENDUM}")
    } else {
        SYSTEM_PROMPT.to_string()
    }
}

/// Build the user message `content`. Without an image this is the plain
/// excerpts+question string — byte-identical to the original request. With one
/// it becomes a provider-shaped content array with the image *before* the text
/// (both APIs weight a leading image correctly): Anthropic takes a base64
/// `image` block, OpenAI-compatible providers take an `image_url` data-URI part.
fn build_user_content(
    kind: ProviderKind,
    question: &str,
    pages: &[WikiPage],
    image_png: Option<&[u8]>,
) -> serde_json::Value {
    let text = build_user_message(question, pages);
    let Some(png) = image_png else {
        return serde_json::Value::String(text);
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    match kind {
        ProviderKind::Anthropic => serde_json::json!([
            {
                "type": "image",
                "source": { "type": "base64", "media_type": "image/png", "data": b64 }
            },
            { "type": "text", "text": text }
        ]),
        ProviderKind::OpenAiCompatible => serde_json::json!([
            {
                "type": "image_url",
                "image_url": { "url": format!("data:image/png;base64,{b64}") }
            },
            { "type": "text", "text": text }
        ]),
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

/// Max output tokens for the query-rewrite completion — the JSON object is tiny,
/// so keep the cap tight. The rewrite is meant to run on a fast *non-reasoning*
/// model (`WIKILENS_REWRITE_MODEL`); a tight cap also makes a mis-configured
/// reasoning model fail fast (empty `content`, seen in the trace) rather than
/// reasoning for many seconds.
const REWRITE_MAX_TOKENS: u32 = 256;

/// System prompt for the lazy query-rewrite (Phase 3 of the retrieval-quality
/// plan). The player's own wiki search found nothing — usually a typo, a
/// paraphrase, or an item/character the wiki names differently. Turn the question
/// into a few concrete wiki queries. Strict JSON out; `parse_rewrite_queries`
/// tolerates fences/prose defensively.
const REWRITE_SYSTEM_PROMPT: &str = "You convert a player's question into search queries for a specific game's wiki. Their own search returned nothing — usually a typo, a paraphrase, or an item/character the wiki names differently. Reply with ONLY a compact JSON object of the form {\"queries\":[\"...\"]}: 1 to 3 short keyword queries, best guess first, exact proper nouns / item / enemy names preferred. No prose, no markdown, no code fences.";

/// Rewrite a failed question into candidate wiki search queries via a cheap,
/// non-streaming model call (the reply is tiny). Returns the parsed queries; an
/// empty vec means "no usable rewrite" and the caller falls back to the raw query.
/// Only invoked on the zero-hit dead-end, so its cost lands only when a search has
/// already failed. See `vault/2026-07-07_llm-query-rewrite-in-retrieval.md`.
pub async fn rewrite_query(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    game: &str,
    question: &str,
) -> Result<Vec<String>, AppError> {
    let user = format!("Game: {game}\nPlayer question: {question}");
    let request = match provider.kind {
        ProviderKind::Anthropic => {
            build_completion_anthropic(client, provider, model, api_key, REWRITE_SYSTEM_PROMPT, &user)
        }
        ProviderKind::OpenAiCompatible => {
            build_completion_openai(client, provider, model, api_key, REWRITE_SYSTEM_PROMPT, &user)
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
    let body = resp.text().await?;
    // Diagnostic: the rewrite is un-live-validated; when tracing, dump the raw
    // response so an empty/prose reply (vs. a clean JSON one) is visible.
    if std::env::var_os("WIKILENS_TRACE_RETRIEVAL").is_some() {
        let preview: String = body.chars().take(500).collect();
        eprintln!("wikilens.rewrite.raw {preview}");
    }
    let text = extract_completion_text(provider.kind, &body)?;
    Ok(parse_rewrite_queries(&text))
}

/// Non-streaming Anthropic Messages request (no wiki pages, no image) for the
/// query rewrite: same auth/version as the answer path but `stream` omitted.
fn build_completion_anthropic(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    system: &str,
    user: &str,
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": REWRITE_MAX_TOKENS,
        "system": system,
        "messages": [ { "role": "user", "content": user } ]
    });
    client
        .post(provider.endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
}

/// Non-streaming OpenAI-compatible Chat Completions request for the query rewrite.
fn build_completion_openai(
    client: &reqwest::Client,
    provider: &Provider,
    model: &str,
    api_key: &str,
    system: &str,
    user: &str,
) -> reqwest::RequestBuilder {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": REWRITE_MAX_TOKENS,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
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

/// Pull the assistant's text out of a non-streaming completion body, branching on
/// the provider's response shape (Anthropic `content[].text`; OpenAI-compatible
/// `choices[0].message.content`).
fn extract_completion_text(kind: ProviderKind, body: &str) -> Result<String, AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;
    let text = match kind {
        ProviderKind::Anthropic => json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
            }),
        ProviderKind::OpenAiCompatible => json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str()),
    };
    text.map(str::to_string)
        .ok_or_else(|| AppError::Parse("no text in completion response".into()))
}

/// Extract the `queries` array from the model's reply, tolerating code fences or
/// stray prose by scanning for the outermost `{`…`}`. Returns trimmed, de-duped
/// (case-insensitive), non-empty queries; an empty vec on any shape it can't read,
/// so the caller falls back to the raw query.
pub fn parse_rewrite_queries(text: &str) -> Vec<String> {
    let slice = extract_json_object(text).unwrap_or(text);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(slice) else {
        return Vec::new();
    };
    let Some(items) = value.get("queries").and_then(|q| q.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if let Some(s) = item.as_str() {
            let s = s.trim();
            if !s.is_empty() && !out.iter().any(|e| e.eq_ignore_ascii_case(s)) {
                out.push(s.to_string());
            }
        }
    }
    out
}

/// The outermost `{`…`}` slice of `text`, if any — lets `parse_rewrite_queries`
/// survive a model that wraps its JSON in prose or ```json fences.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

#[cfg(test)]
mod rewrite_tests {
    use super::*;

    #[test]
    fn parses_a_plain_queries_object() {
        let out = parse_rewrite_queries(r#"{"queries":["Arcane Persistence","Arcane"]}"#);
        assert_eq!(
            out,
            vec!["Arcane Persistence".to_string(), "Arcane".to_string()]
        );
    }

    #[test]
    fn tolerates_code_fences_and_prose() {
        let text = "Sure!\n```json\n{ \"queries\": [\"Last Gasp\"] }\n```";
        assert_eq!(parse_rewrite_queries(text), vec!["Last Gasp".to_string()]);
    }

    #[test]
    fn trims_dedupes_and_drops_blanks() {
        let out = parse_rewrite_queries(r#"{"queries":["  Nova  ","nova","", "Nova Prime"]}"#);
        assert_eq!(out, vec!["Nova".to_string(), "Nova Prime".to_string()]);
    }

    #[test]
    fn empty_on_missing_key_or_garbage() {
        assert!(parse_rewrite_queries(r#"{"foo":1}"#).is_empty());
        assert!(parse_rewrite_queries("not json at all").is_empty());
        assert!(parse_rewrite_queries(r#"{"queries":"notarray"}"#).is_empty());
        assert!(parse_rewrite_queries(r#"{"queries":[1,2,3]}"#).is_empty());
    }

    #[test]
    fn extracts_completion_text_per_provider() {
        let anthropic = r#"{"content":[{"type":"text","text":"hi"}]}"#;
        assert_eq!(
            extract_completion_text(ProviderKind::Anthropic, anthropic).unwrap(),
            "hi"
        );
        let openai = r#"{"choices":[{"message":{"role":"assistant","content":"hey"}}]}"#;
        assert_eq!(
            extract_completion_text(ProviderKind::OpenAiCompatible, openai).unwrap(),
            "hey"
        );
        // Missing text is a parse error, not a panic.
        assert!(extract_completion_text(ProviderKind::Anthropic, r#"{"content":[]}"#).is_err());
    }
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

    // ---- User content (image vs text-only) ----

    #[test]
    fn user_content_without_image_is_the_plain_string() {
        let pages = vec![page("Winter", "Cold season.")];
        let expected = serde_json::Value::String(build_user_message("q?", &pages));
        // Text-only content is identical across both provider shapes — a
        // regression here would change what today's text asks send.
        assert_eq!(
            build_user_content(ProviderKind::Anthropic, "q?", &pages, None),
            expected
        );
        assert_eq!(
            build_user_content(ProviderKind::OpenAiCompatible, "q?", &pages, None),
            expected
        );
    }

    #[test]
    fn anthropic_image_content_puts_base64_image_before_text() {
        let pages = vec![page("Boss", "A tough foe.")];
        let content =
            build_user_content(ProviderKind::Anthropic, "what is this?", &pages, Some(b"PNGDATA"));
        let arr = content.as_array().unwrap();
        assert_eq!(arr[0]["type"], "image");
        assert_eq!(arr[0]["source"]["type"], "base64");
        assert_eq!(arr[0]["source"]["media_type"], "image/png");
        assert_eq!(
            arr[0]["source"]["data"],
            base64::engine::general_purpose::STANDARD.encode(b"PNGDATA")
        );
        assert_eq!(arr[1]["type"], "text");
        assert!(arr[1]["text"].as_str().unwrap().contains("what is this?"));
    }

    #[test]
    fn openai_image_content_uses_data_uri_before_text() {
        let pages = vec![page("Boss", "A tough foe.")];
        let content = build_user_content(
            ProviderKind::OpenAiCompatible,
            "what is this?",
            &pages,
            Some(b"PNGDATA"),
        );
        let arr = content.as_array().unwrap();
        assert_eq!(arr[0]["type"], "image_url");
        let url = arr[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.ends_with(&base64::engine::general_purpose::STANDARD.encode(b"PNGDATA")));
        assert_eq!(arr[1]["type"], "text");
    }

    #[test]
    fn system_prompt_gains_addendum_only_with_image() {
        assert_eq!(system_prompt(false), SYSTEM_PROMPT);
        let with = system_prompt(true);
        assert!(with.starts_with(SYSTEM_PROMPT));
        assert!(with.contains("attached a screenshot"));
        assert_ne!(with, SYSTEM_PROMPT);
    }

    // ---- Friendly non-vision error mapping ----

    #[test]
    fn friendly_error_maps_deepseek_serde_body() {
        let provider = crate::providers::find_provider("deepseek").unwrap();
        // The live-captured DeepSeek 400 body.
        let body = "Failed to deserialize the JSON body into the target type: \
                    messages[0]: unknown variant `image_url`, expected `text`";
        let msg = friendly_image_error(provider, "deepseek-v4-flash", 400, body).unwrap();
        assert!(msg.contains("DeepSeek"));
        assert!(msg.contains("can't read images"));
    }

    #[test]
    fn friendly_error_maps_openrouter_404_and_names_the_model() {
        let provider = crate::providers::find_provider("openrouter").unwrap();
        let body = r#"{"error":{"message":"No endpoints found that support image input"}}"#;
        let msg = friendly_image_error(provider, "some/text-only-model", 404, body).unwrap();
        assert!(msg.contains("some/text-only-model"));
        assert!(msg.contains("can't read images"));
    }

    #[test]
    fn friendly_error_ignores_unrelated_failures() {
        let provider = crate::providers::find_provider("deepseek").unwrap();
        assert!(friendly_image_error(provider, "m", 401, "Invalid API key").is_none());
        assert!(friendly_image_error(provider, "m", 500, "internal server error").is_none());
        // A 404 without the image-input marker is not ours to translate.
        assert!(friendly_image_error(provider, "m", 404, "model not found").is_none());
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

/// Offline wiremock tier: the byte-buffered streaming loop and error routing
/// against a real HTTP round-trip (`vault/2026-07-13_rust-http-mock-integration-tests.md`).
/// wiremock delivers bodies whole, so chunk-*boundary* splits aren't forced
/// here — line splitting, ordering, and termination are what these prove.
#[cfg(test)]
mod http_tests {
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::{anthropic_delta, mock_provider, openai_delta, sse_body, wiki_page};

    /// Common act: stream an answer from the mock provider, collecting deltas.
    async fn ask(
        provider: &Provider,
        image_png: Option<&[u8]>,
    ) -> (Result<String, AppError>, Vec<String>) {
        let client = reqwest::Client::new();
        let pages = [wiki_page("Fishing", "Use a fishing rod at water.")];
        let mut deltas: Vec<String> = Vec::new();
        let result = answer_streaming(
            &client,
            provider,
            "mock-model",
            "test-key",
            "how do I fish?",
            &pages,
            image_png,
            |d| deltas.push(d.to_string()),
        )
        .await;
        (result, deltas)
    }

    #[tokio::test]
    async fn openai_stream_accumulates_deltas_and_stops_at_done() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        // The matchers double as request-builder assertions: Bearer auth, the
        // stream flag, and the max_tokens cap must all be on the wire.
        Mock::given(method("POST"))
            .and(path("/chat"))
            .and(header("Authorization", "Bearer test-key"))
            .and(body_partial_json(json!({
                "model": "mock-model", "stream": true, "max_tokens": 1024
            })))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                sse_body(&[
                    r#"data: {"choices":[{"delta":{"role":"assistant"},"finish_reason":null}]}"#,
                    &openai_delta("Hel"),
                    &openai_delta("lo"),
                    r#"data: {"choices":[],"usage":{"total_tokens":5}}"#,
                    "data: [DONE]",
                    // Anything after [DONE] must never be read.
                    &openai_delta("NEVER"),
                ]),
                "text/event-stream",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let (result, deltas) = ask(&provider, None).await;
        assert_eq!(result.unwrap(), "Hello");
        assert_eq!(deltas, vec!["Hel", "lo"]);
    }

    #[tokio::test]
    async fn openai_extra_headers_are_sent() {
        let server = MockServer::start().await;
        let provider = Provider {
            extra_headers: &[
                ("HTTP-Referer", "https://wikilens.app"),
                ("X-Title", "WikiLens"),
            ],
            ..mock_provider(
                ProviderKind::OpenAiCompatible,
                &format!("{}/chat", server.uri()),
                &server.uri(),
            )
        };
        Mock::given(method("POST"))
            .and(path("/chat"))
            .and(header("HTTP-Referer", "https://wikilens.app"))
            .and(header("X-Title", "WikiLens"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(sse_body(&["data: [DONE]"]), "text/event-stream"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let (result, deltas) = ask(&provider, None).await;
        assert_eq!(result.unwrap(), "");
        assert!(deltas.is_empty());
    }

    #[tokio::test]
    async fn anthropic_stream_close_without_done_returns_accumulated_text() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::Anthropic,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        // Anthropic has no `[DONE]`; the loop must finish on stream close.
        // "Caffè" keeps a multi-byte char flowing through the byte buffer.
        Mock::given(method("POST"))
            .and(path("/chat"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(body_partial_json(json!({ "stream": true, "max_tokens": 1024 })))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                sse_body(&[
                    "event: message_start",
                    r#"data: {"type":"message_start","message":{"id":"msg_1"}}"#,
                    "event: content_block_start",
                    r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
                    "event: content_block_delta",
                    &anthropic_delta("Caffè "),
                    "event: content_block_delta",
                    &anthropic_delta("latte"),
                    "event: message_stop",
                    r#"data: {"type":"message_stop"}"#,
                ]),
                "text/event-stream",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let (result, deltas) = ask(&provider, None).await;
        assert_eq!(result.unwrap(), "Caffè latte");
        assert_eq!(deltas, vec!["Caffè ", "latte"]);
    }

    #[tokio::test]
    async fn openai_midstream_error_aborts_with_status_200() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                sse_body(&[
                    &openai_delta("Hel"),
                    r#"data: {"error":{"message":"rate limited","code":429}}"#,
                    &openai_delta("lo"),
                ]),
                "text/event-stream",
            ))
            .mount(&server)
            .await;

        let (result, deltas) = ask(&provider, None).await;
        match result.unwrap_err() {
            AppError::Llm {
                provider,
                status,
                body,
            } => {
                assert_eq!(provider, "MockProv");
                // 200 is the honest status: HTTP succeeded, the stream failed.
                assert_eq!(status, 200);
                assert!(body.contains("rate limited"), "body: {body}");
            }
            other => panic!("expected AppError::Llm, got {other:?}"),
        }
        // Text before the failure was already forwarded to the UI.
        assert_eq!(deltas, vec!["Hel"]);
    }

    #[tokio::test]
    async fn non_2xx_maps_to_llm_error_with_verbatim_body() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Invalid API key"))
            .mount(&server)
            .await;

        let (result, deltas) = ask(&provider, None).await;
        match result.unwrap_err() {
            AppError::Llm { status, body, .. } => {
                assert_eq!(status, 401);
                assert_eq!(body, "Invalid API key");
            }
            other => panic!("expected AppError::Llm, got {other:?}"),
        }
        assert!(deltas.is_empty());
    }

    #[tokio::test]
    async fn image_unknown_variant_rejection_maps_to_vision_unsupported() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        // The live-captured DeepSeek 400 shape.
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(400).set_body_string(
                "Failed to deserialize the JSON body into the target type: \
                 messages[0]: unknown variant `image_url`, expected `text`",
            ))
            .mount(&server)
            .await;

        let (result, _) = ask(&provider, Some(b"PNG")).await;
        match result.unwrap_err() {
            AppError::VisionUnsupported(msg) => {
                assert!(msg.contains("MockProv"), "msg: {msg}");
                assert!(msg.contains("can't read images"), "msg: {msg}");
            }
            other => panic!("expected VisionUnsupported, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn image_404_support_image_input_maps_to_vision_unsupported() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        // OpenRouter's routing-time rejection.
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"message":"No endpoints found that support image input"}}"#,
            ))
            .mount(&server)
            .await;

        let (result, _) = ask(&provider, Some(b"PNG")).await;
        match result.unwrap_err() {
            AppError::VisionUnsupported(msg) => {
                assert!(msg.contains("mock-model"), "msg: {msg}");
                assert!(msg.contains("can't read images"), "msg: {msg}");
            }
            other => panic!("expected VisionUnsupported, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn image_with_unrelated_error_keeps_llm_backstop() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &format!("{}/chat", server.uri()),
            &server.uri(),
        );
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
            .mount(&server)
            .await;

        // An image is attached, but the failure isn't a vision rejection — the
        // verbatim Llm backstop must survive.
        let (result, _) = ask(&provider, Some(b"PNG")).await;
        match result.unwrap_err() {
            AppError::Llm { status, body, .. } => {
                assert_eq!(status, 500);
                assert_eq!(body, "internal server error");
            }
            other => panic!("expected AppError::Llm, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod sse_property_tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_support::{arbitrary_text, marker_soup};

    const SSE_MARKERS: &[&str] = &[
        "data:", "data: ", ": OPENROUTER PROCESSING", ":", "event: ", "[DONE]",
        r#"{"choices":["#, r#"{"type":"content_block_delta""#, r#""delta":{"#,
        r#""content":"#, r#""text":"#, r#""error":{"#, r#""finish_reason":"#,
        "null", "{", "}", "]", "\"", "\\",
    ];

    proptest! {
        #[test]
        fn sse_parsers_never_panic_on_arbitrary_lines(line in arbitrary_text()) {
            let _ = parse_openai_sse_line(&line);
            let _ = parse_anthropic_sse_line(&line);
        }

        #[test]
        fn sse_parsers_never_panic_on_sse_shaped_soup(line in marker_soup(SSE_MARKERS)) {
            let _ = parse_openai_sse_line(&line);
            let _ = parse_anthropic_sse_line(&line);
        }

        #[test]
        fn lines_without_a_data_prefix_are_ignored(line in arbitrary_text()) {
            prop_assume!(!line.starts_with("data:"));
            prop_assert_eq!(parse_openai_sse_line(&line), SseLine::Ignore);
            prop_assert_eq!(parse_anthropic_sse_line(&line), SseLine::Ignore);
        }
    }
}
