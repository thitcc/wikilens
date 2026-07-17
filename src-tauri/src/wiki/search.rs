//! MediaWiki `list=search` client.

use std::time::Duration;

use crate::error::AppError;
use crate::http;
use crate::wiki::games::GameWiki;

/// Default number of pages to pull for a question. Kept small to bound tokens.
pub const DEFAULT_SEARCH_LIMIT: u32 = 4;

/// Per-request cap, matching the 8s probe/model-list precedent. This is the
/// hottest request in the app — the recovery ladder can run several searches
/// per ask, so each must stay short (previously untimed: one stalled search
/// hung the whole ask).
const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Conversational filler dropped before searching. Default-engine (MySQL
/// fulltext) wikis require every remaining term to literally appear on a page,
/// so filler words directly cost recall — and AND-semantics generally means an
/// intent word the page never uses ("strategy") excludes the canonical entity
/// page outright. Every entry is global across all wikis: vet each addition
/// against covered wikis' page titles first ("farming" and "guide" are real
/// pages — Stardew's Farming skill, Terraria's Guide NPC — and "acquire"
/// redirects on the GW2 wiki, so they must stay searchable).
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "any", "are", "as", "at", "be", "best", "can", "could",
    "did", "do", "does", "for", "from", "get", "has", "have", "how", "i", "in",
    "is", "it", "its", "like", "make", "me", "my", "obtain", "of", "on", "or",
    "should", "some", "strategies", "strategy", "that", "the", "this", "to",
    "was", "way", "what", "when", "where", "which", "who", "why", "will",
    "with", "would", "you", "your",
];

/// Reduce a natural-language question to search keywords: trim punctuation off
/// each token and drop stopwords (case-insensitive; original casing kept).
/// Falls back to the trimmed original question if nothing survives — an empty
/// `srsearch` is never sent.
pub fn preprocess_query(question: &str) -> String {
    let kept: Vec<&str> = question
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|token| !token.is_empty())
        .filter(|token| !STOPWORDS.contains(&token.to_lowercase().as_str()))
        .collect();
    if kept.is_empty() {
        question.trim().to_string()
    } else {
        kept.join(" ")
    }
}

/// Harsher pass for the zero-hit retry: keep only tokens that look like content
/// words — 4+ characters, or Capitalized (entity names like "Leo"). May return
/// an empty string; the caller skips the retry then.
pub fn simplify_query(question: &str) -> String {
    preprocess_query(question)
        .split_whitespace()
        .filter(|token| {
            token.chars().count() >= 4
                || token.chars().next().is_some_and(|c| c.is_uppercase())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Search a game's wiki, returning matching page titles ranked as the wiki
/// ranks them (relevance). Network errors and malformed responses become `AppError`.
pub async fn search(
    client: &reqwest::Client,
    wiki: &GameWiki,
    query: &str,
    limit: u32,
) -> Result<Vec<String>, AppError> {
    Ok(search_full(client, wiki, query, limit).await?.0)
}

/// Like [`search`], but also returns MediaWiki's "did you mean" suggestion when
/// the wiki offers one. The single round-trip already carries it — `srinfo`
/// defaults to `totalhits|suggestion`, so CirrusSearch wikis (Fandom, wiki.gg,
/// minecraft.wiki) include `query.searchinfo.suggestion` on a misspelled query;
/// default-engine wikis (Stardew) simply omit it. We just stop discarding it, to
/// feed the zero-hit retry in `run_ask`.
pub async fn search_full(
    client: &reqwest::Client,
    wiki: &GameWiki,
    query: &str,
    limit: u32,
) -> Result<(Vec<String>, Option<String>), AppError> {
    let limit = limit.to_string();
    let params = build_search_params(query, &limit, wiki.search_namespace.as_deref());
    let resp = client
        .get(&wiki.api_url)
        .query(&params)
        .timeout(SEARCH_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES).await?;

    Ok((parse_search_response(&body)?, parse_search_suggestion(&body)))
}

/// Build the `list=search` query params. Split out so the `srwhat` and
/// `srnamespace` rules can be unit-tested without a network.
fn build_search_params<'a>(
    query: &'a str,
    limit: &'a str,
    search_namespace: Option<&'a str>,
) -> Vec<(&'static str, &'a str)> {
    let mut params = vec![
        ("action", "query"),
        ("list", "search"),
        ("srsearch", query),
        ("srlimit", limit),
        ("format", "json"),
    ];
    // Fulltext search rescues multi-word questions on default-engine wikis
    // (e.g. stardewvalleywiki.com), which otherwise title-match them to 0 hits;
    // Elasticsearch-backed wikis (Fandom) already default to text search. But
    // fulltext also demotes exact-title pages on single-word item queries
    // ("wood" ranks Wood Chipper above Wood), and single words title-match fine
    // everywhere — so opt in for multi-word queries only.
    if query.split_whitespace().count() > 1 {
        params.push(("srwhat", "text"));
    }
    // Shared wikis (UESP) keep each game's pages in their own namespace; the
    // default namespaces there return zero or cross-game hits.
    if let Some(ns) = search_namespace {
        params.push(("srnamespace", ns));
    }
    params
}

/// Extract page titles from a `list=search` JSON body. Split out so it can be
/// unit-tested against fixed sample JSON with no network.
pub fn parse_search_response(body: &str) -> Result<Vec<String>, AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;

    let results = json
        .get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
        .ok_or_else(|| AppError::Parse("missing `query.search` array in response".into()))?;

    let titles = results
        .iter()
        .filter_map(|item| item.get("title").and_then(|t| t.as_str()))
        .map(|s| s.to_string())
        .collect();

    Ok(titles)
}

/// Extract MediaWiki's "did you mean" spelling suggestion
/// (`query.searchinfo.suggestion`) if present, trimmed and non-empty. Unlike
/// [`parse_search_response`] this never errors — the suggestion is an optional
/// hint, so a missing field or a malformed body is just `None`.
pub fn parse_search_suggestion(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let suggestion = json
        .get("query")?
        .get("searchinfo")?
        .get("suggestion")?
        .as_str()?
        .trim();
    (!suggestion.is_empty()).then(|| suggestion.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_word_query_omits_srwhat_and_srnamespace() {
        let params = build_search_params("wood", "4", None);
        assert!(!params.iter().any(|(k, _)| *k == "srwhat"));
        assert!(!params.iter().any(|(k, _)| *k == "srnamespace"));
        assert!(params.contains(&("srsearch", "wood")));
        assert!(params.contains(&("srlimit", "4")));
    }

    #[test]
    fn multi_word_query_opts_into_fulltext() {
        let params = build_search_params("crops winter", "4", None);
        assert!(params.contains(&("srwhat", "text")));
    }

    #[test]
    fn namespace_filter_is_sent_only_when_set() {
        let params = build_search_params("Whiterun", "4", Some("134"));
        assert!(params.contains(&("srnamespace", "134")));
        // Single word: the namespace filter must not drag srwhat in with it.
        assert!(!params.iter().any(|(k, _)| *k == "srwhat"));
    }

    #[test]
    fn parses_titles_in_order() {
        let sample = r#"{
            "batchcomplete": "",
            "query": {
                "searchinfo": { "totalhits": 42 },
                "search": [
                    { "ns": 0, "title": "Winter", "pageid": 1, "snippet": "..." },
                    { "ns": 0, "title": "Crops", "pageid": 2, "snippet": "..." }
                ]
            }
        }"#;
        assert_eq!(
            parse_search_response(sample).unwrap(),
            vec!["Winter".to_string(), "Crops".to_string()]
        );
    }

    #[test]
    fn empty_search_yields_empty_vec() {
        let sample = r#"{ "query": { "search": [] } }"#;
        assert!(parse_search_response(sample).unwrap().is_empty());
    }

    #[test]
    fn missing_search_key_is_parse_error() {
        let sample = r#"{ "query": {} }"#;
        assert!(matches!(
            parse_search_response(sample),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn invalid_json_is_parse_error() {
        assert!(matches!(
            parse_search_response("not json"),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn parses_the_search_suggestion_when_present() {
        // Shape of a CirrusSearch zero-hit response with a spelling suggestion.
        let sample = r#"{
            "batchcomplete": "",
            "query": {
                "searchinfo": {
                    "totalhits": 0,
                    "suggestion": "arcane persistence",
                    "suggestionsnippet": "arcane persistence"
                },
                "search": []
            }
        }"#;
        assert_eq!(
            parse_search_suggestion(sample).as_deref(),
            Some("arcane persistence")
        );
    }

    #[test]
    fn suggestion_is_none_when_absent_blank_or_unparseable() {
        // No searchinfo at all (a clean hit, or a default-engine wiki like Stardew).
        assert_eq!(parse_search_suggestion(r#"{ "query": { "search": [] } }"#), None);
        // searchinfo present but carries no suggestion field.
        assert_eq!(
            parse_search_suggestion(r#"{ "query": { "searchinfo": { "totalhits": 5 } } }"#),
            None
        );
        // A blank/whitespace suggestion is treated as no suggestion.
        assert_eq!(
            parse_search_suggestion(r#"{ "query": { "searchinfo": { "suggestion": "   " } } }"#),
            None
        );
        // Not JSON at all.
        assert_eq!(parse_search_suggestion("not json"), None);
    }

    #[test]
    fn preprocess_drops_stopwords_and_punctuation() {
        assert_eq!(preprocess_query("How do I make Abigail like me?"), "Abigail");
        assert_eq!(preprocess_query("best crops for winter"), "crops winter");
        assert_eq!(
            preprocess_query("what does Abigail like as a gift?"),
            "Abigail gift"
        );
    }

    /// The archon-shards live failure: intent words ("strategy") survive on
    /// AND-semantics wikis and exclude the canonical page — the bare entity
    /// must be what's left (`vault/2026-07-15_rewrite-bare-entity-candidate.md`).
    #[test]
    fn preprocess_drops_intent_words() {
        assert_eq!(
            preprocess_query("best strategy to get archon shards"),
            "archon shards"
        );
        assert_eq!(
            preprocess_query("how do I obtain a fusion core?"),
            "fusion core"
        );
        assert_eq!(
            preprocess_query("strategies for the Eidolon fight"),
            "Eidolon fight"
        );
    }

    #[test]
    fn preprocess_keeps_original_casing() {
        assert_eq!(preprocess_query("Where is Krobus?"), "Krobus");
    }

    #[test]
    fn preprocess_falls_back_when_everything_is_filler() {
        assert_eq!(preprocess_query("  how do you do  "), "how do you do");
        assert_eq!(preprocess_query(""), "");
    }

    #[test]
    fn simplify_keeps_long_and_capitalized_tokens() {
        assert_eq!(simplify_query("what does Leo like?"), "Leo");
        assert_eq!(simplify_query("how to get an iron bar id"), "iron");
    }

    #[test]
    fn simplify_can_return_empty() {
        assert_eq!(simplify_query("how to get it"), "");
    }
}

/// Offline wiremock tier: the hardening pins on the production search path —
/// redirect-following through the factory client, the response byte cap, and
/// (in the `--ignored` tier, because it takes the full 8s) the request
/// timeout (`vault/2026-07-13_wiki-fetch-hardening.md`).
#[cfg(test)]
mod http_tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::mock_wiki;

    const SEARCH_BODY: &str =
        r#"{"query":{"search":[{"title":"Wood"},{"title":"Wood Chipper"}]}}"#;

    /// The legit apex→www case: a wiki that 302s its api.php must keep
    /// working end-to-end under the factory client's redirect policy.
    #[tokio::test]
    async fn search_full_follows_a_wiki_redirect() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", format!("{}/w/api.php", server.uri()).as_str()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_BODY))
            .expect(1)
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let (titles, _) = search_full(&client, &wiki, "wood", 4).await.unwrap();
        assert_eq!(titles, vec!["Wood", "Wood Chipper"]);
    }

    /// The call site must pass the production cap, not just have one available.
    #[tokio::test]
    async fn search_full_refuses_an_oversized_body() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        // Over MAX_RESPONSE_BYTES of valid-JSON padding.
        let body = format!(
            r#"{{"query":{{"search":[]}},"pad":"{}"}}"#,
            "a".repeat(9 * 1024 * 1024)
        );
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let err = search_full(&client, &wiki, "wood", 4).await.unwrap_err();
        assert!(matches!(err, AppError::BodyTooLarge(_)), "got {err:?}");
    }

    /// A WAF-blocked wiki (403 on api.php, like wiki.guildwars2.com) must
    /// surface the friendly refusal copy through the search path.
    #[tokio::test]
    async fn search_full_403_surfaces_friendly_forbidden_message() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let err = search_full(&client, &wiki, "wood", 4).await.unwrap_err();
        assert!(matches!(err, AppError::Http(_)), "got {err:?}");
        assert!(
            err.to_string().contains("blocks automated access"),
            "search must surface the friendly 403 copy: {err}"
        );
    }

    #[tokio::test]
    #[ignore = "slow (~8s): pins the SEARCH_TIMEOUT behavior"]
    async fn search_hang_times_out() {
        let server = MockServer::start().await;
        let wiki = mock_wiki(&server.uri());
        Mock::given(method("GET"))
            .and(path("/api.php"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(SEARCH_BODY)
                    .set_delay(SEARCH_TIMEOUT + Duration::from_secs(2)),
            )
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let start = std::time::Instant::now();
        let err = search_full(&client, &wiki, "wood", 4).await.unwrap_err();
        assert!(matches!(err, AppError::Http(_)), "got {err:?}");
        assert!(
            start.elapsed() < SEARCH_TIMEOUT + Duration::from_secs(1),
            "must fail via SEARCH_TIMEOUT, not the mock's longer delay"
        );
    }
}
