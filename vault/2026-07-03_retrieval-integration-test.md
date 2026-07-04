---
title: Live wiki retrieval integration test
type: plan
status: done
created: 2026-07-03
updated: 2026-07-03
tags: [wiki, rag]
related: ["[[2026-07-02_wikitext-extraction]]"]
commit: 3291fc7
---

# Live wiki retrieval integration test

## Context / problem
The Rust suite (~37 unit tests) covers the pure `parse_*` logic against fixed JSON
fixtures, but nothing exercises a real HTTP round-trip. A game wiki changing its API
response shape, or a subtly wrong query param, would pass every unit test and only
surface at runtime. There is no integration tier.

## Goal / non-goals
- **Goal:** one end-to-end test that hits a live game wiki through `search` →
  `fetch_pages` and asserts we get clean, relevant plaintext — proving the wire
  contract the offline unit tests can't.
- **Non-goal:** an LLM smoke test (needs a key + burns tokens) and full GUI automation
  (tauri-driver) — both deferred.

## Approach
- In-crate `#[cfg(test)] mod live` in `src/wiki/mod.rs`, marked `#[ignore]` so
  `cargo test` stays offline and fast; run on demand with `cargo test -- --ignored`.
- Build a `reqwest::Client` with `wiki::USER_AGENT` (MediaWiki etiquette), look up the
  Stardew wiki, search "Abigail", fetch the ranked titles, and assert: non-empty
  titles + pages; each page's text non-empty with no `{{`/`[[` markup left; URLs under
  the wiki's `page_url`; and the corpus mentions "abigail" (relevance sanity).
- Add `tokio` as a dev-dependency for `#[tokio::test]`.

## Decisions & trade-offs
- **In-crate `#[ignore]` test, not a `tests/` integration crate.** A `tests/` crate is
  black-box (public API only), which would force making the `wiki` modules `pub` and
  re-declaring `reqwest`/`tokio` as dev-deps. In-crate tests already see internals and
  the direct `reqwest` dep, so this is lighter for the same coverage. Not architecturally
  significant enough for its own decision doc.
- `#[ignore]` keeps offline/CI `cargo test` green; the network test is opt-in.

## Status log
- 2026-07-03 — created; implementing.
- 2026-07-03 — done; landed in `3291fc7`. All 37 offline tests pass; the ignored
  live test passes against the real Stardew wiki (`cargo test -- --ignored`, ~0.8s).
