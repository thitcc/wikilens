---
title: Session circuit breaker for a rewrite that never yields candidates
type: plan
status: todo
created: 2026-07-10
updated: 2026-07-10
tags: [llm, rust]
related: ["[[2026-07-10_retrieval-path-timeouts]]", "[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_reasoning-skip-and-capability-tags]]"]
commit:
---

# Session circuit breaker for a rewrite that never yields candidates

## Context / problem

When the effective rewrite model is doomed — a reasoning-only model whose reply lands in
`reasoning_content` (parser reads `content` → zero candidates), a misconfigured
`WIKILENS_REWRITE_MODEL`, an unreachable host — every single ask pays the full rewrite cost
(multi-second; bounded at 4s once [[2026-07-10_retrieval-path-timeouts]] lands) and gets
nothing back, silently, for the whole session. The failure is invisible unless
`WIKILENS_TRACE_RETRIEVAL` or `WIKILENS_DEBUG` is on. Proposed by the second AI review,
confirmed by the verifier pass.

## Goal / non-goals

- Goal: after 2 consecutive rewrite attempts that produce zero candidates (error or empty
  parse), stop attempting rewrites for the rest of the session, and say so **once, loudly**
  on stderr — including likely causes and the fixes.
- Goal: any success fully resets the counter (transient blips don't trip it).
- Non-goal: no persistence across restarts (a restart is the reset lever, and env fixes need
  one anyway).
- Non-goal: no automatic model switching (rejected: silent cross-provider calls — see the
  decision doc).

## Approach

1. `src-tauri/src/state.rs`: `pub rewrite_failures: AtomicU32` (SeqCst, same pattern as
   `ask_in_progress`), init in `new()`. `pub const REWRITE_BREAKER_LIMIT: u32 = 2;`
   Methods: `rewrite_breaker_tripped(&self) -> bool` and
   `record_rewrite_outcome(&self, got_candidates: bool)` — success stores 0; failure
   `fetch_add(1)`; on the exact crossing (`prev + 1 == LIMIT`) print ONE loud,
   non-trace-gated `eprintln!` naming the likely causes (reasoning-only model,
   misconfiguration, unreachable host) and the fixes (`WIKILENS_REWRITE_MODEL`, restart to
   re-enable).
2. `src-tauri/src/commands.rs` `rewrite_fut`: breaker-skip branch before the attempt
   (after the off-switch check); on `Ok(outcome)` call
   `record_rewrite_outcome(!outcome.queries.is_empty())`, on `Err` record `false`. Skip
   paths never touch the counter.
3. Debug-table integration (`WIKILENS_DEBUG`): a breaker skip records
   `report.phase_skipped("rewrite", "skipped (circuit breaker)")` and does **not** set
   rewrite usage — the tokens row shows `-/-` (call never made), consistent with the
   table's contract.
4. Tests in `state.rs` (4): fresh → not tripped; 2 failures → tripped;
   fail → success → fail → not tripped; tripped stays tripped.

## Decisions & trade-offs

- Two strikes (not one): a single failure can be a transient 5xx/timeout; two consecutive
  zero-candidate results on a path that should basically always parse is a config problem.
- Counting *empty parses* as failures (not just errors) is the point — the reasoning-model
  failure mode is a successful HTTP call with an unusable body.
- Per-session only: cheap, no state file, and the loud eprintln tells the user exactly how
  to fix the cause rather than the symptom.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#2, priority 2); depends on
  [[2026-07-10_retrieval-path-timeouts]] (bounds each strike at ≤4s).
