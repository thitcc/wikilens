//! Static registry of supported game wikis.
//!
//! Adding a new game = add one `GameWiki` entry to `GAMES`. Nothing else in the
//! codebase needs to change.

/// A supported game's MediaWiki endpoints.
#[derive(Debug, Clone, Copy)]
pub struct GameWiki {
    /// Stable identifier used by the frontend and the `ask` command (e.g. "stardew").
    pub id: &'static str,
    /// Human-readable name shown in the game picker.
    pub name: &'static str,
    /// MediaWiki `api.php` endpoint used for search + extracts.
    pub api_url: &'static str,
    /// Base URL for human-readable pages; a page URL is `page_url + Encoded_Title`.
    /// Derive it from the wiki's real `articlepath` — some wikis serve pages
    /// under `/w/`, not `/wiki/` (minecraft.wiki, wiki.warframe.com).
    pub page_url: &'static str,
    /// Restrict search to these namespace ids (pipe-separated, e.g. `"134"`).
    /// For shared wikis that keep each game's pages in their own namespace
    /// (UESP) — the default namespaces there return zero or cross-game hits.
    /// `None` = the wiki's default namespaces.
    pub search_namespace: Option<&'static str>,
}

/// The supported wikis. Every endpoint is verified live before it lands here
/// (siteinfo + a game-specific search — see the registry-expansion plan doc);
/// if one stops responding, keep it here and note it in the README.
/// Curation rule: prefer the official/independent wiki over a stale Fandom
/// copy when both exist (Warframe, Terraria, and Minecraft all migrated).
pub static GAMES: &[GameWiki] = &[
    GameWiki {
        id: "stardew",
        name: "Stardew Valley",
        api_url: "https://stardewvalleywiki.com/mediawiki/api.php",
        page_url: "https://stardewvalleywiki.com/",
        search_namespace: None,
    },
    GameWiki {
        id: "corekeeper",
        name: "Core Keeper",
        api_url: "https://core-keeper.fandom.com/api.php",
        page_url: "https://core-keeper.fandom.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "conanexiles",
        name: "Conan Exiles",
        api_url: "https://conanexiles.fandom.com/api.php",
        page_url: "https://conanexiles.fandom.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "warframe",
        name: "Warframe",
        // The official wiki (community migrated off Fandom); pages under /w/.
        api_url: "https://wiki.warframe.com/api.php",
        page_url: "https://wiki.warframe.com/w/",
        search_namespace: None,
    },
    GameWiki {
        id: "gw2",
        name: "Guild Wars 2",
        api_url: "https://wiki.guildwars2.com/api.php",
        page_url: "https://wiki.guildwars2.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "poe",
        name: "Path of Exile",
        api_url: "https://www.poewiki.net/w/api.php",
        page_url: "https://www.poewiki.net/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "poe2",
        name: "Path of Exile 2",
        api_url: "https://www.poe2wiki.net/w/api.php",
        page_url: "https://www.poe2wiki.net/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "abioticfactor",
        name: "Abiotic Factor",
        api_url: "https://abioticfactor.wiki.gg/api.php",
        page_url: "https://abioticfactor.wiki.gg/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "davethediver",
        name: "Dave the Diver",
        api_url: "https://dave-the-diver.fandom.com/api.php",
        page_url: "https://dave-the-diver.fandom.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "skyrim",
        name: "The Elder Scrolls V: Skyrim",
        // UESP covers every Elder Scrolls game; Skyrim lives in namespace 134
        // ("Skyrim:*" titles). Without the filter, search returns zero hits.
        api_url: "https://en.uesp.net/w/api.php",
        page_url: "https://en.uesp.net/wiki/",
        search_namespace: Some("134"),
    },
    GameWiki {
        id: "fallout4",
        name: "Fallout 4",
        // Nukapedia covers every Fallout game in one namespace; cross-game
        // bleed is accepted for now and measured by the golden queries.
        api_url: "https://fallout.fandom.com/api.php",
        page_url: "https://fallout.fandom.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "grounded",
        name: "Grounded",
        api_url: "https://grounded.wiki.gg/api.php",
        page_url: "https://grounded.wiki.gg/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "grounded2",
        name: "Grounded 2",
        // No standalone wiki exists: Grounded 2 pages live on Grounded's
        // Fandom wiki with "(Grounded 2)"-suffixed titles.
        api_url: "https://grounded.fandom.com/api.php",
        page_url: "https://grounded.fandom.com/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "terraria",
        name: "Terraria",
        api_url: "https://terraria.wiki.gg/api.php",
        page_url: "https://terraria.wiki.gg/wiki/",
        search_namespace: None,
    },
    GameWiki {
        id: "minecraft",
        name: "Minecraft",
        // minecraft.wiki serves articles under /w/, not /wiki/.
        api_url: "https://minecraft.wiki/api.php",
        page_url: "https://minecraft.wiki/w/",
        search_namespace: None,
    },
];

/// Look up a wiki by its `id`. Returns `None` for unknown ids.
pub fn find_game(id: &str) -> Option<&'static GameWiki> {
    GAMES.iter().find(|g| g.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_game_known_and_unknown() {
        assert_eq!(find_game("stardew").map(|g| g.name), Some("Stardew Valley"));
        assert_eq!(find_game("minecraft").map(|g| g.name), Some("Minecraft"));
        assert!(find_game("does-not-exist").is_none());
    }

    #[test]
    fn skyrim_searches_the_uesp_skyrim_namespace() {
        assert_eq!(
            find_game("skyrim").and_then(|g| g.search_namespace),
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
}
