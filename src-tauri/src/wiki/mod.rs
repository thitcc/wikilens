//! Wiki layer: a static registry of supported game wikis plus MediaWiki
//! search + plaintext-extract clients. No caching yet (see roadmap).

pub mod fetch;
pub mod games;
pub mod search;
pub mod wikitext;

/// Sent on every wiki request. Fandom/MediaWiki etiquette asks for a
/// descriptive, contactable User-Agent — keep it set on the shared client.
pub const USER_AGENT: &str = "wikilens/0.1 (game overlay; contact: none)";

#[cfg(test)]
mod live {
    //! End-to-end integration test: a real HTTP round-trip against a live game
    //! wiki (`search` -> `fetch_pages` -> plaintext). This is the tier the offline
    //! `parse_*` unit tests can't cover — it would catch a wiki changing its API
    //! shape or a wrong query param.
    //!
    //! `#[ignore]`d so `cargo test` stays offline and fast. Run it deliberately:
    //!   cargo test -p wikilens --lib -- --ignored
    //! (or just `cargo test -- --ignored` from `src-tauri/`).
    use super::{fetch, games, search};

    #[tokio::test]
    #[ignore = "hits the live Stardew Valley wiki; run with `cargo test -- --ignored`"]
    async fn stardew_search_then_fetch_returns_clean_pages() {
        let client = reqwest::Client::builder()
            .user_agent(super::USER_AGENT)
            .build()
            .expect("build reqwest client");

        let wiki = games::find_game("stardew").expect("stardew is registered");

        // search -> ranked titles
        let titles = search::search(&client, wiki, "Abigail", search::DEFAULT_SEARCH_LIMIT)
            .await
            .expect("live search should succeed");
        assert!(!titles.is_empty(), "search returned no titles");

        // fetch -> cleaned plaintext pages
        let pages = fetch::fetch_pages(&client, wiki, &titles)
            .await
            .expect("live fetch should succeed");
        assert!(!pages.is_empty(), "fetch returned no pages");

        for page in &pages {
            assert!(!page.text.trim().is_empty(), "page {:?} had empty text", page.title);
            assert!(!page.text.contains("{{"), "page {:?} still has template markup", page.title);
            assert!(!page.text.contains("[["), "page {:?} still has wikilink markup", page.title);
            assert!(
                page.url.starts_with(wiki.page_url),
                "page {:?} url {:?} is not under {:?}",
                page.title,
                page.url,
                wiki.page_url
            );
        }

        // Relevance sanity: searching "Abigail" should surface Abigail somewhere.
        let corpus = pages.iter().map(|p| p.text.to_lowercase()).collect::<String>();
        assert!(corpus.contains("abigail"), "no fetched page mentioned Abigail");
    }
}
