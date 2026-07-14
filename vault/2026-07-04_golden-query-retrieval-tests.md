---
title: Golden-query live retrieval tests
type: plan
status: done
created: 2026-07-04
updated: 2026-07-13
tags: [wiki, rag, rust]
related: ["[[2026-07-03_retrieval-integration-test]]", "[[2026-07-04_search-srwhat-text]]", "[[2026-07-04_query-preprocessing-zero-hit-retry]]", "[[2026-07-13_manual-smoke-checklist-live-cadence]]"]
commit: [9506cad, 65ace9e]
---

# Golden-query live retrieval tests

## In simple terms
"Did this change actually make answers better?" is hard to judge for AI output — but
easy for the search step. If a player asks "best crops for winter", we know the
Winter Seeds page should be among the 4 pages we hand the AI. This plan writes down
a list of real questions with the pages they should retrieve, as tests we can run
before and after any change. It turns "I think it's better" into a pass/fail number
— without spending a single AI token.

## Context / problem
Answer quality is fuzzy to evaluate, but retrieval is deterministic: either the
right page is in the top-4 the model receives or it isn't. The existing ignored
live test (`src/wiki/mod.rs`, from [[2026-07-03_retrieval-integration-test]]) covers
one exact-title query ("Abigail") — it proves the wire contract but cannot catch the
natural-language failure class that motivated [[2026-07-04_search-srwhat-text]] and
[[2026-07-04_query-preprocessing-zero-hit-retry]]. Without golden queries, those
fixes ship unmeasured and future regressions go unnoticed.

## Goal / non-goals
- **Goal:** a golden set of natural-language questions with expected pages,
  asserted as hit@top-4 in ignored live tests — the before/after yardstick for any
  retrieval change.
- **Non-goal:** LLM answer-quality evaluation (needs API keys, burns tokens,
  subjective judging — explicitly scoped out by the earlier test plan too).
- **Non-goal:** running these in CI — they stay `#[ignore]`, opt-in via
  `cargo test -- --ignored`, keeping offline `cargo test` fast and green.

## Approach
- Extend the existing `#[cfg(test)] mod live` in `src-tauri/src/wiki/mod.rs` with a
  table of golden cases: `(game_id, question, expected_any_of_titles)`. Seed set
  from the live probes:
  - stardew: "best crops for winter" → any of {Winter Seeds, Powdermelon, Seasons}
  - stardew: "what does Abigail like as a gift" → any of {Abigail, Villagers, Leek}
  - stardew: "wood" → {Wood} (regression guard for the already-working case)
  - corekeeper: "best food for early game" → any of {Cooking, Foods}
  - corekeeper: "how do I get more health" → any of {Health, Healing potency}
- One helper that runs all cases, asserts each hit@4, and prints a per-case summary
  so a partial failure is readable at a glance.
- After [[2026-07-04_query-preprocessing-zero-hit-retry]] lands: add a case that
  zero-hits on the raw query, proving the fallback engages.
- Keep requests sequential with `wiki::USER_AGENT` (MediaWiki etiquette).

## Decisions & trade-offs
- **hit@top-4 as the metric:** matches `DEFAULT_SEARCH_LIMIT` — it measures exactly
  what the model gets to see, nothing softer.
- **Live wikis over recorded fixtures:** the wire contract and the wikis' real
  ranking are the thing under test; fixtures would go stale silently. Cost: tests
  can break when a wiki reorganizes pages — acceptable for an opt-in suite, and
  `expected_any_of` absorbs minor ranking shuffles.

## Status log
- 2026-07-04 — created from the retrieval-failure diagnosis; queued as step 3 of 4
  (write after step 1 so the golden set starts green, then guard steps 2 and 4).
- 2026-07-04 — done; suite in `65ace9e`. The very first run caught a real
  regression from [[2026-07-04_search-srwhat-text]]: fulltext demoted exact-title
  pages on single-word queries ("wood" → Wood Chipper, no Wood), plus a stopword
  gap ("as"); both fixed in `9506cad` (srwhat=text now multi-word only). Two
  deviations from the plan: cases carry a `strict` flag — the singular-"gift"
  phrasing is tracked as a KNOWN-GAP (AND + no stemming) instead of gating, to be
  flipped strict when query handling or extraction improves; and the raw-query
  zero-hit fallback case became a nonsense-query guard (empty list, not a parse
  error) since a deterministic preprocess-fails/simplify-rescues live case can't
  be constructed reliably. Final: 6 cases — 5 strict OK, 1 known gap; 42 offline +
  3 live tests green.
- 2026-07-13 — table hoisted to a shared `#[cfg(test)] const GOLDEN_CASES` at
  the `wiki` module level (with [[2026-07-13_manual-smoke-checklist-live-cadence]],
  `766be60`): an offline test now enforces the "every built-in game has a golden
  query" anchor rule, which immediately exposed conanexiles as the one uncovered
  game — a strict case added ("how do I make steel" → Steel Bar/Steelfire,
  live-verified, Steel Bar ranks #1). Suite is now 20 cases — 18 strict OK,
  2 known gaps (Abigail-gift, fusion-core bleed). The run cadence this suite
  lacked is documented in `docs/smoke-checklist.md`; known-gap flips keep being
  recorded here.
