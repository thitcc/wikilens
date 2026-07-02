//! MediaWiki `list=search` client.

use crate::error::AppError;
use crate::wiki::games::GameWiki;

/// Default number of pages to pull for a question. Kept small to bound tokens.
pub const DEFAULT_SEARCH_LIMIT: u32 = 4;

/// Search a game's wiki, returning matching page titles ranked as the wiki
/// ranks them (relevance). Network errors and malformed responses become `AppError`.
pub async fn search(
    client: &reqwest::Client,
    wiki: &GameWiki,
    query: &str,
    limit: u32,
) -> Result<Vec<String>, AppError> {
    let limit = limit.to_string();
    let body = client
        .get(wiki.api_url)
        .query(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", query),
            ("srlimit", limit.as_str()),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_search_response(&body)
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
}
