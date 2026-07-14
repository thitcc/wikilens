---
title: Bound the retrieval path — 4s rewrite timeout, 8s search timeout
type: plan
status: done
created: 2026-07-10
updated: 2026-07-14
tags: [llm, wiki, rust]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_rewrite-circuit-breaker]]", "[[2026-07-13_wiki-fetch-hardening]]"]
commit: a245f18
---

# Bound the retrieval path — 4s rewrite timeout, 8s search timeout

## Context / problem

The eager query rewrite (`llm::rewrite_query`) is the only unbounded HTTP call on the ask's
critical path: the shared `reqwest::Client` deliberately has no global timeout — per-request
timeouts are the codebase convention (page fetch 12s; models/probe/titles 8s) — and the
rewrite request sets none. Because the rewrite runs inside `futures_util::future::join` with
the raw search, a hung LLM host serializes into **every** ask: the UI freezes at "Searching…",
there is no cancel, and `AppState::ask_in_progress` rejects retries until restart. The raw
search (`search_full`) is likewise uncapped. This was the multi-agent review's HIGH finding
(three independent verifiers confirmed). Live debug-table evidence (2026-07-10, Abigail ask):
even the *healthy* case cost 2432ms of rewrite against a 464ms raw search — the join waited
~2s on the rewrite; the pathological case waits forever.

## Goal / non-goals

- Goal: every call on the ask critical path has an explicit per-request timeout; a dead or
  slow rewrite/search host degrades the ask instead of hanging it.
- Goal: a timed-out rewrite falls into the existing graceful "no candidates" path (raw search
  still answers).
- Non-goal: no timeout on the streaming answer call (long generations are legitimate; the
  stream delivers progress).
- Non-goal: no global client timeout — keep the per-request convention.

## Approach

1. `src-tauri/src/llm.rs`: `const REWRITE_TIMEOUT: Duration = Duration::from_secs(4);`
   applied as `request.timeout(REWRITE_TIMEOUT).send()` in `rewrite_query` **only** — take
   care not to touch the streaming path in `answer_streaming` (both build via
   `reqwest::RequestBuilder`; target the rewrite call site). Doc comment: the join blocks the
   whole ask, and the shared client deliberately has no global timeout.
2. `src-tauri/src/wiki/search.rs`: `const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);`
   applied in `search_full` — transitively covers the raw search, both candidate searches,
   and every retry-net search (all route through it via `search`); 8s matches the
   models/probe/titles convention.
3. No debug-table changes needed: a timeout surfaces as an `Err` and lands in the existing
   `rewrite … error: <msg>` phase row / `record` path of `WIKILENS_DEBUG`.

## Decisions & trade-offs

- 4s rewrite budget is deliberately tight: the reply is ≤256 tokens
  (`REWRITE_MAX_TOKENS`) on a model expected to be fast; live healthy-case measurements
  (~2.4s cold, less warm) fit inside it, and a slow rewrite is worth abandoning — the raw
  search already has hits by then.
- Timeout applies per request, not per phase — a timed-out rewrite still costs its 4s in the
  join before falling back. The session circuit breaker
  ([[2026-07-10_rewrite-circuit-breaker]]) exists to stop paying that repeatedly.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#1, priority 1); execution order:
  first of the seven finding plans.
- 2026-07-14 — **done** in `a245f18`. Most of the plan had already landed via the
  [[2026-07-13_wiki-fetch-hardening]] sweep (`a1db9d7`): `SEARCH_TIMEOUT = 8s` in
  `search_full` (this plan's exact target, with its `#[ignore]`d pin
  `search_hang_times_out`) and an interim `REWRITE_TIMEOUT = 15s`. This change delivers
  the remaining delta: the rewrite budget drops 15s → 4s per the rationale above, the
  constant's doc comment loses its stale "zero-hit ask" premise (the rewrite is eager
  now), and the missing timeout pin lands (`rewrite_hang_times_out`, `#[ignore]`d like
  the search/fetch pins; passes in ~4.0s — via the timeout, not the mock's 6s delay).
  `RewriteOutcome` gains `#[derive(Debug)]` for the test's `unwrap_err`. Step 2 of the
  approach (search timeout) required no code — verified present and pinned.
