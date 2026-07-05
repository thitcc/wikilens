---
title: Built-in game registry — twelve new curated wikis
type: plan
status: todo
created: 2026-07-05
updated: 2026-07-05
tags: [wiki, rust]
related: ["[[2026-07-05_user-added-game-wikis]]", "[[2026-07-04_golden-query-retrieval-tests]]", "[[2026-07-04_search-srwhat-text]]"]
---

# Built-in game registry — twelve new curated wikis

## In simple terms
WikiLens knows three games today. This adds twelve more the user asked for —
Warframe, Guild Wars 2, Path of Exile 1+2, Abiotic Factor, Dave the Diver,
Skyrim, Fallout 4, Grounded 1+2, Terraria, Minecraft — each wired to the best
wiki for that game, all **verified live** on 2026-07-05 (every endpoint
answered a siteinfo call and a game-specific search). One game (Skyrim) needs
a tiny new capability: its wiki (UESP) keeps each game's pages in a separate
namespace, so search grows an optional namespace filter.

## Context / problem
Adding a game is one `GameWiki` entry (`src-tauri/src/wiki/games.rs` —
id/name/api_url/page_url; registry doc-comment: "Nothing else in the codebase
needs to change"). The pipeline is generic MediaWiki, so the work is curation:
pick the *right* wiki per game and prove it responds. Live probes
(2026-07-05) settled every candidate; three curation calls fell out of the
evidence (see Decisions). One wrinkle breaks the "nothing else changes" rule:
UESP's default search returns **zero** hits (content lives in per-game
namespaces; `srnamespace=134` returns exactly the right `Skyrim:*` pages), so
`search.rs` needs an optional `srnamespace` param.

## Goal / non-goals
- **Goal:** twelve new `GameWiki` entries, ids: `warframe`, `gw2`, `poe`,
  `poe2`, `abioticfactor`, `davethediver`, `skyrim`, `fallout4`, `grounded`,
  `grounded2`, `terraria`, `minecraft`.
- **Goal:** optional per-game search namespace (`search_namespace:
  Option<&'static str>` on `GameWiki`; conditional `("srnamespace", ns)` push
  in `search.rs`, same pattern as the conditional `srwhat` push).
- **Goal:** 1–2 golden queries per new game in
  `golden_queries_hit_expected_pages` (`wiki/mod.rs`) so retrieval quality is
  measured, not assumed.
- **Non-goal:** per-game query hints for shared wikis (bleed accepted for v1,
  see Decisions); user-added games (own plan:
  [[2026-07-05_user-added-game-wikis]]); any UI change (the picker fills
  itself from `list_games`).

## Approach
1. **`search.rs`** — add the optional `srnamespace` param; unit-test the
   param construction (present only when set).
2. **`games.rs`** — the twelve entries, endpoints verbatim from the verified
   table below; `skyrim` sets `search_namespace: Some("134")`, all others
   `None` (existing three entries gain the field with `None`).
3. **Golden queries** — extend the live suite; mark shared-wiki cases
   non-strict where cross-game bleed is plausible (fallout4, grounded2)
   until measured.
4. **Verify** — `cargo test` (offline), then the ignored golden suite live,
   then a spot-check in the running app (one question per a few games,
   confirming answers and source links resolve).

Verified endpoints (live-probed 2026-07-05 — siteinfo + game-term search):

| id | name | api_url | page_url |
|---|---|---|---|
| warframe | Warframe | `https://wiki.warframe.com/api.php` | `https://wiki.warframe.com/w/` |
| gw2 | Guild Wars 2 | `https://wiki.guildwars2.com/api.php` | `https://wiki.guildwars2.com/wiki/` |
| poe | Path of Exile | `https://www.poewiki.net/w/api.php` | `https://www.poewiki.net/wiki/` |
| poe2 | Path of Exile 2 | `https://www.poe2wiki.net/w/api.php` | `https://www.poe2wiki.net/wiki/` |
| abioticfactor | Abiotic Factor | `https://abioticfactor.wiki.gg/api.php` | `https://abioticfactor.wiki.gg/wiki/` |
| davethediver | Dave the Diver | `https://dave-the-diver.fandom.com/api.php` | `https://dave-the-diver.fandom.com/wiki/` |
| skyrim | The Elder Scrolls V: Skyrim | `https://en.uesp.net/w/api.php` (+ srnamespace 134) | `https://en.uesp.net/wiki/` |
| fallout4 | Fallout 4 | `https://fallout.fandom.com/api.php` | `https://fallout.fandom.com/wiki/` |
| grounded | Grounded | `https://grounded.wiki.gg/api.php` | `https://grounded.wiki.gg/wiki/` |
| grounded2 | Grounded 2 | `https://grounded.fandom.com/api.php` | `https://grounded.fandom.com/wiki/` |
| terraria | Terraria | `https://terraria.wiki.gg/api.php` | `https://terraria.wiki.gg/wiki/` |
| minecraft | Minecraft | `https://minecraft.wiki/api.php` | `https://minecraft.wiki/w/` |

Gotcha caught by the probes: Minecraft's article path is `/w/$1` (so is the
official Warframe wiki's) — not the usual `/wiki/`. The `page_url` column is
derived from each wiki's actual `articlepath`, never assumed.

## Decisions & trade-offs
- **Prefer the official/independent wiki over a Fandom copy when both exist**
  (Warframe → wiki.warframe.com, Terraria → wiki.gg, Minecraft →
  minecraft.wiki): those communities migrated and the Fandom copies rot.
  Dave the Diver stays on Fandom — no alternative exists.
- **Skyrim → UESP with a namespace filter**, not elderscrolls.fandom: UESP is
  the canonical Elder Scrolls source; the filter costs one optional field and
  returns exactly `Skyrim:*` pages, where the Fandom wiki mixes Arena/Oblivion
  pages into results.
- **Fallout 4 → Nukapedia with cross-game bleed accepted**: fallout.wiki
  exposes no reachable `api.php` (three paths probed), and Nukapedia has no
  per-game namespaces. Golden queries measure the bleed; a per-game query
  hint is a possible follow-up if it hurts.
- **Grounded 2 shares Grounded's Fandom wiki** — no standalone G2 wiki exists
  (probes: grounded2.wiki.gg / .fandom.com are dead; grounded.fandom's top
  "Aphid" hit is literally "Aphid (Grounded 2)"). Grounded 1 points at the
  cleaner wiki.gg (G1-only), Grounded 2 at the shared Fandom wiki.

## Status log
- 2026-07-05 — created after the games discussion; all twelve wikis
  live-probed and the three curation calls settled; queued as todo alongside
  [[2026-07-05_user-added-game-wikis]].
