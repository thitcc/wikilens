//! MediaWiki `list=search` client.

use crate::error::AppError;
use crate::wiki::games::GameWiki;

/// Default number of pages to pull for a question. Kept small to bound tokens.
pub const DEFAULT_SEARCH_LIMIT: u32 = 4;

/// Conversational filler dropped before searching. Default-engine (MySQL
/// fulltext) wikis require every remaining term to literally appear on a page,
/// so filler words directly cost recall.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "any", "are", "as", "at", "be", "best", "can", "could",
    "did", "do", "does", "for", "from", "get", "has", "have", "how", "i", "in",
    "is", "it", "its", "like", "make", "me", "my", "of", "on", "or", "should",
    "some", "that", "the", "this", "to", "was", "way", "what", "when", "where",
    "which", "who", "why", "will", "with", "would", "you", "your",
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
    let limit = limit.to_string();
    let params = build_search_params(query, &limit, wiki.search_namespace);
    let body = client
        .get(wiki.api_url)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_search_response(&body)
}

/// Build the `list=search` query params. Split out so the `srwhat` and
/// `srnamespace` rules can be unit-tested without a network.
fn build_search_params<'a>(
    query: &'a str,
    limit: &'a str,
    search_namespace: Option<&'static str>,
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
    fn preprocess_drops_stopwords_and_punctuation() {
        assert_eq!(preprocess_query("How do I make Abigail like me?"), "Abigail");
        assert_eq!(preprocess_query("best crops for winter"), "crops winter");
        assert_eq!(
            preprocess_query("what does Abigail like as a gift?"),
            "Abigail gift"
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
