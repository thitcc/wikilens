---
title: Post-merge dead-code review and cleanup
type: plan
status: done
created: 2026-07-14
updated: 2026-07-14
tags: [rust, frontend]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_wiki-fetch-hardening]]"]
commit: 967cfc4
---

# Post-merge dead-code review and cleanup

## Context / problem
Three merges landed in quick succession — the testing-workflow branch and the
WIKILENS_DEBUG instrumentation into `main`, then `main` into
`fix/wiki-fetch-hardening` (all in `main` at `2f0b7e7`). Each was resolved as a
union of both sides, which can strand superseded code. A review of the primary
ask workflow plus a systematic audit (five finders over disjoint slices —
Rust LLM path, Rust wiki/http, Rust test support, frontend, CSS — with an
adversarial verifier per candidate) swept the blind spots the gates can't see:
clippy `-D warnings` misses unused **pub** items in a lib crate, and tsc misses
unused type exports and dead CSS.

**Verdict: the merges are clean.** Nine candidates, two confirmed dead — both
scaffold-era (`344eac3`), not merge residue — and one genuine merge-kept
duplication.

## Goal / non-goals
- Goal: remove the two confirmed-dead items and dedupe the merge-kept test
  helper; record the refuted candidates so they aren't re-flagged by a future
  audit.
- Non-goal: any behavior change — every gate stays green untouched.
- Non-goal: removing reserved API surface (see the keep-list below).

## Approach
1. `Serialize` derive on `WikiPage` (`src-tauri/src/wiki/fetch.rs`) — nothing
   ever serializes it: `run_ask` maps pages into `commands::Source` before IPC,
   the prompt builder reads fields as strings, insta assertions are string
   snapshots. Drop the derive and the now-orphaned `use serde::Serialize;`.
2. `StreamEvent` type (`src/types.ts`) — exported since the scaffold, never
   imported by any commit; the event plumbing types `ask://status` /
   `ask://delta` separately in `src/api.ts`. Delete the type + its JSDoc;
   `AskStatus` stays (imported by `api.ts` and `App.tsx`).
3. `tests::page` in `src-tauri/src/llm.rs` — byte-identical duplicate of
   `test_support::wiki_page`; the union merge kept the scaffold-era local copy
   while the same file's `http_tests` mod already imports the shared helper.
   Replace the local fn with `use crate::test_support::wiki_page as page;` so
   all six call sites compile unchanged.

**Deliberately kept** (audited, refuted as dead — do not remove):
- `impl Default for AppState` (`state.rs`): satisfies clippy's
  `new_without_default` under `-D warnings`.
- `MODELS` / `CANDIDATES` exports (`src/test/backend.ts`): the consts are the
  live default IPC handlers; exporting every fixture is the module's uniform
  test-support convention.
- `Badge` `variant="accent"` arm + `.badge--accent` CSS: documented reserved
  extension point ([[2026-07-06_model-vision-badges]]) with a planned consumer
  in [[2026-07-10_reasoning-skip-and-capability-tags]].

## Decisions & trade-offs
The `tests::page` dedupe is a refactor, not a deletion — removing it outright
would break six call sites that pin the prompt-fencing invariants. Riding it in
this plan (rather than leaving it) because it is the only true merge residue
the audit found, and the fix is one line.

## Status log
- 2026-07-14 — created from the post-merge dead-code review; audit complete,
  three items in scope, executing now.
- 2026-07-14 — **done.** All three items landed in `967cfc4` (net −18 lines):
  derive + orphaned import gone from `fetch.rs`, `StreamEvent` gone from
  `types.ts`, `tests::page` now an aliased `test_support::wiki_page` import.
  Gates green: tsc clean, Vitest 17/17, clippy `-D warnings` clean, cargo test
  206/206. Zero behavior change.
