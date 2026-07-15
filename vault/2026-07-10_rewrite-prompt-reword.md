---
title: Reword the rewrite system prompt — drop the false zero-hit premise, feed the consensus signal
type: plan
status: done
created: 2026-07-10
updated: 2026-07-14
tags: [llm, rag]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_merge-raw-hit-guarantee]]", "[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_reasoning-skip-and-capability-tags]]"]
commit: e4f88c9638c6244daeeaeff94f62e582d3da673c
---

# Reword the rewrite system prompt — drop the false zero-hit premise, feed the consensus signal

## Context / problem

`REWRITE_SYSTEM_PROMPT` (llm.rs) still opens with "Their own search returned nothing" — a
premise that has been false since the lazy→eager escalation (the rewrite now runs on **every**
ask, concurrently with a raw search that usually has hits). Consequences:

- The false premise pushes the model to *replace* the query rather than *complement* it —
  discouraging it from repeating the question's correct proper nouns, which suppresses the
  **consensus** signal (`merge_hits` ranks in-both hits first, its strongest evidence).
- It asks for "1 to 3" queries, but only `REWRITE_SEARCH_LIMIT = 2` are searched — the third
  is silently dropped.
- **Live evidence (2026-07-10):** for "what gifts does Abigail likes?" the candidates were
  keyword soup — "Abigail likes loves gifts" | "Abigail preferences gifts" | "Abigail gift
  guide" — never the entity "Abigail" itself, and the third was dropped by the limit.

Several doc comments carry the same stale premise.

## Goal / non-goals

- Goal: the prompt states the real task — the raw keyword search runs in parallel; add what
  it would miss (typos, paraphrases, the wiki's own name for the described thing); KEEP the
  question's correct proper nouns (repeating a right term helps confirm the matching page —
  it feeds consensus); produce **1 or 2** queries.
- Goal: stale comments corrected in the same pass.
- Non-goal: no format change — still the strict `{"queries":[...]}` JSON contract
  (`parse_rewrite_queries` untouched).
- Non-goal: no new prompt-engineering experiments beyond the premise/count/nouns fixes —
  measure first, iterate later.

## Approach

1. `src-tauri/src/llm.rs`: rewrite `REWRITE_SYSTEM_PROMPT` per the goal above (keep the
   compact-JSON-only instruction and the "best guess first, exact proper nouns preferred"
   guidance).
2. Stale comment sweep: `rewrite_query` doc ("Only invoked on the zero-hit dead-end" — now
   eager), `REWRITE_MAX_TOKENS` doc (mention the reasoning-skip once
   [[2026-07-10_reasoning-skip-and-capability-tags]] lands), `wiki/titles.rs` module doc
   ("On a zero-hit search" premise), and the `commands.rs` rewrite-model comment (note the
   deliberate same-model default + privacy rationale).
3. No test changes expected: prompt text isn't asserted; parser tests are input-driven.

## Decisions & trade-offs

- "1 or 2" instead of raising the search limit: two candidate searches is the latency/cost
  budget ([[2026-07-10_concurrent-candidate-searches-and-status]] makes them concurrent);
  asking for what we won't use invites waste.
- Keep-own-nouns is the consensus lever: a candidate repeating "Abigail" makes her page an
  in-both hit that `merge_hits` ranks first — attacking the same live failure
  [[2026-07-10_merge-raw-hit-guarantee]] closes from the merge side.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#7, MEDIUM), with live
  candidate-quality evidence from the first debug table.
- 2026-07-14 — executed. New prompt states the parallel raw search, keeps correct
  proper nouns, asks for 1 or 2 queries; stale comments swept (`rewrite_query`,
  `REWRITE_MAX_TOKENS` — the reasoning-skip landed in the meantime, `wiki/titles.rs`,
  `stage_enabled`, rewrite-model default + privacy rationale in commands.rs).
  Live check (Haiku, direct API call with the exact new prompt): the Abigail case
  now yields exactly 2 candidates, both keeping "Abigail"; a typo case ("arcane
  persitance") yields the corrected title first. Old prompt as baseline still
  produced 3 candidates (third dropped by the limit).
