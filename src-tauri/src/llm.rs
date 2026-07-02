//! Anthropic Messages API streaming client.
//!
//! The wiki excerpts are the source of truth; the system prompt forbids the
//! model from answering beyond them. The API key is passed in from the command
//! layer and never stored or logged here.

use futures_util::StreamExt;

use crate::error::AppError;
use crate::wiki::fetch::WikiPage;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-haiku-4-5-20251001";
const MAX_TOKENS: u32 = 1024;

/// Verbatim system prompt from the scaffold spec (§4.6). Do not edit casually —
/// it is the guardrail that keeps answers grounded in the provided wiki text.
const SYSTEM_PROMPT: &str = "You are a game-wiki assistant embedded in an in-game overlay. Answer the player's question using ONLY the wiki excerpts provided below. If the excerpts do not contain the answer, say so plainly and suggest what to search instead. Be concise and practical — the player is mid-game. Use short markdown: bold key items, small lists when comparing options. Do not mention that you were given excerpts; just answer. Wiki excerpts follow, each with its page title.";

/// Stream an answer from Anthropic. Each text delta is handed to `on_delta` as
/// it arrives; the full accumulated answer is returned at the end.
pub async fn answer_streaming<F>(
    client: &reqwest::Client,
    api_key: &str,
    question: &str,
    pages: &[WikiPage],
    mut on_delta: F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    let request_body = serde_json::json!({
        "model": MODEL,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": build_user_message(question, pages) }
        ]
    });

    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Anthropic { status, body });
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
            if let Some(delta) = parse_sse_line(line.trim_end()) {
                on_delta(&delta);
                answer.push_str(&delta);
            }
        }
    }

    Ok(answer)
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

/// Extract the text from a single SSE line, or `None` if it carries no text
/// delta (event lines, pings, non-text deltas, blank lines, `[DONE]`).
fn parse_sse_line(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(data).ok()?;
    if json.get("type")?.as_str()? != "content_block_delta" {
        return None;
    }
    let delta = json.get("delta")?;
    if delta.get("type")?.as_str()? != "text_delta" {
        return None;
    }
    delta.get("text")?.as_str().map(str::to_string)
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

    #[test]
    fn parses_text_delta() {
        let line = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        assert_eq!(parse_sse_line(line), Some("Hello".to_string()));
    }

    #[test]
    fn ignores_non_text_events() {
        assert_eq!(parse_sse_line("event: content_block_delta"), None);
        assert_eq!(
            parse_sse_line(r#"data: {"type":"message_start","message":{}}"#),
            None
        );
        assert_eq!(
            parse_sse_line(r#"data: {"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{}"}}"#),
            None
        );
        assert_eq!(parse_sse_line(""), None);
        assert_eq!(parse_sse_line("data: [DONE]"), None);
    }
}
