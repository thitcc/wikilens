//! Probe validation for user-added wikis.
//!
//! Given a base URL (or a bare game name, via `suggest`), find a working
//! MediaWiki `api.php` and derive the wiki's canonical endpoints from its own
//! siteinfo — `page_url` comes from the real `articlepath` (the `/w/$1` vs
//! `/wiki/$1` split is real: minecraft.wiki, wiki.warframe.com) and `api_url`
//! from `server + scriptpath`, never assumed from what the user typed. A
//! candidate must also answer one test search before it is offered/saved:
//! search is what the whole ask pipeline stands on.
//!
//! All requests are sequential (MediaWiki etiquette) with pure, offline-
//! testable parsing split out.

use std::time::Duration;

use serde::Serialize;

use crate::error::AppError;
use crate::http;
use crate::wiki::search;

/// Per-request cap, matching the model-list fetch precedent (`models.rs`).
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Well-known `api.php` locations, most common first. `/mediawiki/api.php`
/// is real-world too (stardewvalleywiki.com).
const CANDIDATE_API_PATHS: &[&str] = &["/api.php", "/w/api.php", "/mediawiki/api.php"];

/// A probe-verified wiki, endpoints already canonical.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiCandidate {
    /// The wiki's own sitename ("Terraria Wiki") — shown so the user can
    /// tell candidates apart.
    pub name: String,
    pub api_url: String,
    pub page_url: String,
}

/// The `query.general` siteinfo fields the derivation needs.
#[derive(Debug, PartialEq)]
pub struct SiteInfo {
    pub sitename: String,
    /// May be protocol-relative (`//wiki.guildwars2.com`).
    pub server: String,
    /// E.g. `/wiki/$1`, `/w/$1`, or just `/$1`.
    pub articlepath: String,
    /// E.g. `/w`, `/mediawiki`, or `""` for root installs (minecraft.wiki).
    pub scriptpath: String,
}

/// Normalize user input to a probe base: trims, defaults to `https://`,
/// accepts http(s) only, and drops any path/query. Returns the base origin
/// plus, when the pasted URL already points at an `api.php`, that exact URL —
/// probed first, which covers wikis under unusual prefixes.
pub fn normalize_base_url(input: &str) -> Option<(String, Option<String>)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = reqwest::Url::parse(&with_scheme).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    let mut base = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        base.push_str(&format!(":{port}"));
    }
    let exact_api = url
        .path()
        .ends_with("api.php")
        .then(|| format!("{base}{}", url.path()));
    Some((base, exact_api))
}

/// The `api.php` URLs to try for a base, in order: the user's exact api.php
/// URL when they pasted one, then the well-known paths (deduped).
fn candidate_api_urls(base: &str, exact_api: Option<String>) -> Vec<String> {
    let mut urls: Vec<String> = exact_api.into_iter().collect();
    for path in CANDIDATE_API_PATHS {
        let url = format!("{base}{path}");
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

/// Extract the siteinfo fields from an `action=query&meta=siteinfo` body.
/// A missing `scriptpath` is treated as `""` (root install); the rest are
/// required.
pub fn parse_siteinfo(body: &str) -> Result<SiteInfo, AppError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| AppError::Parse(e.to_string()))?;
    let general = json
        .get("query")
        .and_then(|q| q.get("general"))
        .ok_or_else(|| AppError::Parse("missing `query.general` in siteinfo response".into()))?;
    let field = |key: &str| {
        general
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| AppError::Parse(format!("missing `{key}` in siteinfo response")))
    };
    Ok(SiteInfo {
        sitename: field("sitename")?,
        server: field("server")?,
        articlepath: field("articlepath")?,
        scriptpath: field("scriptpath").unwrap_or_default(),
    })
}

/// Derive the canonical endpoints from siteinfo. `scheme` fixes up a
/// protocol-relative `server`.
pub fn derive_endpoints(info: &SiteInfo, scheme: &str) -> (String, String) {
    let server = match info.server.strip_prefix("//") {
        Some(rest) => format!("{scheme}://{rest}"),
        None => info.server.clone(),
    };
    let api_url = format!("{server}{}/api.php", info.scriptpath);
    // "/wiki/$1" -> "https://host/wiki/"; build_page_url appends the title.
    let page_url = format!("{server}{}", info.articlepath.replace("$1", ""));
    (api_url, page_url)
}

/// Probe one base URL/pasted address. The first candidate `api.php` whose
/// siteinfo parses wins; its derived endpoints are then proven with one test
/// search (against the *derived* api_url — which also validates the
/// derivation) before the candidate is returned.
pub async fn probe_base(
    client: &reqwest::Client,
    input: &str,
) -> Result<WikiCandidate, AppError> {
    let (base, exact_api) = normalize_base_url(input).ok_or_else(|| {
        AppError::Probe(format!(
            "\"{}\" doesn't look like a web address.",
            input.trim()
        ))
    })?;
    let scheme = if base.starts_with("http://") { "http" } else { "https" };

    for api_url in candidate_api_urls(&base, exact_api) {
        let Some(info) = fetch_siteinfo(client, &api_url).await else {
            continue;
        };
        let (canonical_api, page_url) = derive_endpoints(&info, scheme);
        // siteinfo answered, so this IS the wiki — a search failure here is a
        // real verdict, not a reason to try other paths.
        validate_search(client, &canonical_api).await?;
        return Ok(WikiCandidate {
            name: info.sitename,
            api_url: canonical_api,
            page_url,
        });
    }

    Err(AppError::Probe(format!(
        "Couldn't find a MediaWiki API at {base} — check the address, or paste the wiki's api.php URL."
    )))
}

/// Probe the common wiki hosts for a game name; returns every verified wiki.
/// Two slug shapes are tried (Fandom hyphenates: dave-the-diver.fandom.com)
/// across wiki.gg and Fandom — both platforms serve `api.php` at the root.
/// Sequential; dead hosts fail fast, and redirect aliases collapse on the
/// canonical api_url.
pub async fn suggest(client: &reqwest::Client, name: &str) -> Vec<WikiCandidate> {
    let mut found: Vec<WikiCandidate> = Vec::new();
    for domain in candidate_domains(name) {
        let api_url = format!("https://{domain}/api.php");
        let Some(info) = fetch_siteinfo(client, &api_url).await else {
            continue;
        };
        let (canonical_api, page_url) = derive_endpoints(&info, "https");
        if found.iter().any(|c| c.api_url == canonical_api) {
            continue;
        }
        if validate_search(client, &canonical_api).await.is_err() {
            continue;
        }
        found.push(WikiCandidate {
            name: info.sitename,
            api_url: canonical_api,
            page_url,
        });
    }
    found
}

/// One siteinfo round-trip; any network/HTTP/parse failure is a `None`
/// ("not a MediaWiki here" — the caller tries the next candidate).
async fn fetch_siteinfo(client: &reqwest::Client, api_url: &str) -> Option<SiteInfo> {
    let resp = client
        .get(api_url)
        .query(&[
            ("action", "query"),
            ("meta", "siteinfo"),
            ("siprop", "general"),
            ("format", "json"),
        ])
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES)
        .await
        .ok()?;
    parse_siteinfo(&body).ok()
}

/// The pipeline stands on `list=search`; a wiki that can't answer one is
/// useless to WikiLens even if its siteinfo looks fine. Zero hits are fine —
/// only the response shape must parse.
async fn validate_search(client: &reqwest::Client, api_url: &str) -> Result<(), AppError> {
    let broken = || {
        AppError::Probe(format!(
            "Found a MediaWiki at {api_url}, but its search API didn't answer — WikiLens can't use it."
        ))
    };
    let resp = client
        .get(api_url)
        .query(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", "a"),
            ("srlimit", "1"),
            ("format", "json"),
        ])
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .map_err(|_| broken())?
        .error_for_status()
        .map_err(|_| broken())?;
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES)
        .await
        .map_err(|_| broken())?;
    search::parse_search_response(&body).map_err(|_| broken())?;
    Ok(())
}

/// Fold a (lowercased) accented Latin letter to its ASCII base, so "Pokémon"
/// slugs to "pokemon" (a live host) rather than "pokmon" (a dead one).
/// Non-Latin scripts still drop out — the manual URL field covers those.
fn fold_ascii(c: char) -> Option<char> {
    Some(match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'đ' | 'ď' => 'd',
        'è'..='ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
        'ì'..='ï' | 'ī' | 'į' => 'i',
        'ł' => 'l',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
        'ŕ' | 'ř' => 'r',
        'ś' | 'š' | 'ß' => 's',
        'ť' => 't',
        'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'ž' | 'ź' | 'ż' => 'z',
        _ => return None,
    })
}

/// One lowercased character → its slug character, if it has one.
fn slug_char(c: char) -> Option<char> {
    if c.is_ascii_alphanumeric() {
        Some(c)
    } else {
        fold_ascii(c)
    }
}

/// Game name → registry-style id: lowercase, ASCII letters and digits only,
/// accents folded ("Dave the Diver" → "davethediver", matching the built-in
/// convention).
pub fn slugify(name: &str) -> String {
    name.to_lowercase().chars().filter_map(slug_char).collect()
}

/// The hyphenated variant Fandom favors ("Dave the Diver" → "dave-the-diver").
fn hyphenated_slug(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .map(|word| word.chars().filter_map(slug_char).collect::<String>())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Domains worth probing for a name: joined + hyphenated slugs × wiki.gg +
/// Fandom, deduped (single-word names collapse to two domains), wiki.gg
/// first per the registry's official-over-Fandom curation rule.
pub fn candidate_domains(name: &str) -> Vec<String> {
    let mut domains = Vec::new();
    for slug in [slugify(name), hyphenated_slug(name)] {
        if slug.is_empty() {
            continue;
        }
        for domain in [format!("{slug}.wiki.gg"), format!("{slug}.fandom.com")] {
            if !domains.contains(&domain) {
                domains.push(domain);
            }
        }
    }
    domains
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_defaults_scheme_and_strips_path() {
        assert_eq!(
            normalize_base_url("minecraft.wiki/w/Creeper"),
            Some(("https://minecraft.wiki".to_string(), None))
        );
        assert_eq!(
            normalize_base_url("  https://terraria.wiki.gg/wiki/Zenith?so=1  "),
            Some(("https://terraria.wiki.gg".to_string(), None))
        );
        assert_eq!(
            normalize_base_url("http://localhost:8080/wiki/Main"),
            Some(("http://localhost:8080".to_string(), None))
        );
    }

    #[test]
    fn normalize_keeps_a_pasted_api_php_as_first_candidate() {
        let (base, exact) = normalize_base_url("https://en.uesp.net/w/api.php").unwrap();
        assert_eq!(base, "https://en.uesp.net");
        assert_eq!(exact.as_deref(), Some("https://en.uesp.net/w/api.php"));

        // And it heads the candidate list without duplicating a known path.
        let urls = candidate_api_urls(&base, exact);
        assert_eq!(urls[0], "https://en.uesp.net/w/api.php");
        assert_eq!(
            urls.iter().filter(|u| u.ends_with("/w/api.php")).count(),
            1
        );
    }

    #[test]
    fn normalize_rejects_junk_and_non_http() {
        assert_eq!(normalize_base_url(""), None);
        assert_eq!(normalize_base_url("   "), None);
        assert_eq!(normalize_base_url("not a url"), None);
        assert_eq!(normalize_base_url("ftp://files.example.com"), None);
    }

    #[test]
    fn siteinfo_parses_the_general_fields() {
        // Shape of a real siteinfo response (protocol-relative server: GW2).
        let body = r#"{
            "batchcomplete": "",
            "query": { "general": {
                "sitename": "Guild Wars 2 Wiki",
                "server": "//wiki.guildwars2.com",
                "articlepath": "/wiki/$1",
                "scriptpath": ""
            } }
        }"#;
        let info = parse_siteinfo(body).unwrap();
        assert_eq!(info.sitename, "Guild Wars 2 Wiki");
        assert_eq!(info.server, "//wiki.guildwars2.com");
        assert_eq!(info.articlepath, "/wiki/$1");
        assert_eq!(info.scriptpath, "");
    }

    #[test]
    fn siteinfo_missing_general_is_a_parse_error() {
        assert!(matches!(
            parse_siteinfo(r#"{ "query": {} }"#),
            Err(AppError::Parse(_))
        ));
        // Fandom's dead-subdomain redirect lands on an HTML page.
        assert!(matches!(
            parse_siteinfo("<!doctype html><html>…</html>"),
            Err(AppError::Parse(_))
        ));
    }

    #[test]
    fn derive_handles_the_known_articlepath_shapes() {
        // Root install with /w/ pages (minecraft.wiki).
        let (api, page) = derive_endpoints(
            &SiteInfo {
                sitename: "Minecraft Wiki".into(),
                server: "https://minecraft.wiki".into(),
                articlepath: "/w/$1".into(),
                scriptpath: "".into(),
            },
            "https",
        );
        assert_eq!(api, "https://minecraft.wiki/api.php");
        assert_eq!(page, "https://minecraft.wiki/w/");

        // Scripts under /mediawiki, pages at the root (stardewvalleywiki.com).
        let (api, page) = derive_endpoints(
            &SiteInfo {
                sitename: "Stardew Valley Wiki".into(),
                server: "https://stardewvalleywiki.com".into(),
                articlepath: "/$1".into(),
                scriptpath: "/mediawiki".into(),
            },
            "https",
        );
        assert_eq!(api, "https://stardewvalleywiki.com/mediawiki/api.php");
        assert_eq!(page, "https://stardewvalleywiki.com/");
    }

    #[test]
    fn derive_fixes_up_a_protocol_relative_server() {
        let (api, page) = derive_endpoints(
            &SiteInfo {
                sitename: "Guild Wars 2 Wiki".into(),
                server: "//wiki.guildwars2.com".into(),
                articlepath: "/wiki/$1".into(),
                scriptpath: "".into(),
            },
            "https",
        );
        assert_eq!(api, "https://wiki.guildwars2.com/api.php");
        assert_eq!(page, "https://wiki.guildwars2.com/wiki/");
    }

    #[test]
    fn slugs_match_the_registry_conventions() {
        assert_eq!(slugify("Dave the Diver"), "davethediver");
        assert_eq!(slugify("Path of Exile 2"), "pathofexile2");
        assert_eq!(hyphenated_slug("Dave the Diver"), "dave-the-diver");
        assert_eq!(hyphenated_slug("Core Keeper"), "core-keeper");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn slugs_fold_accented_latin_letters() {
        assert_eq!(slugify("Pokémon"), "pokemon");
        assert_eq!(hyphenated_slug("Ragnarök Online"), "ragnarok-online");
        assert_eq!(slugify("Åska Über"), "askauber");
        // Non-Latin scripts still drop out (manual URL covers these).
        assert_eq!(slugify("ゼルダ"), "");
    }

    #[test]
    fn candidate_domains_cover_both_slug_shapes_and_dedupe() {
        assert_eq!(
            candidate_domains("Dave the Diver"),
            vec![
                "davethediver.wiki.gg",
                "davethediver.fandom.com",
                "dave-the-diver.wiki.gg",
                "dave-the-diver.fandom.com",
            ]
        );
        // Single word: both slug shapes collapse.
        assert_eq!(
            candidate_domains("Terraria"),
            vec!["terraria.wiki.gg", "terraria.fandom.com"]
        );
        assert!(candidate_domains("???").is_empty());
    }

    mod live {
        //! Live probes against real wikis — proves the endpoint derivation on
        //! both articlepath/scriptpath shapes and the suggest flow.
        //! `#[ignore]`d: run with `cargo test -- --ignored` from src-tauri/.
        use super::super::*;

        fn client() -> reqwest::Client {
            crate::http::build_client()
        }

        #[tokio::test]
        #[ignore = "probes live wikis; run with `cargo test -- --ignored`"]
        async fn probe_derives_canonical_endpoints() {
            // Root install, /w/ articlepath.
            let c = probe_base(&client(), "minecraft.wiki").await.unwrap();
            assert_eq!(c.api_url, "https://minecraft.wiki/api.php");
            assert_eq!(c.page_url, "https://minecraft.wiki/w/");

            // api.php under /mediawiki (third candidate path), pages at root.
            let c = probe_base(&client(), "https://stardewvalleywiki.com")
                .await
                .unwrap();
            assert_eq!(c.api_url, "https://stardewvalleywiki.com/mediawiki/api.php");
            assert_eq!(c.page_url, "https://stardewvalleywiki.com/");
        }

        #[tokio::test]
        #[ignore = "probes live wikis; run with `cargo test -- --ignored`"]
        async fn suggest_finds_terraria_on_wiki_gg() {
            let found = suggest(&client(), "Terraria").await;
            assert!(
                found
                    .iter()
                    .any(|c| c.api_url == "https://terraria.wiki.gg/api.php"),
                "terraria.wiki.gg missing from {found:?}"
            );
        }

        #[tokio::test]
        #[ignore = "probes live hosts; run with `cargo test -- --ignored`"]
        async fn probe_rejects_a_non_wiki_host() {
            let err = probe_base(&client(), "example.com").await.unwrap_err();
            assert!(matches!(err, AppError::Probe(_)));
        }
    }
}

/// Offline wiremock tier: `probe_base`'s candidate walk and the
/// "siteinfo answered → a search failure is a verdict" rule
/// (`vault/2026-07-13_rust-http-mock-integration-tests.md`). `suggest` stays
/// live-only — its candidate hosts are hardcoded wiki.gg/Fandom domains —
/// but it shares `fetch_siteinfo`/`validate_search` with `probe_base`.
#[cfg(test)]
mod http_tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    /// A siteinfo body whose `server` points back at the mock, so the derived
    /// canonical endpoints stay on the mock server.
    fn siteinfo_body(server_uri: &str, scriptpath: &str) -> String {
        serde_json::json!({
            "query": { "general": {
                "sitename": "Mock Wiki",
                "server": server_uri,
                "articlepath": "/wiki/$1",
                "scriptpath": scriptpath,
            } }
        })
        .to_string()
    }

    async fn mount_siteinfo(server: &MockServer, api_path: &str, scriptpath: &str) {
        Mock::given(method("GET"))
            .and(path(api_path))
            .and(query_param("meta", "siteinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_string(siteinfo_body(&server.uri(), scriptpath)))
            .mount(server)
            .await;
    }

    async fn mount_search(server: &MockServer, api_path: &str, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path(api_path))
            .and(query_param("list", "search"))
            .respond_with(response)
            .mount(server)
            .await;
    }

    /// Paths of every recorded siteinfo probe, in arrival order.
    async fn siteinfo_request_paths(server: &MockServer) -> Vec<String> {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.query_pairs().any(|(k, v)| k == "meta" && v == "siteinfo"))
            .map(|r| r.url.path().to_string())
            .collect()
    }

    #[tokio::test]
    async fn probe_iterates_candidates_in_order_until_siteinfo_answers() {
        let server = MockServer::start().await;
        let uri = server.uri();
        // Only the third well-known path hosts a wiki; the first two 404
        // (wiremock's default for unmatched requests) → "not a MediaWiki
        // here", try the next candidate.
        mount_siteinfo(&server, "/mediawiki/api.php", "/mediawiki").await;
        mount_search(
            &server,
            "/mediawiki/api.php",
            ResponseTemplate::new(200).set_body_string(r#"{"query":{"search":[]}}"#),
        )
        .await;

        let candidate = probe_base(&crate::http::build_client(), &uri).await.unwrap();
        assert_eq!(
            candidate,
            WikiCandidate {
                name: "Mock Wiki".to_string(),
                api_url: format!("{uri}/mediawiki/api.php"),
                page_url: format!("{uri}/wiki/"),
            }
        );
        assert_eq!(
            siteinfo_request_paths(&server).await,
            vec!["/api.php", "/w/api.php", "/mediawiki/api.php"],
            "candidates probed in the documented order"
        );
    }

    #[tokio::test]
    async fn search_failure_after_siteinfo_is_a_verdict_not_a_retry() {
        let server = MockServer::start().await;
        let uri = server.uri();
        // siteinfo answers at the FIRST candidate — so when its search dies,
        // the probe must abort rather than move on to /w/api.php.
        mount_siteinfo(&server, "/api.php", "").await;
        mount_search(&server, "/api.php", ResponseTemplate::new(500)).await;

        let err = probe_base(&crate::http::build_client(), &uri).await.unwrap_err();
        match err {
            AppError::Probe(msg) => {
                assert!(msg.contains("search API didn't answer"), "msg: {msg}")
            }
            other => panic!("expected AppError::Probe, got {other:?}"),
        }
        assert_eq!(
            siteinfo_request_paths(&server).await,
            vec!["/api.php"],
            "no other candidate path may be tried after siteinfo answered"
        );
    }

    #[tokio::test]
    async fn pasted_api_php_url_is_probed_first() {
        let server = MockServer::start().await;
        let uri = server.uri();
        // An api.php under a prefix no well-known candidate covers — reachable
        // only because a pasted api.php URL heads the candidate list.
        mount_siteinfo(&server, "/custom/api.php", "/custom").await;
        mount_search(
            &server,
            "/custom/api.php",
            ResponseTemplate::new(200).set_body_string(r#"{"query":{"search":[]}}"#),
        )
        .await;

        let candidate = probe_base(&crate::http::build_client(), &format!("{uri}/custom/api.php"))
            .await
            .unwrap();
        assert_eq!(candidate.api_url, format!("{uri}/custom/api.php"));

        let requests = server.received_requests().await.unwrap();
        assert!(
            requests.iter().all(|r| r.url.path() == "/custom/api.php"),
            "every request must hit the pasted endpoint, none the well-known paths"
        );
    }
}
