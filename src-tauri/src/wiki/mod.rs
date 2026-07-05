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

    /// Golden retrieval cases: real player questions and the page that must land
    /// in the top-N titles (hit@4 — exactly what the model gets to see). The
    /// runner mirrors `run_ask`'s search semantics: preprocess the question, then
    /// one simplify retry on zero hits. Run before/after any retrieval change to
    /// turn "did it get better?" into pass/fail.
    #[tokio::test]
    #[ignore = "hits live game wikis; run with `cargo test -- --ignored`"]
    async fn golden_queries_hit_expected_pages() {
        // (game_id, question, expected: any one of these titles in the top-4,
        //  strict: gate the suite vs. track as a known gap without failing)
        let cases: &[(&str, &str, &[&str], bool)] = &[
            ("stardew", "best crops for winter", &["Winter Seeds", "Powdermelon", "Seasons"], true),
            ("stardew", "what does Abigail like", &["Abigail"], true),
            // Known gap: AND semantics + no stemming rank gift-item pages above
            // the canonical Abigail page for singular "gift". Flip to strict when
            // query handling or extraction improves (see the retrieval plans).
            ("stardew", "what does Abigail like as a gift", &["Abigail", "Villagers", "Leek"], false),
            ("stardew", "wood", &["Wood"], true),
            ("corekeeper", "best food for early game", &["Cooking", "Foods"], true),
            ("corekeeper", "how do I get more health", &["Health", "Healing potency"], true),
        ];

        let client = reqwest::Client::builder()
            .user_agent(super::USER_AGENT)
            .build()
            .expect("build reqwest client");

        // Sequential on purpose — MediaWiki etiquette, same as production.
        let mut misses = Vec::new();
        for (game_id, question, expected, strict) in cases {
            let wiki = games::find_game(game_id).expect("game is registered");

            let query = search::preprocess_query(question);
            let mut titles = search::search(&client, wiki, &query, search::DEFAULT_SEARCH_LIMIT)
                .await
                .expect("live search should succeed");
            if titles.is_empty() {
                let simplified = search::simplify_query(question);
                if !simplified.is_empty() && simplified != query {
                    titles =
                        search::search(&client, wiki, &simplified, search::DEFAULT_SEARCH_LIMIT)
                            .await
                            .expect("live retry search should succeed");
                }
            }

            let hit = expected.iter().any(|e| titles.iter().any(|t| t == e));
            let verdict = match (hit, strict) {
                (true, _) => "OK",
                (false, true) => "MISS",
                (false, false) => "KNOWN-GAP",
            };
            println!(
                "[golden] {game_id}: {question:?} -> {titles:?} (want any of {expected:?}) {verdict}"
            );
            if !hit && *strict {
                misses.push(format!("{game_id}: {question:?} got {titles:?}, wanted any of {expected:?}"));
            }
        }

        assert!(misses.is_empty(), "golden-query misses:\n{}", misses.join("\n"));
    }

    /// The zero-hit path must be a graceful empty list, not an error: an API
    /// change that turns "no results" into a missing `query.search` key would
    /// show the player a raw parse error instead of the canned answer.
    #[tokio::test]
    #[ignore = "hits the live Stardew Valley wiki; run with `cargo test -- --ignored`"]
    async fn nonsense_query_returns_empty_not_error() {
        let client = reqwest::Client::builder()
            .user_agent(super::USER_AGENT)
            .build()
            .expect("build reqwest client");

        let wiki = games::find_game("stardew").expect("stardew is registered");
        let titles = search::search(
            &client,
            wiki,
            "zzxqv wibblewomp plumbus",
            search::DEFAULT_SEARCH_LIMIT,
        )
        .await
        .expect("a no-match search must not be an error");
        assert!(titles.is_empty(), "nonsense query unexpectedly matched: {titles:?}");
    }
}
