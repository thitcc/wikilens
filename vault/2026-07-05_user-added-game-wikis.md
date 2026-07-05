---
title: User-added game wikis with probe validation
type: plan
status: done
created: 2026-07-05
updated: 2026-07-05
tags: [wiki, rust, frontend]
related: ["[[2026-07-05_builtin-game-registry-expansion]]", "[[2026-07-05_model-picker-menu]]"]
commit: 513c072
---

# User-added game wikis with probe validation

## In simple terms
Today only we can add games (a code change per game). This lets the player do
it from the overlay: type a game name or paste its wiki address, WikiLens
checks the wiki actually works (it must be a MediaWiki with a usable API —
the thing the whole answer pipeline depends on), and only then saves it. No
settings screen, no web search — an "Add a game…" flow in the game picker,
with WikiLens guessing the two most common wiki hosts automatically.

## Context / problem
The registry is static Rust data; the retrieval pipeline is generic MediaWiki
(any Fandom / wiki.gg / self-hosted wiki works — proven across all engine
classes in [[2026-07-05_builtin-game-registry-expansion]]). But not every
game wiki *is* a MediaWiki (e.g. Fextralife), so "paste any URL" without
validation would silently produce garbage answers. And the registry must stay
Rust-side: `ask` should never fetch a URL the frontend supplies per-request
(trust boundary — the webview has no network permissions for a reason).

## Goal / non-goals
- **Goal:** user-added games persist in a JSON config in the Tauri app-data
  dir, merged with the built-in registry at load; `find_game` remains the
  single lookup path.
- **Goal:** `add_game` command with **probe validation** in Rust: given a
  domain/URL, try the well-known API paths (`/api.php`, `/w/api.php`); the
  candidate must answer a `siteinfo` call and a test search before it is
  saved. `remove_game(id)` for user entries only.
- **Goal:** auto-suggest from a bare name: slugify and probe
  `{slug}.wiki.gg` and `{slug}.fandom.com`, offer verified hits — two HTTP
  probes, no search engine.
- **Goal:** "Add a game…" affordance in the game-picker area (menu pattern
  like the model menu), with probe failures surfaced plainly ("couldn't find
  a MediaWiki API there").
- **Non-goal:** auto-discovering wikis via web search — flaky, adds a
  dependency, and still needs the same validation afterwards. Rejected.
- **Non-goal:** editing/removing built-in games; a full Settings panel (build
  one when there's more than one setting — the configurable hotkey would
  justify it later); exposing the namespace filter to users (advanced field,
  revisit on demand).

## Approach
1. **Storage** — `wikis.json` in the app-data dir (Tauri path resolver):
   `[{id, name, api_url, page_url}]`. Loaded once at startup into state;
   written on add/remove. Implementation decision at execution time:
   `GameWiki` is all `&'static str`, so user entries need either an owned
   variant unified behind a small trait/enum, or `Box::leak` at load (leak is
   bounded — entries live for the process lifetime anyway).
2. **Probe (`wiki/probe.rs` or similar)** — candidate URL → try API paths →
   `action=query&meta=siteinfo` (yields sitename + `articlepath`, from which
   `page_url` is *derived*, never assumed — the `/w/$1` vs `/wiki/$1` split
   is real, see the registry plan) → one test search. Timeout in line with
   the 8s model-fetch precedent. Sequential, per MediaWiki etiquette.
3. **Commands** — `add_game(name, url_or_blank) -> GameInfo` (blank URL ⇒
   auto-suggest flow returns candidates instead), `remove_game(id)`. Exact
   IPC shape (one command with modes vs `suggest_wikis` + `add_game`) decided
   at execution.
4. **Frontend** — "Add a game…" entry point by the game picker; name field;
   candidate list (verified probes) or manual URL fallback; saved game
   selected immediately. `list_games` already feeds the picker, so new
   entries appear with no other UI change.
5. **Docs** — CLAUDE.md §4 "Adding a game" bullet gains the user-path
   sentence; gotcha for the articlepath derivation.

## Decisions & trade-offs
- **Probe-validate, never trust a pasted URL** — the answer quality contract
  ("only from the wiki") is only as good as the wiki behind it.
- **Pattern-guess wiki.gg/Fandom instead of web search** — covers the large
  majority of game wikis for two requests and zero dependencies; the manual
  URL field covers the rest (UESP-style independents).
- **App-data JSON over localStorage** — the registry is consumed Rust-side
  (search/fetch), must survive webview storage resets, and keeps the ask
  command's trust boundary intact.
- If the storage/ownership question (`&'static` vs owned registry) turns out
  to have architectural weight during execution, split it into a decision doc.

## Status log
- 2026-07-05 — created from the games discussion (recommendation accepted:
  curated built-ins + validated user additions; web-search discovery
  rejected). Queued as todo; execute after
  [[2026-07-05_builtin-game-registry-expansion]].
- 2026-07-05 — placement settled in discussion before execution: the flow
  lives in an "Add a game" popover off a "+" button by the picker (ModelMenu
  pattern, top-anchored); **no settings panel** until a second real setting
  exists (configurable hotkey would be the trigger); no design gate — the
  menu pattern and tokens already exist, a Claude Design pass on the probe
  states can follow once they're real.
- 2026-07-05 — executed and landed in `513c072`. The ownership question went
  **owned `GameWiki`** (String fields, `LazyLock` registry, `find_game`
  signature unchanged) — mechanical and contained (~4 files), so no separate
  decision doc. Probe learnings: `/mediawiki/api.php` joined the candidate
  paths (stardewvalleywiki.com is live proof), endpoints are derived from
  siteinfo's `server`+`scriptpath`+`articlepath` (protocol-relative servers
  fixed up; redirect aliases dedupe on the canonical api_url), and both
  joined+hyphenated slugs are probed (dave-the-diver.fandom.com). All three
  live probe tests passed first run — minecraft.wiki (`/w/`, root
  scriptpath), stardew (`/mediawiki`), suggest("Terraria") → terraria.wiki.gg
  — and the golden suite reran clean after the refactor. Adversarial review
  (28 agents, 4 lenses, 3-refuter panels): 8 raw findings → 7 upheld → 5
  distinct defects, all fixed: startup read-error emptied the store and the
  next add clobbered wikis.json (now: only NotFound ⇒ empty; other errors ⇒
  mutations refused); a future built-in claiming a stored id made it
  undeletable (remove_game now consults the store first, list_games hides
  shadowed entries); stale suggestions survived a name edit; double-click
  remove painted a bogus error (busy state + synchronous row drop); slugify
  dropped accents ("Pokémon" probed dead domains — now folded to ASCII).
  Offline: 89 tests. In-app smoke (add a real wiki, ask, remove) stays with
  the user.
