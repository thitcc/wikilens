//! Shared HTTP client policy — the one place every outbound request inherits
//! its hardening from (`vault/2026-07-13_wiki-fetch-hardening.md`).
//!
//! All HTTP happens in Rust (the webview has no network permissions), and all
//! of it flows through the client built here: wiki search/fetch/probe, model
//! catalogs, and LLM streams. Build clients only via [`build_client`] so new
//! code paths inherit the redirect policy and timeouts.

use std::time::Duration;

use reqwest::redirect;

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
}
