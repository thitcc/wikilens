---
title: Query preprocessing and zero-hit retry
type: plan
status: done
created: 2026-07-04
updated: 2026-07-04
tags: [wiki, rag, rust, frontend]
related: ["[[2026-07-04_search-srwhat-text]]", "[[2026-07-04_golden-query-retrieval-tests]]"]
commit: 66c4c9f
---

# Query preprocessing and zero-hit retry

## In simple terms
Even after the `srwhat=text` fix, the Stardew wiki's search is strict: every
meaningful word in the question must literally appear on a page, and "crops" does
not match "crop". So a chatty question with one unusual word can still come back
empty. This plan cleans the question into search keywords before searching (dropping
filler like "how do I"), and if the search still finds nothing, it automatically
retries once with a simpler query — telling the player "Broadening the search…"
instead of failing silently.

## Context / problem
The default MySQL fulltext engine (Stardew) ANDs every non-stopword term and has
**no stemming** — live probes showed "crop" and "crops" return disjoint top results.
A verbose question containing one rare or misspelled word zeroes out even with
`srwhat=text`. Today the raw question goes verbatim into `srsearch`
(`search.rs:23`), and a zero-hit search jumps straight to the canned "couldn't find
anything" answer (`commands.rs:143-151`) with no second attempt and no signal to the
player that a fallback could have helped.

## Goal / non-goals
- **Goal:** chatty questions degrade gracefully — keyword-strip before searching,
  one automatic simplified retry on zero hits, and a visible "Broadening the
  search…" status so the UX stays honest.
- **Non-goal:** LLM-based query rewriting (an extra model call per ask — latency,
  cost, and a key dependency; deferred until [[2026-07-04_golden-query-retrieval-tests]]
  proves the rule-based version insufficient. Taking it later is a real fork →
  decision doc first).
- **Non-goal:** changing `DEFAULT_SEARCH_LIMIT` or result ranking.

## Approach
- New pure fn `preprocess_query(question: &str) -> String` in
  `src-tauri/src/wiki/search.rs`: tokenize on whitespace, strip punctuation, drop a
  small const English stopword list (how, do, i, what, is, the, a, best, way, to,
  get, …), rejoin; if the result is empty, return the trimmed original — never send
  an empty `srsearch`. Offline unit tests beside `parse_search_response`
  (`search.rs:57`).
- Call it at the search call site in `run_ask` (`commands.rs:142`).
- Zero-hit branch: before the canned answer, retry once with a harsher pass (e.g.
  keep only tokens with len ≥ 4 or Capitalized tokens); only one extra HTTP request
  and only on the zero-hit path (MediaWiki etiquette).
- Emit `ask://status` `"retrying"` in the retry branch (same fire-and-forget
  `let _ =` style as the other emits); extend the `AskStatus` union in
  `src/types.ts:28` and the label map in `App.tsx` with
  `retrying: "Broadening the search…"`.
- Optional (decide during implementation): add `fallback_query: Option<String>` to
  `AskResult` so the UI can show "searched for: `<keywords>`" next to the sources.

## Decisions and trade-offs
- Rule-based preprocessing over LLM rewrite: probes showed the failing class is
  mechanical (stopwords + AND semantics), so rules should cover it at zero latency
  and zero cost; measure before escalating.
- Retry once, not a loop: bounded requests, predictable UX.

## Status log
- 2026-07-04 — created from the retrieval-failure diagnosis; queued as step 2 of 4,
  after [[2026-07-04_search-srwhat-text]].
- 2026-07-04 — done; landed in `66c4c9f`. `preprocess_query` + `simplify_query` with
  5 offline unit tests (42 total green), `retrying` status wired through types.ts
  and App.tsx, tsc + live test green. The optional `fallback_query` field on
  `AskResult` was skipped — the status event is enough transparency for now.
