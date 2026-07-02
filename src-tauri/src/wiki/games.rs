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
    pub page_url: &'static str,
}

/// The supported wikis. Endpoints are verified during scaffolding (Phase 5);
/// if one stops responding, keep it here and note it in the README.
pub static GAMES: &[GameWiki] = &[
    GameWiki {
        id: "stardew",
        name: "Stardew Valley",
        api_url: "https://stardewvalleywiki.com/mediawiki/api.php",
        page_url: "https://stardewvalleywiki.com/",
    },
    GameWiki {
        id: "corekeeper",
        name: "Core Keeper",
        api_url: "https://core-keeper.fandom.com/api.php",
        page_url: "https://core-keeper.fandom.com/wiki/",
    },
    GameWiki {
        id: "conanexiles",
        name: "Conan Exiles",
        api_url: "https://conanexiles.fandom.com/api.php",
        page_url: "https://conanexiles.fandom.com/wiki/",
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
        assert!(find_game("does-not-exist").is_none());
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
