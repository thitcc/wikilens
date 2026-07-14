//! MediaWiki plaintext client for a set of titles.
//!
//! Primary path: rendered HTML (`action=parse&prop=text`, one sequential
//! request per page with a timeout) reduced by [`crate::wiki::html`] — this
//! keeps infobox rows and data tables, which raw wikitext cannot contain (the
//! data is template/Lua-generated server-side). Pages whose parse call fails
//! or times out fall back to one batched raw-wikitext request
//! (`prop=revisions`) cleaned by [`crate::wiki::wikitext`] — prose only, so
//! answers never get worse than the pre-HTML pipeline. `prop=extracts` is
//! still avoided: many game wikis lack TextExtracts, and whole-article
//! extracts are capped to a single page.

use std::time::Duration;

use crate::error::AppError;
use crate::http;
use crate::wiki::games::GameWiki;

/// Per-page character cap. Bounds LLM token cost; overflow is marked truncated.
pub const MAX_PAGE_CHARS: usize = 8_000;

/// Per-request cap for page-content calls: the per-page `action=parse`
/// requests and the batched `prop=revisions` fallback. Fandom occasionally
/// stalls for tens of seconds on a cold render; past this we use the wikitext
/// fallback rather than hanging the ask (the shared client bounds connects
/// and read gaps, not total request time — see `crate::http`).
const PARSE_TIMEOUT: Duration = Duration::from_secs(12);

/// A single wiki page reduced to plaintext, with a human-readable URL.
#[derive(Debug, Clone)]
pub struct WikiPage {
    pub title: String,
    pub text: String,
    pub url: String,
}

/// Fetch plaintext for the given titles: rendered HTML per page, with a
/// single batched wikitext request as the fallback for any failures.
///
/// MediaWiki etiquette: requests are issued *sequentially* — do not
/// parallelize them.
pub async fn fetch_pages(
    client: &reqwest::Client,
    wiki: &GameWiki,
    titles: &[String],
) -> Result<Vec<WikiPage>, AppError> {
    if titles.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut fallback: Vec<String> = Vec::new();
    for title in titles {
        match fetch_rendered_page(client, wiki, title).await {
            Ok(Some(page)) => out.push(page),
            Ok(None) => {} // rendered fine but reduced to nothing — skip
            Err(_) => fallback.push(title.clone()),
        }
    }

    if !fallback.is_empty() {
        match fetch_pages_wikitext(client, wiki, &fallback).await {
            Ok(mut pages) => out.append(&mut pages),
            // Surface the error only when there is nothing else to answer from.
            Err(e) if out.is_empty() => return Err(e),
            Err(_) => {}
        }
    }

    sort_by_relevance(&mut out, titles);
    Ok(out)
}

/// Fetch one page's rendered HTML (`action=parse`) and reduce it to
/// plaintext. `Ok(None)` means the page rendered but reduced to nothing.
async fn fetch_rendered_page(
    client: &reqwest::Client,
    wiki: &GameWiki,
    title: &str,
) -> Result<Option<WikiPage>, AppError> {
    let resp = client
        .get(&wiki.api_url)
        .query(&[
            ("action", "parse"),
            ("page", title),
            ("prop", "text"),
            ("formatversion", "2"),
            ("disableeditsection", "1"),
            ("disablelimitreport", "1"),
            ("redirects", "1"),
            ("format", "json"),
        ])
        .timeout(PARSE_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES).await?;

    let (final_title, html) = parse_parse_response(&body)?;
    let text = crate::wiki::html::to_plaintext(&html);
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(WikiPage {
        url: build_page_url(wiki, &final_title),
        text: truncate_text(&text),
        title: final_title,
    }))
}

/// Extract `(final_title, rendered_html)` from an `action=parse` body
/// (formatversion=2, so `parse.text` is a plain string). Split out for unit
/// testing. A missing page comes back as an `error` object → `AppError::Parse`
/// → the caller's wikitext fallback.
pub fn parse_parse_response(body: &str) -> Result<(String, String), AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;

    let parse = json
        .get("parse")
        .ok_or_else(|| AppError::Parse("missing `parse` object in response".into()))?;
    let title = parse
        .get("title")
        .and_then(|t| t.as_str())
        .ok_or_else(|| AppError::Parse("missing `parse.title` in response".into()))?;
    let text = parse
        .get("text")
        .and_then(|t| t.as_str())
        .ok_or_else(|| AppError::Parse("missing `parse.text` string in response".into()))?;
    Ok((title.to_string(), text.to_string()))
}

/// Degraded mode: raw wikitext for the given titles in one batched request.
async fn fetch_pages_wikitext(
    client: &reqwest::Client,
    wiki: &GameWiki,
    titles: &[String],
) -> Result<Vec<WikiPage>, AppError> {
    let joined = titles.join("|");
    let resp = client
        .get(&wiki.api_url)
        .query(&[
            ("action", "query"),
            ("prop", "revisions"),
            ("rvprop", "content"),
            ("rvslots", "main"),
            ("redirects", "1"),
            ("titles", joined.as_str()),
            ("format", "json"),
        ])
        .timeout(PARSE_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES).await?;

    parse_revisions_response(&body, wiki, titles)
}

/// Parse a `prop=revisions` body, converting each page's raw wikitext to
/// plaintext. Split out for unit testing.
///
/// `query.pages` comes back keyed by page id in no useful order, so results are
/// re-sorted to follow `titles` (the search-relevance ranking). Any page whose
/// title isn't in `titles` — e.g. one reached through a redirect — is appended
/// after the ranked ones.
pub fn parse_revisions_response(
    body: &str,
    wiki: &GameWiki,
    titles: &[String],
) -> Result<Vec<WikiPage>, AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;

    let pages = json
        .get("query")
        .and_then(|q| q.get("pages"))
        .and_then(|p| p.as_object())
        .ok_or_else(|| AppError::Parse("missing `query.pages` in response".into()))?;

    let mut out = Vec::new();
    for page in pages.values() {
        if page.get("missing").is_some() {
            continue;
        }
        let Some(title) = page.get("title").and_then(|t| t.as_str()) else {
            continue;
        };
        // revisions[0].slots.main["*"] holds the raw wikitext (non-formatversion-2).
        let wikitext = page
            .get("revisions")
            .and_then(|r| r.as_array())
            .and_then(|revs| revs.first())
            .and_then(|rev| rev.get("slots"))
            .and_then(|slots| slots.get("main"))
            .and_then(|main| main.get("*"))
            .and_then(|content| content.as_str())
            .unwrap_or_default();

        let text = crate::wiki::wikitext::to_plaintext(wikitext);
        if text.trim().is_empty() {
            continue;
        }
        out.push(WikiPage {
            title: title.to_string(),
            text: truncate_text(&text),
            url: build_page_url(wiki, title),
        });
    }

    sort_by_relevance(&mut out, titles);
    Ok(out)
}

/// Restore search-relevance order (titles not in the ranking sort to the end).
fn sort_by_relevance(pages: &mut [WikiPage], titles: &[String]) {
    pages.sort_by_key(|page| {
        titles
            .iter()
            .position(|t| t == &page.title)
            .unwrap_or(usize::MAX)
    });
}

/// Truncate to `MAX_PAGE_CHARS` on a char boundary, appending a marker if cut.
pub fn truncate_text(text: &str) -> String {
    if text.chars().count() <= MAX_PAGE_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_PAGE_CHARS).collect();
    format!("{head}…[truncated]")
}

/// Build the human-readable page URL: `page_url` + title with spaces as
/// underscores, then percent-encoded (MediaWiki-style: unreserved and a few
/// path-safe punctuation characters are left as-is).
pub fn build_page_url(wiki: &GameWiki, title: &str) -> String {
    let underscored = title.replace(' ', "_");
    format!("{}{}", wiki.page_url, encode_title(&underscored))
}

/// Percent-encode a wiki title for use in a URL path. Keeps the characters
/// MediaWiki leaves unescaped so links like `Category:Crops` stay clean.
fn encode_title(s: &str) -> String {
    // Unreserved (RFC 3986) plus the path-safe set MediaWiki keeps literal.
    const SAFE: &[u8] = b"-_.~:/()'!*,;@";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || SAFE.contains(&b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stardew() -> GameWiki {
        GameWiki {
            id: "stardew".to_string(),
            name: "Stardew Valley".to_string(),
            api_url: "https://stardewvalleywiki.com/mediawiki/api.php".to_string(),
            page_url: "https://stardewvalleywiki.com/".to_string(),
            search_namespace: None,
        }
    }
    fn core_keeper() -> GameWiki {
        GameWiki {
            id: "corekeeper".to_string(),
            name: "Core Keeper".to_string(),
            api_url: "https://core-keeper.fandom.com/api.php".to_string(),
            page_url: "https://core-keeper.fandom.com/wiki/".to_string(),
            search_namespace: None,
        }
    }

    #[test]
    fn url_replaces_spaces_with_underscores() {
        assert_eq!(
            build_page_url(&stardew(), "Prismatic Shard"),
            "https://stardewvalleywiki.com/Prismatic_Shard"
        );
        assert_eq!(
            build_page_url(&core_keeper(), "Copper Ore"),
            "https://core-keeper.fandom.com/wiki/Copper_Ore"
        );
    }

    #[test]
    fn url_keeps_namespace_colon_and_parens() {
        assert_eq!(
            build_page_url(&stardew(), "Category:Crops"),
            "https://stardewvalleywiki.com/Category:Crops"
        );
        assert_eq!(
            build_page_url(&stardew(), "Bream (Fish)"),
            "https://stardewvalleywiki.com/Bream_(Fish)"
        );
    }

    #[test]
    fn url_percent_encodes_unsafe_chars() {
        // Ampersand and question mark must be encoded to stay in the path.
        assert_eq!(
            build_page_url(&stardew(), "Bob & Alice?"),
            "https://stardewvalleywiki.com/Bob_%26_Alice%3F"
        );
    }

    #[test]
    fn truncate_marks_overflow() {
        let short = "hello";
        assert_eq!(truncate_text(short), "hello");

        let long = "a".repeat(MAX_PAGE_CHARS + 50);
        let out = truncate_text(&long);
        assert!(out.ends_with("…[truncated]"));
        assert_eq!(out.chars().count(), MAX_PAGE_CHARS + "…[truncated]".chars().count());
    }

    #[test]
    fn revisions_follow_search_relevance_order_not_pageid() {
        // pageids sort lexically as "1","10","2" -> Copper, Iron, Tin; the search
        // ranking is Iron, Copper, Tin — the output must follow the ranking.
        let sample = r#"{
            "query": {
                "pages": {
                    "1": { "pageid": 1, "title": "Copper Ore", "revisions": [ { "slots": { "main": { "*": "copper prose" } } } ] },
                    "10": { "pageid": 10, "title": "Iron Ore", "revisions": [ { "slots": { "main": { "*": "iron prose" } } } ] },
                    "2": { "pageid": 2, "title": "Tin Ore", "revisions": [ { "slots": { "main": { "*": "tin prose" } } } ] }
                }
            }
        }"#;
        let titles = vec!["Iron Ore".to_string(), "Copper Ore".to_string(), "Tin Ore".to_string()];
        let pages = parse_revisions_response(sample, &stardew(), &titles).unwrap();
        let ordered: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(ordered, vec!["Iron Ore", "Copper Ore", "Tin Ore"]);
    }

    #[test]
    fn parse_response_extracts_final_title_and_html() {
        // Shape of a real `action=parse&formatversion=2` response.
        let sample = r#"{"parse":{"title":"Powdermelon","pageid":15027,"redirects":[],"text":"<div class=\"mw-parser-output\"><p>Hi</p></div>"}}"#;
        let (title, html) = parse_parse_response(sample).unwrap();
        assert_eq!(title, "Powdermelon");
        assert!(html.starts_with("<div class=\"mw-parser-output\">"));
    }

    #[test]
    fn parse_response_error_body_is_parse_error() {
        // A missing page returns an error object, not a `parse` object.
        let sample = r#"{"error":{"code":"missingtitle","info":"The page you specified doesn't exist."}}"#;
        assert!(matches!(
            parse_parse_response(sample),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn missing_pages_key_is_parse_error() {
        assert!(matches!(
            parse_revisions_response(r#"{ "query": {} }"#, &stardew(), &[]),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn revisions_clean_wikitext_to_plaintext() {
        // Shape of a real `prop=revisions&rvslots=main` response (Core Keeper).
        let sample = r#"{
            "query": {
                "pages": {
                    "411": {
                        "pageid": 411,
                        "title": "Wood",
                        "revisions": [
                            { "slots": { "main": { "*": "{{Object infobox|auto=Wood}}\n\n'''Wood''' is a [[crafting material]] found in the [[Undergrounds]].\n\n== Obtaining ==\n{{Obtaining}}" } } }
                        ]
                    }
                }
            }
        }"#;
        let titles = vec!["Wood".to_string()];
        let pages = parse_revisions_response(sample, &core_keeper(), &titles).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "Wood");
        assert!(pages[0].text.contains("Wood is a crafting material found in the Undergrounds"));
        assert!(!pages[0].text.contains("{{"));
        assert!(!pages[0].text.contains("[["));
        assert_eq!(pages[0].url, "https://core-keeper.fandom.com/wiki/Wood");
    }

    #[test]
    fn revisions_skips_missing_and_empty_pages() {
        let sample = r#"{
            "query": {
                "pages": {
                    "1": { "pageid": 1, "title": "Gone", "missing": "" },
                    "2": { "pageid": 2, "title": "Templatey", "revisions": [ { "slots": { "main": { "*": "{{stub}}" } } } ] }
                }
            }
        }"#;
        let titles = vec!["Gone".to_string(), "Templatey".to_string()];
        let pages = parse_revisions_response(sample, &core_keeper(), &titles).unwrap();
        // "Gone" is missing; "Templatey" cleans to empty (only a template) — both skipped.
        assert!(pages.is_empty());
    }
}

/// Offline wiremock tier: the rendered-HTML → batched-wikitext fallback
/// ladder of `fetch_pages`, which no unit test can reach
/// (`vault/2026-07-13_rust-http-mock-integration-tests.md`). Single-word
/// titles keep query matching free of space-encoding ambiguity.
#[cfg(test)]
mod http_tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::mock_wiki;

    /// A successful `action=parse` body for one title.
    fn parse_ok(title: &str, html: &str) -> String {
        serde_json::json!({
            "parse": {
                "title": title,
                "text": format!("<div class=\"mw-parser-output\">{html}</div>"),
            }
        })
        .to_string()
    }

    /// Mount a mock for one title's `action=parse` request.
    async fn mount_parse(server: &MockServer, title: &str, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .and(query_param("action", "parse"))
            .and(query_param("page", title))
            .respond_with(response)
            .mount(server)
            .await;
    }

    /// Mount the batched `prop=revisions` fallback mock, with a call-count
    /// expectation — `expect(0)` is what proves the ladder was never climbed.
    async fn mount_revisions(server: &MockServer, response: ResponseTemplate, expected_calls: u64) {
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .and(query_param("prop", "revisions"))
            .respond_with(response)
            .expect(expected_calls)
            .mount(server)
            .await;
    }

    fn titles(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[tokio::test]
    async fn all_parses_succeed_without_touching_fallback() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        mount_parse(
            &server,
            "Wood",
            ResponseTemplate::new(200).set_body_string(parse_ok("Wood", "<p>Wood is a material.</p>")),
        )
        .await;
        mount_parse(
            &server,
            "Iron",
            ResponseTemplate::new(200).set_body_string(parse_ok("Iron", "<p>Iron is a metal.</p>")),
        )
        .await;
        mount_revisions(&server, ResponseTemplate::new(200).set_body_string("{}"), 0).await;

        let client = crate::http::build_client();
        let pages = fetch_pages(&client, &wiki, &titles(&["Wood", "Iron"])).await.unwrap();

        let got: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(got, vec!["Wood", "Iron"]);
        assert!(pages[0].text.contains("Wood is a material"));
        assert!(pages[0].url.starts_with(&wiki.page_url));
    }

    #[tokio::test]
    async fn failed_parse_falls_back_to_one_batched_revisions_request() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        mount_parse(
            &server,
            "Wood",
            ResponseTemplate::new(200).set_body_string(parse_ok("Wood", "<p>Wood is a material.</p>")),
        )
        .await;
        // A missing page is an HTTP-200 error object — the fallback must
        // trigger on the parse failure, not only on transport failures.
        mount_parse(
            &server,
            "Iron",
            ResponseTemplate::new(200).set_body_string(
                r#"{"error":{"code":"missingtitle","info":"The page you specified doesn't exist."}}"#,
            ),
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .and(query_param("prop", "revisions"))
            .and(query_param("titles", "Iron"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"query":{"pages":{"1":{"pageid":1,"title":"Iron","revisions":[{"slots":{"main":{"*":"'''Iron''' is a metal."}}}]}}}}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let pages = fetch_pages(&client, &wiki, &titles(&["Wood", "Iron"])).await.unwrap();

        let got: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(got, vec!["Wood", "Iron"], "relevance order survives the mixed paths");
        assert!(pages[1].text.contains("Iron is a metal"));
        assert!(!pages[1].text.contains("'''"), "wikitext markup cleaned");
    }

    #[tokio::test]
    async fn fallback_error_is_swallowed_when_something_else_fetched() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        mount_parse(
            &server,
            "Wood",
            ResponseTemplate::new(200).set_body_string(parse_ok("Wood", "<p>Wood is a material.</p>")),
        )
        .await;
        mount_parse(&server, "Iron", ResponseTemplate::new(500)).await;
        mount_revisions(&server, ResponseTemplate::new(500), 1).await;

        let client = crate::http::build_client();
        let pages = fetch_pages(&client, &wiki, &titles(&["Wood", "Iron"])).await.unwrap();

        // The dead fallback costs Iron, never the whole answer.
        let got: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(got, vec!["Wood"]);
    }

    #[tokio::test]
    async fn fallback_error_surfaces_when_nothing_fetched() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        mount_parse(&server, "Wood", ResponseTemplate::new(500)).await;
        mount_parse(&server, "Iron", ResponseTemplate::new(500)).await;
        mount_revisions(&server, ResponseTemplate::new(500), 1).await;

        let client = crate::http::build_client();
        let err = fetch_pages(&client, &wiki, &titles(&["Wood", "Iron"])).await.unwrap_err();
        // Everything failed — the revisions transport error must surface.
        assert!(matches!(err, AppError::Http(_)), "got {err:?}");
    }

    /// Pins the (new) `PARSE_TIMEOUT` on the batched revisions fallback,
    /// previously untimed; `--ignored` tier because it takes the full 12s.
    #[tokio::test]
    #[ignore = "slow (~12s): pins the revisions-fallback timeout"]
    async fn revisions_hang_times_out() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        // The parse 500s, so the delayed fallback is the only source and its
        // timeout error must surface (nothing else fetched to swallow it).
        mount_parse(&server, "Wood", ResponseTemplate::new(500)).await;
        mount_revisions(
            &server,
            ResponseTemplate::new(200)
                .set_body_string("{}")
                .set_delay(Duration::from_secs(15)),
            1,
        )
        .await;

        let client = crate::http::build_client();
        let start = std::time::Instant::now();
        let err = fetch_pages(&client, &wiki, &titles(&["Wood"])).await.unwrap_err();
        assert!(matches!(err, AppError::Http(_)), "got {err:?}");
        assert!(
            start.elapsed() < Duration::from_secs(14),
            "must fail via PARSE_TIMEOUT, not the mock's longer delay"
        );
    }

    #[tokio::test]
    async fn empty_render_is_skipped_not_sent_to_fallback() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        // Rendered fine but reduces to nothing — a skip, not a failure.
        mount_parse(
            &server,
            "Stub",
            ResponseTemplate::new(200).set_body_string(parse_ok("Stub", "")),
        )
        .await;
        mount_revisions(&server, ResponseTemplate::new(200).set_body_string("{}"), 0).await;

        let client = crate::http::build_client();
        let pages = fetch_pages(&client, &wiki, &titles(&["Stub"])).await.unwrap();
        assert!(pages.is_empty());
    }
}
