//! Local per-game title index for deterministic typo → page resolution.
//!
//! On a zero-hit search the LLM rewrite (`llm::rewrite_query`) is the probabilistic
//! option; this is the deterministic one. Fetch the wiki's real page titles once
//! (`list=allpages`), then fuzzy-match the query against them locally with
//! Jaro-Winkler. A strong match means the page exists under a name the search
//! engine's own spelling logic missed ("arcanr persitance" → "Arcane Persistence")
//! — so we can fetch it directly. Deterministic and offline: it can't hallucinate,
//! and unlike the model it knows titles added in yesterday's patch.
//!
//! The title list is memoised per game for the app session (see
//! `AppState::title_cache`). Cross-restart persistence is the SQLite-cache roadmap
//! item; this in-memory cache pays the one-off `allpages` walk once per session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::AppError;
use crate::http;
use crate::wiki::games::GameWiki;

/// Cap on titles pulled per game — bounds the one-off fetch and memory. Large
/// enough to cover a wiki's entity/item pages, small enough to stay cheap.
const MAX_TITLES: usize = 6_000;
/// Hard cap on requests in the `allpages` walk, independent of title count — a
/// wiki that returns few titles per page can't drag us into a long request chain.
/// At `PAGE_LIMIT` = 500 the title cap is reached in ~12 requests; 20 is headroom.
const MAX_REQUESTS: usize = 20;
/// `aplimit` per request (MediaWiki caps anonymous callers at 500).
const PAGE_LIMIT: usize = 500;
/// Per-request timeout for the `allpages` walk (matches the model-list precedent).
const ALLPAGES_TIMEOUT: Duration = Duration::from_secs(8);
/// Overall wall-clock budget for the whole `allpages` walk, so a slow wiki can't
/// turn the first zero-hit query for a game into a long stall before the later
/// recovery stages run.
const ALLPAGES_BUDGET: Duration = Duration::from_secs(15);
/// Minimum Jaro-Winkler similarity to accept a title as a query's match. Strict,
/// so only a real typo/near-miss resolves — not a loosely-related page.
const MATCH_THRESHOLD: f64 = 0.92;
/// Reject a match when the shorter of (query, title) is below this fraction of the
/// longer — stops a short query claiming a much longer prefix-sharing title on the
/// Winkler prefix boost alone ("iron" → "Iron Ore"). Typos preserve length, so a
/// real correction is always close in length.
const LENGTH_RATIO_FLOOR: f64 = 0.6;

/// A game's page titles, ready for fuzzy lookup. Cheap to share via `Arc`.
#[derive(Debug, Clone)]
pub struct TitleIndex {
    titles: Vec<String>,
    /// Whether to drop a leading `Namespace:` before scoring. True only for wikis
    /// that keep their pages in a namespace (UESP) — on a plain namespace-0 wiki a
    /// colon is part of the title ("Update 30: The New War"), so stripping it would
    /// mis-score the title.
    strip_ns: bool,
}

impl TitleIndex {
    fn new(titles: Vec<String>, strip_ns: bool) -> Self {
        Self { titles, strip_ns }
    }

    /// The best title whose similarity to `query` clears [`MATCH_THRESHOLD`] (and
    /// the [`LENGTH_RATIO_FLOOR`] guard), if any. Compares the normalized query
    /// against each normalized title — dropping a `Namespace:` prefix only on
    /// namespaced wikis, so UESP's "Skyrim:Whiterun" matches "whiterun". Returns
    /// the original-cased title.
    pub fn best_match(&self, query: &str) -> Option<String> {
        let q = normalize(query);
        if q.is_empty() {
            return None;
        }
        let qlen = q.chars().count();
        let mut best: Option<(f64, &String)> = None;
        for title in &self.titles {
            let stripped = if self.strip_ns {
                strip_namespace(title)
            } else {
                title.as_str()
            };
            let cand = normalize(stripped);
            let clen = cand.chars().count();
            if clen == 0 {
                continue;
            }
            // Length-ratio guard: a real typo correction is close in length, so a
            // short query can't claim a much longer title on the prefix boost.
            let (short, long) = if qlen <= clen { (qlen, clen) } else { (clen, qlen) };
            if (short as f64) / (long as f64) < LENGTH_RATIO_FLOOR {
                continue;
            }
            let score = jaro_winkler(&q, &cand);
            if score >= MATCH_THRESHOLD && best.is_none_or(|(b, _)| score > b) {
                best = Some((score, title));
            }
        }
        best.map(|(_, t)| t.clone())
    }
}

/// Get a game's title index from the session cache, fetching + caching it on first
/// use. `None` if the fetch fails — the caller then simply skips this recovery
/// stage. Never holds the lock across the `await` (same discipline as
/// `AppState::models_cache`).
pub async fn get_or_fetch(
    cache: &Mutex<HashMap<String, Arc<TitleIndex>>>,
    client: &reqwest::Client,
    wiki: &GameWiki,
) -> Option<Arc<TitleIndex>> {
    if let Some(index) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&wiki.id)
        .cloned()
    {
        return Some(index);
    }
    // Fetch outside the lock; a std Mutex must never be held across an await.
    let titles = fetch_all_titles(client, wiki).await.ok()?;
    let index = Arc::new(TitleIndex::new(titles, wiki.search_namespace.is_some()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    // Another ask may have inserted while we fetched — keep whichever landed first.
    Some(guard.entry(wiki.id.clone()).or_insert(index).clone())
}

/// Fetch up to [`MAX_TITLES`] page titles for a game via `list=allpages`,
/// following `apcontinue`. Restricted to the game's `search_namespace` when set
/// (UESP), else namespace 0 (articles). Sequential and bounded — MediaWiki-polite.
async fn fetch_all_titles(
    client: &reqwest::Client,
    wiki: &GameWiki,
) -> Result<Vec<String>, AppError> {
    // `apnamespace` takes a single namespace; `search_namespace` is documented as
    // pipe-separated (for `list=search`), so use its first segment. Today only
    // Skyrim sets it, to the single value "134".
    let namespace = wiki
        .search_namespace
        .as_deref()
        .unwrap_or("0")
        .split('|')
        .next()
        .unwrap_or("0")
        .to_string();
    let limit = PAGE_LIMIT.to_string();
    let mut titles: Vec<String> = Vec::new();
    let mut apcontinue: Option<String> = None;

    let start = std::time::Instant::now();
    for _ in 0..MAX_REQUESTS {
        if titles.len() >= MAX_TITLES || start.elapsed() >= ALLPAGES_BUDGET {
            break;
        }
        let mut params: Vec<(&str, String)> = vec![
            ("action", "query".to_string()),
            ("list", "allpages".to_string()),
            ("apnamespace", namespace.clone()),
            ("aplimit", limit.clone()),
            ("format", "json".to_string()),
        ];
        if let Some(cont) = &apcontinue {
            params.push(("apcontinue", cont.clone()));
        }
        let resp = client
            .get(&wiki.api_url)
            .query(&params)
            .timeout(ALLPAGES_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES).await?;
        let (mut page, next) = parse_allpages(&body)?;
        titles.append(&mut page);
        match next {
            Some(cont) => apcontinue = Some(cont),
            None => break,
        }
    }
    titles.truncate(MAX_TITLES);
    Ok(titles)
}

/// Parse a `list=allpages` body into `(titles, apcontinue)`. Split out for offline
/// unit tests. A missing `query.allpages` array is a parse error.
fn parse_allpages(body: &str) -> Result<(Vec<String>, Option<String>), AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;
    let pages = json
        .get("query")
        .and_then(|q| q.get("allpages"))
        .and_then(|a| a.as_array())
        .ok_or_else(|| AppError::Parse("missing `query.allpages` array".into()))?;
    let titles = pages
        .iter()
        .filter_map(|p| p.get("title").and_then(|t| t.as_str()))
        .map(str::to_string)
        .collect();
    let apcontinue = json
        .get("continue")
        .and_then(|c| c.get("apcontinue"))
        .and_then(|c| c.as_str())
        .map(str::to_string);
    Ok((titles, apcontinue))
}

/// Lowercase and collapse everything non-alphanumeric to single spaces, so scoring
/// compares words, not punctuation or casing.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            prev_space = false;
        } else if !prev_space && !out.is_empty() {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

/// Drop a leading `Namespace:` prefix (UESP "Skyrim:Whiterun" → "Whiterun") so
/// scoring compares the human title. A colon with nothing after it is left alone.
fn strip_namespace(title: &str) -> &str {
    match title.split_once(':') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => title,
    }
}

/// Jaro-Winkler similarity in `[0.0, 1.0]` — good for short strings and typos
/// (transpositions, a few wrong letters). Standard algorithm, pure + unit-tested.
fn jaro_winkler(a: &str, b: &str) -> f64 {
    let j = jaro(a, b);
    if j == 0.0 {
        return 0.0;
    }
    // Winkler boost: reward a common prefix of up to 4 chars.
    let prefix = a
        .chars()
        .zip(b.chars())
        .take(4)
        .take_while(|(x, y)| x == y)
        .count();
    j + prefix as f64 * 0.1 * (1.0 - j)
}

/// Jaro similarity — the base [`jaro_winkler`] adjusts.
fn jaro(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (alen, blen) = (a.len(), b.len());
    if alen == 0 && blen == 0 {
        return 1.0;
    }
    if alen == 0 || blen == 0 {
        return 0.0;
    }
    let match_dist = (alen.max(blen) / 2).saturating_sub(1);
    let mut a_matched = vec![false; alen];
    let mut b_matched = vec![false; blen];
    let mut matches = 0usize;
    for i in 0..alen {
        let lo = i.saturating_sub(match_dist);
        let hi = (i + match_dist + 1).min(blen);
        for j in lo..hi {
            if !b_matched[j] && a[i] == b[j] {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..alen {
        if a_matched[i] {
            while !b_matched[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = matches as f64;
    let t = transpositions as f64 / 2.0;
    (m / alen as f64 + m / blen as f64 + (m - t) / m) / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_allpages_titles_and_continue() {
        let body = r#"{
            "continue": { "apcontinue": "Beach", "continue": "-||" },
            "query": { "allpages": [
                { "pageid": 1, "ns": 0, "title": "Abigail" },
                { "pageid": 2, "ns": 0, "title": "Amethyst" }
            ] }
        }"#;
        let (titles, cont) = parse_allpages(body).unwrap();
        assert_eq!(titles, vec!["Abigail".to_string(), "Amethyst".to_string()]);
        assert_eq!(cont.as_deref(), Some("Beach"));
    }

    #[test]
    fn allpages_without_continue_is_the_last_page() {
        let body = r#"{ "query": { "allpages": [ { "title": "Zenith" } ] } }"#;
        let (titles, cont) = parse_allpages(body).unwrap();
        assert_eq!(titles, vec!["Zenith".to_string()]);
        assert!(cont.is_none());
    }

    #[test]
    fn allpages_missing_array_is_a_parse_error() {
        assert!(matches!(
            parse_allpages(r#"{ "query": {} }"#),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn normalize_lowercases_and_collapses_punctuation() {
        assert_eq!(normalize("Arcane Persistence!"), "arcane persistence");
        assert_eq!(normalize("  Wood-Chipper  "), "wood chipper");
        assert_eq!(normalize("Skyrim:Whiterun"), "skyrim whiterun");
    }

    #[test]
    fn strip_namespace_drops_a_leading_prefix() {
        assert_eq!(strip_namespace("Skyrim:Whiterun"), "Whiterun");
        assert_eq!(strip_namespace("Whiterun"), "Whiterun");
        assert_eq!(strip_namespace("A:"), "A:");
    }

    #[test]
    fn jaro_winkler_matches_known_values() {
        let approx = |a: f64, b: f64| (a - b).abs() < 0.01;
        assert!(approx(jaro_winkler("martha", "marhta"), 0.961));
        assert_eq!(jaro_winkler("same", "same"), 1.0);
        assert_eq!(jaro_winkler("", ""), 1.0);
        assert_eq!(jaro_winkler("abc", ""), 0.0);
        assert!(jaro_winkler("cat", "dog") < 0.5);
    }

    #[test]
    fn best_match_resolves_a_typo_to_a_real_title() {
        let index = TitleIndex::new(
            vec![
                "Excalibur".to_string(),
                "Persistence".to_string(),
                "Mag".to_string(),
            ],
            false,
        );
        // One/two-character typos resolve to the real, original-cased title.
        assert_eq!(index.best_match("Excalibar").as_deref(), Some("Excalibur"));
        assert_eq!(index.best_match("persistance").as_deref(), Some("Persistence"));
        // A clearly-unrelated query resolves to nothing (the threshold guards it).
        assert_eq!(index.best_match("zzzzzzzz"), None);
        assert_eq!(index.best_match(""), None);
    }

    #[test]
    fn short_query_does_not_match_a_longer_prefix_title() {
        // The Winkler prefix boost must not let a short query claim a much longer
        // title it merely prefixes (guarded by threshold + length ratio).
        let index = TitleIndex::new(vec!["Magic".to_string(), "Iron Ore".to_string()], false);
        assert_eq!(index.best_match("mag"), None);
        assert_eq!(index.best_match("iron"), None);
    }

    #[test]
    fn namespace_stripping_is_gated_on_the_wiki_using_one() {
        // Namespaced wiki (UESP): strip "Skyrim:" so a bare query matches.
        let namespaced = TitleIndex::new(vec!["Skyrim:Whiterun".to_string()], true);
        assert_eq!(
            namespaced.best_match("whiterun").as_deref(),
            Some("Skyrim:Whiterun")
        );
        // Plain namespace-0 wiki: a colon is content, so the whole title is scored
        // and a bare query doesn't spuriously match on the tail.
        let plain = TitleIndex::new(vec!["Skyrim:Whiterun".to_string()], false);
        assert_eq!(plain.best_match("whiterun"), None);
    }
}
