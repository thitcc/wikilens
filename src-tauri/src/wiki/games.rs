//! Registry of built-in game wikis.
//!
//! Adding a built-in game = add one entry to `GAMES`. Nothing else in the
//! codebase needs to change. User-added wikis (`wiki/user.rs`) share this
//! same `GameWiki` type — which is why the fields are owned `String`s — and
//! are merged in by the command layer, never here.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// A game's MediaWiki endpoints. Owned so built-in and user-added (loaded
/// from `wikis.json` at runtime) entries are the same type; `Deserialize` is
/// for that store — built-ins are constructed below, endpoints verified live.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameWiki {
    /// Stable identifier used by the frontend and the `ask` command (e.g. "stardew").
    pub id: String,
    /// Human-readable name shown in the game picker.
    pub name: String,
    /// MediaWiki `api.php` endpoint used for search + extracts.
    pub api_url: String,
    /// Base URL for human-readable pages; a page URL is `page_url + Encoded_Title`.
    /// Derive it from the wiki's real `articlepath` — some wikis serve pages
    /// under `/w/`, not `/wiki/` (minecraft.wiki, wiki.warframe.com).
    pub page_url: String,
    /// Restrict search to these namespace ids (pipe-separated, e.g. `"134"`).
    /// For shared wikis that keep each game's pages in their own namespace
    /// (UESP) — the default namespaces there return zero or cross-game hits.
    /// `None` = the wiki's default namespaces (and the field is omitted from
    /// stored user entries, which never set it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_namespace: Option<String>,
}

/// One-liner constructor keeping the registry entries readable.
fn wiki(id: &str, name: &str, api_url: &str, page_url: &str) -> GameWiki {
    GameWiki {
        id: id.to_string(),
        name: name.to_string(),
        api_url: api_url.to_string(),
        page_url: page_url.to_string(),
        search_namespace: None,
    }
}

/// The built-in wikis. Every endpoint is verified live before it lands here
/// (siteinfo + a game-specific search — see the registry-expansion plan doc);
/// if one stops responding, keep it here and note it in the README.
/// Curation rule: prefer the official/independent wiki over a stale Fandom
/// copy when both exist (Warframe, Terraria, and Minecraft all migrated).
pub static GAMES: LazyLock<Vec<GameWiki>> = LazyLock::new(|| {
    vec![
        wiki(
            "stardew",
            "Stardew Valley",
            "https://stardewvalleywiki.com/mediawiki/api.php",
            "https://stardewvalleywiki.com/",
        ),
        wiki(
            "corekeeper",
            "Core Keeper",
            "https://core-keeper.fandom.com/api.php",
            "https://core-keeper.fandom.com/wiki/",
        ),
        wiki(
            "conanexiles",
            "Conan Exiles",
            "https://conanexiles.fandom.com/api.php",
            "https://conanexiles.fandom.com/wiki/",
        ),
        // The official wiki (community migrated off Fandom); pages under /w/.
        wiki(
            "warframe",
            "Warframe",
            "https://wiki.warframe.com/api.php",
            "https://wiki.warframe.com/w/",
        ),
        wiki(
            "gw2",
            "Guild Wars 2",
            "https://wiki.guildwars2.com/api.php",
            "https://wiki.guildwars2.com/wiki/",
        ),
        wiki(
            "poe",
            "Path of Exile",
            "https://www.poewiki.net/w/api.php",
            "https://www.poewiki.net/wiki/",
        ),
        wiki(
            "poe2",
            "Path of Exile 2",
            "https://www.poe2wiki.net/w/api.php",
            "https://www.poe2wiki.net/wiki/",
        ),
        wiki(
            "abioticfactor",
            "Abiotic Factor",
            "https://abioticfactor.wiki.gg/api.php",
            "https://abioticfactor.wiki.gg/wiki/",
        ),
        wiki(
            "davethediver",
            "Dave the Diver",
            "https://dave-the-diver.fandom.com/api.php",
            "https://dave-the-diver.fandom.com/wiki/",
        ),
        // UESP covers every Elder Scrolls game; Skyrim lives in namespace 134
        // ("Skyrim:*" titles). Without the filter, search returns zero hits.
        GameWiki {
            search_namespace: Some("134".to_string()),
            ..wiki(
                "skyrim",
                "The Elder Scrolls V: Skyrim",
                "https://en.uesp.net/w/api.php",
                "https://en.uesp.net/wiki/",
            )
        },
        // Nukapedia covers every Fallout game in one namespace; cross-game
        // bleed is accepted for now and measured by the golden queries.
        wiki(
            "fallout4",
            "Fallout 4",
            "https://fallout.fandom.com/api.php",
            "https://fallout.fandom.com/wiki/",
        ),
        wiki(
            "grounded",
            "Grounded",
            "https://grounded.wiki.gg/api.php",
            "https://grounded.wiki.gg/wiki/",
        ),
        // No standalone wiki exists: Grounded 2 pages live on Grounded's
        // Fandom wiki with "(Grounded 2)"-suffixed titles.
        wiki(
            "grounded2",
            "Grounded 2",
            "https://grounded.fandom.com/api.php",
            "https://grounded.fandom.com/wiki/",
        ),
        wiki(
            "terraria",
            "Terraria",
            "https://terraria.wiki.gg/api.php",
            "https://terraria.wiki.gg/wiki/",
        ),
        // minecraft.wiki serves articles under /w/, not /wiki/.
        wiki(
            "minecraft",
            "Minecraft",
            "https://minecraft.wiki/api.php",
            "https://minecraft.wiki/w/",
        ),
    ]
});

/// Look up a built-in wiki by its `id`. Returns `None` for unknown ids —
/// including user-added ones; the command layer falls back to the user store.
pub fn find_game(id: &str) -> Option<&'static GameWiki> {
    GAMES.iter().find(|g| g.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_game_known_and_unknown() {
        assert_eq!(
            find_game("stardew").map(|g| g.name.as_str()),
            Some("Stardew Valley")
        );
        assert_eq!(
            find_game("minecraft").map(|g| g.name.as_str()),
            Some("Minecraft")
        );
        assert!(find_game("does-not-exist").is_none());
    }

    #[test]
    fn skyrim_searches_the_uesp_skyrim_namespace() {
        assert_eq!(
            find_game("skyrim").and_then(|g| g.search_namespace.as_deref()),
            Some("134")
        );
    }

    #[test]
    fn ids_are_unique() {
        for (i, a) in GAMES.iter().enumerate() {
            for b in &GAMES[i + 1..] {
                assert_ne!(a.id, b.id, "duplicate game id: {}", a.id);
            }
        }
    }

    #[test]
    fn stored_form_omits_unset_namespace() {
        // User entries never set the namespace; the stored JSON must stay the
        // documented `{id, name, api_url, page_url}` shape.
        let entry = wiki("x", "X", "https://x.example/api.php", "https://x.example/wiki/");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("search_namespace"), "unexpected field in {json}");

        // And round-trips without it.
        let back: GameWiki = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "x");
        assert!(back.search_namespace.is_none());
    }
}
