---
title: Fix multi-word wiki search with srwhat=text
type: plan
status: done
created: 2026-07-04
updated: 2026-07-04
tags: [wiki, rag, rust]
related: ["[[2026-07-03_retrieval-integration-test]]", "[[2026-07-04_query-preprocessing-zero-hit-retry]]", "[[2026-07-04_golden-query-retrieval-tests]]"]
commit: [53cd6c4, 9506cad]
---

# Fix multi-word wiki search with srwhat=text

## In simple terms
When a player asks a normal question like "best crops for winter" on Stardew, the
app says it found nothing — but the wiki has the answer. The bug: the Stardew wiki's
search only looks at page *titles* unless we explicitly ask it to search page *text*.
Adding one parameter (`srwhat=text`) to our search request fixes the whole class of
failures. No AI, prompt, or ranking change involved — the AI was never even being
called on these questions.

## Context / problem
Live probes (2026-07-04) against `stardewvalleywiki.com`: it runs MediaWiki 1.35.1
with **no search extension** — the default MySQL engine (`SearchMySQL`). On that
engine, our `list=search` call without `srwhat` behaves as a **title search**: every
multi-word query returns 0 hits ("best crops for winter", "winter crops",
"Abigail gifts", even "crops winter"), while single words ("wood", "winter") match
fine. Zero titles short-circuits `run_ask` into the canned "couldn't find anything"
answer (`commands.rs:143-151`) — the LLM is never invoked.

With `srwhat=text` the same queries all succeed with highly relevant top-4 results:
"best crops for winter" → Powdermelon, Winter Seeds, Seasons, Winter (81 hits);
"Abigail gifts" → Villagers, Leek, Tea Set (her loved gifts); "how do I make Abigail
like me" → Telephone, Pierre, Caroline, Abigail. Verified the parameter is a no-op on
Fandom (Core Keeper's Elasticsearch-backed UnifiedSearch returns identical results
with and without it), so it is safe to send unconditionally for every registered wiki.
Note: CirrusSearch rejects `srwhat=title`, but `text` is its default mode — no risk.

## Goal / non-goals
- **Goal:** multi-word / natural-language questions retrieve pages on
  default-engine wikis (Stardew) exactly as they already do on Fandom wikis.
- **Non-goal:** query preprocessing, retries, or ranking changes — that's
  [[2026-07-04_query-preprocessing-zero-hit-retry]].

## Approach
- Add `("srwhat", "text")` to the query params in `src-tauri/src/wiki/search.rs`
  (the param block at `search.rs:20-26`).
- Run the offline unit tests (`cd src-tauri && cargo test`) — the `parse_*` tests are
  fixture-based and should be unaffected.
- Verify live: `cargo test -- --ignored` (existing live test), then a manual ask of
  "best crops for winter" on Stardew through the app.
- Measurement lands separately in [[2026-07-04_golden-query-retrieval-tests]].

## Decisions & trade-offs
- Send the parameter unconditionally rather than per-wiki: it is the default on
  Elasticsearch-backed wikis and the fix on default-engine wikis, so there is no
  fork worth a decision doc.
- Keep assuming lowest-common-denominator search semantics (no stemming, all terms
  ANDed) — `srwhat=text` fixes *where* we search, not those semantics.

## Status log
- 2026-07-04 — created from the retrieval-failure diagnosis; queued as step 1 of 4.
- 2026-07-04 — done; landed in `53cd6c4` with a CLAUDE.md §5 gotcha. All 37 offline
  tests pass; the ignored live test passes against the real Stardew wiki with the
  new param (`cargo test -- --ignored`, ~0.7s).
- 2026-07-04 — revised in `9506cad`: the golden suite
  ([[2026-07-04_golden-query-retrieval-tests]]) showed unconditional `srwhat=text`
  demotes exact-title pages on single-word queries ("wood" ranked Wood Chipper
  above Wood), so it is now sent for multi-word queries only — single words
  title-match fine on both engine types.
