---
title: Document the retrieval env vars and fix the stale DeepSeek default in README/.env.example
type: plan
status: done
created: 2026-07-10
updated: 2026-07-14
tags: [llm]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_reasoning-skip-and-capability-tags]]", "[[2026-07-10_rewrite-circuit-breaker]]"]
commit: 2620a36
---

# Document the retrieval env vars and fix the stale DeepSeek default in README/.env.example

## Context / problem

Five retrieval env vars — `WIKILENS_QUERY_REWRITE`, `WIKILENS_REWRITE_MODEL`,
`WIKILENS_REWRITE_PROVIDER`, `WIKILENS_TITLE_INDEX`, `WIKILENS_TRACE_RETRIEVAL` — are the
escape hatches for the whole rewrite feature, but they're documented only in the orphaned
`docs/ai-workflow.html`. Worse, README:38 and `.env.example` still name the DeepSeek default
as `deepseek-chat`, while the code default is `deepseek-v4-flash` (providers.rs — the
`deepseek-chat` alias retires 2026-07-24), and README:73–74's model-drift example is stale.
A user following the README configures a model that's about to disappear and has no way to
discover the rewrite controls. (`WIKILENS_DEBUG` is already documented — this plan covers
the rest.)

## Goal / non-goals

- Goal: README carries a "Retrieval tuning (advanced)" table for the five vars; the
  `WIKILENS_REWRITE_PROVIDER` row explicitly warns: **"your question text is then sent to
  that second vendor as well"** (the privacy rationale behind the same-model default).
- Goal: stale `deepseek-chat` references fixed everywhere; drift example reworded.
- Non-goal: `docs/ai-workflow.html` stays untouched (orphaned; not worth maintaining two
  copies).
- Non-goal: no code changes.

## Approach

1. `README.md`: fix the default id at line ~38; reword the drift example (~73–74, e.g.
   retired alias → current id); add the "Retrieval tuning (advanced)" env-var table (var,
   default, effect) with the second-vendor sentence on the `WIKILENS_REWRITE_PROVIDER` row;
   if [[2026-07-10_reasoning-skip-and-capability-tags]] /
   [[2026-07-10_rewrite-circuit-breaker]] have shipped by then, add one sentence each on
   the automatic reasoning-skip and the session breaker; add *(Understanding)* to the
   smoke-test status flow if [[2026-07-10_concurrent-candidate-searches-and-status]] has
   shipped.
2. `.env.example`: fix `WIKILENS_DEEPSEEK_MODEL=deepseek-chat` → `deepseek-v4-flash`; add a
   commented "Retrieval tuning (optional)" block mirroring the README table, next to the
   existing `WIKILENS_DEBUG` entry.
3. `CLAUDE.md` §4: a pointer from the conventions to the README table (avoid duplicating
   the var list in three places).

## Decisions & trade-offs

- Sequence-flexible by design: the table documents whatever behavior exists at execution
  time — running this last (after the behavior plans) writes it once, correctly.
- README as the single source for user-facing env docs; `.env.example` mirrors as comments
  (copy-paste surface), CLAUDE.md only points.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#6, MEDIUM); deliberately last in
  the execution order so it documents shipped behavior.
- 2026-07-14 — done. README gained the "Retrieval tuning (advanced)" five-var table (with
  the second-vendor warning on the `WIKILENS_REWRITE_PROVIDER` row) plus one sentence each
  on the shipped reasoning-skip and session circuit breaker; `.env.example` mirrors the
  table as a commented block and both stale `deepseek-chat` defaults now read
  `deepseek-v4-flash`; CLAUDE.md §4 points at the README table. The conditional
  *(Understanding)* smoke-test edit was skipped —
  [[2026-07-10_concurrent-candidate-searches-and-status]] hasn't shipped.
