//! Shared HTTP client policy — the one place every outbound request inherits
//! its hardening from (`vault/2026-07-13_wiki-fetch-hardening.md`).
//!
//! All HTTP happens in Rust (the webview has no network permissions), and all
//! of it flows through the client built here: wiki search/fetch/probe, model
//! catalogs, and LLM streams. Build clients only via [`build_client`] so new
//! code paths inherit the redirect policy and timeouts.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::redirect;

use crate::error::AppError;

/// Redirect hop limit. reqwest's default follows 10; wikis legitimately
/// redirect (apex→www, http→https upgrades, wiki-farm moves) but never this
/// deep — past 5 hops it's a loop or someone playing games.
const MAX_REDIRECTS: usize = 5;

/// TCP/TLS connect budget, applied client-wide. Bounds the requests that have
/// no per-request `.timeout(...)` (notably the LLM answer stream) without
/// limiting how long a healthy response may run.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Max gap between reads, applied client-wide. Kills a mid-stream stall — the
/// one infinite hang `CONNECT_TIMEOUT` can't reach — while never ending a
/// long healthy stream: both SSE protocols keep-alive far below this
/// (OpenAI-compatible `:` comments, Anthropic `ping` events). Per-request
/// `.timeout(...)` still bounds total time where set.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Build the app's HTTP client: wiki User-Agent, redirect policy
/// ([`redirect_decision`]), connect/read timeouts.
///
/// Panics on builder failure instead of falling back to `Client::new()`: the
/// only failure mode is platform-TLS init, where the bare fallback would
/// panic identically — and if it somehow didn't, it would silently run
/// without the User-Agent and all of this hardening.
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(crate::wiki::USER_AGENT)
        .redirect(redirect::Policy::custom(|attempt| {
            match redirect_decision(attempt.previous(), attempt.url()) {
                Ok(()) => attempt.follow(),
                Err(reason) => attempt.error(reason),
            }
        }))
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .expect("HTTP client: platform TLS init failed")
}

/// Whole-body cap for every non-streaming response read. Wiki API responses
/// are KBs (a monster rendered page is low-single-digit MB inside JSON); the
/// ceiling case is OpenRouter's ~2 MB decompressed model catalog — 8 MiB is
/// 4× that. Pass to [`read_body_capped`] at every call site so the cap stays
/// greppable next to the request it bounds.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Cap for non-2xx error bodies, which feed user-facing `AppError::Llm` text
/// shown verbatim in the error box (previously unbounded). Real provider
/// errors are well under 16 KiB; anything past it is noise, so
/// [`read_error_body`] truncates instead of failing.
const MAX_ERROR_BODY_BYTES: usize = 16 * 1024;

/// Read a whole response body, refusing past `cap`. A `Content-Length` over
/// the cap fails fast, but that header is a hint only — gzip responses lose
/// it and `bytes_stream()` yields DECOMPRESSED bytes, so the running total on
/// the stream is the real guard (a gzip bomb is caught here, not by the
/// header).
///
/// Byte-parity note vs the `.text()` this replaces: `.text()` honors the
/// Content-Type charset, but every endpoint we call emits UTF-8 JSON, so
/// lossy UTF-8 decoding is equivalent.
pub async fn read_body_capped(resp: reqwest::Response, cap: usize) -> Result<String, AppError> {
    let host = resp
        .url()
        .host_str()
        .unwrap_or("the server")
        .to_string();
    if let Some(len) = resp.content_length() {
        if len > cap as u64 {
            return Err(AppError::BodyTooLarge(too_large_message(&host, cap)));
        }
    }
    let mut body: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len() + chunk.len() > cap {
            return Err(AppError::BodyTooLarge(too_large_message(&host, cap)));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Best-effort error-body read for user-facing display, truncated to
/// [`MAX_ERROR_BODY_BYTES`] (with a trailing `…` when cut). Never fails: a
/// transport error mid-read returns what already arrived — this replaces
/// `.text().await.unwrap_or_default()` on non-2xx paths.
pub async fn read_error_body(resp: reqwest::Response) -> String {
    let mut body: Vec<u8> = Vec::new();
    let mut truncated = false;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { break };
        let room = MAX_ERROR_BODY_BYTES - body.len();
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    let mut text = String::from_utf8_lossy(&body).into_owned();
    if truncated {
        text.push('…');
    }
    text
}

/// Complete user-facing text for a [`AppError::BodyTooLarge`] refusal.
fn too_large_message(host: &str, cap: usize) -> String {
    let limit = if cap >= 1024 * 1024 {
        format!("{} MB", cap / (1024 * 1024))
    } else {
        format!("{} KB", cap / 1024)
    };
    format!("The response from {host} was too large (over {limit}) and was not read.")
}

/// The redirect rule, split from the `Policy` so it unit-tests offline
/// (wiremock can't serve https, so the downgrade arm is unreachable in an
/// end-to-end test): follow up to [`MAX_REDIRECTS`] hops, refuse https→http
/// downgrades, checked against the previous hop so a downgrade anywhere in a
/// chain halts it while plain-http hops (localhost/LAN wikis — a supported
/// use case) stay legal.
///
/// Cross-host hops are deliberately allowed: wiki origin (built-in vs
/// user-added) is erased by fetch time, apex→www and wiki-farm migrations are
/// legitimate, and a blanket refusal buys nothing against the threat model.
/// The downgrade rule is the load-bearing part — reqwest does not strip
/// Anthropic's `x-api-key` header on redirects, and refusing the downgrade is
/// what keeps a key off plaintext HTTP.
fn redirect_decision(previous: &[reqwest::Url], next: &reqwest::Url) -> Result<(), String> {
    if previous.len() > MAX_REDIRECTS {
        return Err(format!(
            "refused to follow more than {MAX_REDIRECTS} redirects"
        ));
    }
    if previous.last().is_some_and(|p| p.scheme() == "https") && next.scheme() == "http" {
        return Err(format!(
            "refused an https->http redirect downgrade (to {})",
            next.host_str().unwrap_or("unknown host")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).expect("test url")
    }

    fn chain(urls: &[&str]) -> Vec<reqwest::Url> {
        urls.iter().map(|u| url(u)).collect()
    }

    #[test]
    fn allows_https_to_https() {
        let prev = chain(&["https://a.example/"]);
        assert_eq!(redirect_decision(&prev, &url("https://b.example/")), Ok(()));
    }

    #[test]
    fn allows_plain_http_hops_for_lan_wikis() {
        let prev = chain(&["http://192.168.1.10/"]);
        assert_eq!(
            redirect_decision(&prev, &url("http://192.168.1.10/wiki/")),
            Ok(())
        );
    }

    #[test]
    fn allows_http_to_https_upgrade() {
        let prev = chain(&["http://wiki.example/"]);
        assert_eq!(
            redirect_decision(&prev, &url("https://wiki.example/")),
            Ok(())
        );
    }

    #[test]
    fn refuses_https_to_http_downgrade() {
        let prev = chain(&["https://wiki.example/"]);
        let reason = redirect_decision(&prev, &url("http://internal.example/")).unwrap_err();
        assert!(reason.contains("downgrade"), "reason: {reason}");
    }

    #[test]
    fn refuses_downgrade_after_an_upgrade() {
        // http→https→http: the chain must halt at the downgrade hop even
        // though the original request started on plain http.
        let prev = chain(&["http://a.example/", "https://b.example/"]);
        let reason = redirect_decision(&prev, &url("http://c.example/")).unwrap_err();
        assert!(reason.contains("downgrade"), "reason: {reason}");
    }

    #[test]
    fn allows_exactly_max_redirects() {
        // previous = original + 4 intermediates → this is the 5th redirect.
        let hops: Vec<String> = (0..MAX_REDIRECTS)
            .map(|i| format!("https://example.com/hop{i}"))
            .collect();
        let prev: Vec<reqwest::Url> = hops.iter().map(|h| url(h)).collect();
        assert_eq!(prev.len(), MAX_REDIRECTS);
        assert_eq!(
            redirect_decision(&prev, &url("https://example.com/final")),
            Ok(())
        );
    }

    #[test]
    fn refuses_past_max_redirects() {
        let hops: Vec<String> = (0..=MAX_REDIRECTS)
            .map(|i| format!("https://example.com/hop{i}"))
            .collect();
        let prev: Vec<reqwest::Url> = hops.iter().map(|h| url(h)).collect();
        let reason = redirect_decision(&prev, &url("https://example.com/final")).unwrap_err();
        assert!(reason.contains("5"), "reason must name the limit: {reason}");
    }
}

/// Offline wiremock tier: the factory client's redirect behavior over real
/// HTTP round-trips (`vault/2026-07-13_wiki-fetch-hardening.md`). The
/// https→http downgrade arm can't run here (wiremock is plain-http) — that
/// rule is pinned by the pure-fn tests above.
#[cfg(test)]
mod http_tests {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::error::AppError;

    #[tokio::test]
    async fn factory_client_follows_same_origin_redirects_and_keeps_ua() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/old"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", format!("{}/new", server.uri()).as_str()),
            )
            .mount(&server)
            .await;
        // The UA matcher proves the factory's User-Agent survives the hop.
        Mock::given(method("GET"))
            .and(path("/new"))
            .and(header("user-agent", crate::wiki::USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(1)
            .mount(&server)
            .await;

        let body = build_client()
            .get(format!("{}/old", server.uri()))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "ok");
    }

    /// Pins the DELIBERATE allow of cross-host redirects (see
    /// `redirect_decision`) — if a future change tightens the policy, this
    /// test failing is the signal that the decision was revisited.
    #[tokio::test]
    async fn factory_client_follows_cross_origin_redirects() {
        let origin = MockServer::start().await;
        let target = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/moved"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", format!("{}/landed", target.uri()).as_str()),
            )
            .mount(&origin)
            .await;
        Mock::given(method("GET"))
            .and(path("/landed"))
            .respond_with(ResponseTemplate::new(200).set_body_string("landed"))
            .expect(1)
            .mount(&target)
            .await;

        let body = build_client()
            .get(format!("{}/moved", origin.uri()))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "landed");
    }

    #[tokio::test]
    async fn factory_client_refuses_a_sixth_redirect() {
        let server = MockServer::start().await;
        let uri = server.uri();
        // hop0..hop5 each redirect onward — six 302s; refusal must land on
        // the sixth, so /hop6 may never be requested.
        for i in 0..=5 {
            Mock::given(method("GET"))
                .and(path(format!("/hop{i}")))
                .respond_with(
                    ResponseTemplate::new(302)
                        .insert_header("Location", format!("{uri}/hop{}", i + 1).as_str()),
                )
                .mount(&server)
                .await;
        }
        Mock::given(method("GET"))
            .and(path("/hop6"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let err = build_client()
            .get(format!("{uri}/hop0"))
            .send()
            .await
            .unwrap_err();
        assert!(err.is_redirect(), "expected a redirect-policy error: {err}");
        // Through AppError::Http, the policy's reason must reach the user —
        // this also pins the source-chain walk in `error.rs` (reqwest 0.12
        // no longer inlines sources in Display).
        let msg = AppError::Http(err).to_string();
        assert!(
            msg.contains("refused to follow more than 5 redirects"),
            "user-facing message lost the refusal reason: {msg}"
        );
    }

    /// A 403 is a WAF/anti-bot refusal (e.g. wiki.guildwars2.com blocks all
    /// non-browser clients) — the user must see plain language, not reqwest's
    /// raw status chain (see `error.rs::http_message`).
    #[tokio::test]
    async fn forbidden_status_becomes_friendly_copy() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let err = build_client()
            .get(format!("{}/api.php", server.uri()))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap_err();
        let msg = AppError::Http(err).to_string();
        assert!(
            msg.contains("blocks automated access") && msg.contains("(403 Forbidden)"),
            "403 must map to the friendly copy: {msg}"
        );
        assert!(
            !msg.contains("Network request failed"),
            "403 must not fall through to the generic message: {msg}"
        );
    }

    #[tokio::test]
    async fn read_body_capped_returns_small_bodies_intact() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/small"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello wiki"))
            .mount(&server)
            .await;

        let resp = build_client()
            .get(format!("{}/small", server.uri()))
            .send()
            .await
            .unwrap();
        let body = read_body_capped(resp, 1024).await.unwrap();
        assert_eq!(body, "hello wiki");
    }

    #[tokio::test]
    async fn read_body_capped_refuses_an_oversized_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(vec![b'a'; 4096], "text/plain"))
            .mount(&server)
            .await;

        let resp = build_client()
            .get(format!("{}/big", server.uri()))
            .send()
            .await
            .unwrap();
        let err = read_body_capped(resp, 1024).await.unwrap_err();
        match err {
            AppError::BodyTooLarge(msg) => {
                assert!(msg.contains("too large"), "msg: {msg}");
                assert!(msg.contains("1 KB"), "msg must name the limit: {msg}");
            }
            other => panic!("expected BodyTooLarge, got {other:?}"),
        }
    }

    /// The gzip-bomb pin — the whole reason the cap counts STREAM bytes: a
    /// tiny wire payload decompresses far past the cap, and Content-Length
    /// (when present at all) only ever describes the wire size.
    #[tokio::test]
    async fn read_body_capped_counts_decompressed_bytes() {
        use std::io::Write as _;

        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&vec![b'a'; 64 * 1024]).unwrap();
        let gz = enc.finish().unwrap();
        assert!(gz.len() < 1024, "wire payload must be tiny, got {} bytes", gz.len());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bomb"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-encoding", "gzip")
                    .set_body_raw(gz, "application/json"),
            )
            .mount(&server)
            .await;

        let resp = build_client()
            .get(format!("{}/bomb", server.uri()))
            .send()
            .await
            .unwrap();
        let err = read_body_capped(resp, 16 * 1024).await.unwrap_err();
        assert!(matches!(err, AppError::BodyTooLarge(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn read_error_body_truncates_instead_of_failing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/err"))
            .respond_with(ResponseTemplate::new(500).set_body_raw(vec![b'e'; 64 * 1024], "text/plain"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/small-err"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream down"))
            .mount(&server)
            .await;

        let resp = build_client()
            .get(format!("{}/err", server.uri()))
            .send()
            .await
            .unwrap();
        let text = read_error_body(resp).await;
        assert!(text.ends_with('…'), "truncated body must end with an ellipsis");
        assert!(text.len() <= 16 * 1024 + '…'.len_utf8());

        let resp = build_client()
            .get(format!("{}/small-err", server.uri()))
            .send()
            .await
            .unwrap();
        assert_eq!(read_error_body(resp).await, "upstream down");
    }
}
