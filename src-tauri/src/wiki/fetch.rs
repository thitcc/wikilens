//! MediaWiki `prop=extracts` client: fetch clean plaintext for a set of titles.

use serde::Serialize;

use crate::error::AppError;
use crate::wiki::games::GameWiki;

/// Per-page character cap. Bounds LLM token cost; overflow is marked truncated.
pub const MAX_PAGE_CHARS: usize = 8_000;

/// A single wiki page reduced to plaintext, with a human-readable URL.
#[derive(Debug, Clone, Serialize)]
pub struct WikiPage {
    pub title: String,
    pub text: String,
    pub url: String,
}

/// Fetch plaintext extracts for the given titles in one API call.
///
/// MediaWiki etiquette: this issues a *single* batched request for all titles
/// rather than one request per page — do not parallelize this.
pub async fn fetch_pages(
    client: &reqwest::Client,
    wiki: &GameWiki,
    titles: &[String],
) -> Result<Vec<WikiPage>, AppError> {
    if titles.is_empty() {
        return Ok(Vec::new());
    }

    let joined = titles.join("|");
    let body = client
        .get(wiki.api_url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts"),
            ("explaintext", "1"),
            ("exsectionformat", "plain"),
            ("redirects", "1"),
            ("titles", joined.as_str()),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_pages_response(&body, wiki, titles)
}

/// Parse a `prop=extracts` JSON body into `WikiPage`s. Split out for unit testing.
///
/// `query.pages` comes back keyed by page id in no useful order, so results are
/// re-sorted to follow `titles` (the search-relevance ranking). Any page whose
/// title isn't in `titles` — e.g. one reached through a redirect — is appended
/// after the ranked ones.
pub fn parse_pages_response(
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
        // Titles the wiki couldn't resolve are marked `missing`; skip them.
        if page.get("missing").is_some() {
            continue;
        }
        let Some(title) = page.get("title").and_then(|t| t.as_str()) else {
            continue;
        };
        let text = page
            .get("extract")
            .and_then(|e| e.as_str())
            .unwrap_or_default();
        // Pages with an empty extract carry no signal for the LLM; skip them.
        if text.is_empty() {
            continue;
        }
        out.push(WikiPage {
            title: title.to_string(),
            text: truncate_text(text),
            url: build_page_url(wiki, title),
        });
    }

    // Restore search-relevance order (titles not in the ranking sort to the end).
    out.sort_by_key(|page| {
        titles
            .iter()
            .position(|t| t == &page.title)
            .unwrap_or(usize::MAX)
    });

    Ok(out)
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

    const STARDEW: GameWiki = GameWiki {
        id: "stardew",
        name: "Stardew Valley",
        api_url: "https://stardewvalleywiki.com/mediawiki/api.php",
        page_url: "https://stardewvalleywiki.com/",
    };
    const CORE_KEEPER: GameWiki = GameWiki {
        id: "corekeeper",
        name: "Core Keeper",
        api_url: "https://core-keeper.fandom.com/api.php",
        page_url: "https://core-keeper.fandom.com/wiki/",
    };

    #[test]
    fn url_replaces_spaces_with_underscores() {
        assert_eq!(
            build_page_url(&STARDEW, "Prismatic Shard"),
            "https://stardewvalleywiki.com/Prismatic_Shard"
        );
        assert_eq!(
            build_page_url(&CORE_KEEPER, "Copper Ore"),
            "https://core-keeper.fandom.com/wiki/Copper_Ore"
        );
    }

    #[test]
    fn url_keeps_namespace_colon_and_parens() {
        assert_eq!(
            build_page_url(&STARDEW, "Category:Crops"),
            "https://stardewvalleywiki.com/Category:Crops"
        );
        assert_eq!(
            build_page_url(&STARDEW, "Bream (Fish)"),
            "https://stardewvalleywiki.com/Bream_(Fish)"
        );
    }

    #[test]
    fn url_percent_encodes_unsafe_chars() {
        // Ampersand and question mark must be encoded to stay in the path.
        assert_eq!(
            build_page_url(&STARDEW, "Bob & Alice?"),
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
    fn parses_pages_and_skips_missing_and_empty() {
        let sample = r#"{
            "query": {
                "pages": {
                    "1": { "pageid": 1, "title": "Winter", "extract": "Winter is a season." },
                    "2": { "pageid": 2, "title": "Nonexistent Page", "missing": "" },
                    "3": { "pageid": 3, "title": "Blank", "extract": "" }
                }
            }
        }"#;
        let titles = vec!["Winter".to_string(), "Nonexistent Page".to_string(), "Blank".to_string()];
        let pages = parse_pages_response(sample, &STARDEW, &titles).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "Winter");
        assert_eq!(pages[0].text, "Winter is a season.");
        assert_eq!(pages[0].url, "https://stardewvalleywiki.com/Winter");
    }

    #[test]
    fn pages_follow_search_relevance_order_not_pageid() {
        // pageids sort lexically as "1","10","2" -> Copper, Iron, Tin; the
        // search ranking is Iron, Copper, Tin — the output must follow ranking.
        let sample = r#"{
            "query": {
                "pages": {
                    "1": { "pageid": 1, "title": "Copper Ore", "extract": "copper" },
                    "10": { "pageid": 10, "title": "Iron Ore", "extract": "iron" },
                    "2": { "pageid": 2, "title": "Tin Ore", "extract": "tin" }
                }
            }
        }"#;
        let titles = vec!["Iron Ore".to_string(), "Copper Ore".to_string(), "Tin Ore".to_string()];
        let pages = parse_pages_response(sample, &STARDEW, &titles).unwrap();
        let ordered: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(ordered, vec!["Iron Ore", "Copper Ore", "Tin Ore"]);
    }

    #[test]
    fn missing_pages_key_is_parse_error() {
        assert!(matches!(
            parse_pages_response(r#"{ "query": {} }"#, &STARDEW, &[]),
            Err(AppError::Parse(_))
        ));
    }
}
