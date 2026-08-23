---
title: Retrieval eval suite — durable fixture, 3-round stability, answer-level judge
type: plan
status: done
created: 2026-08-22
updated: 2026-08-22
tags: [rag, wiki, llm, testing]
related:
  - "[[2026-08-17_core-pipeline-analysis]]"
  - "[[2026-07-07_retrieval-quality-improvements]]"
  - "[[2026-07-04_golden-query-retrieval-tests]]"
  - "[[2026-07-10_rewrite-circuit-breaker]]"
  - "[[2026-07-10_merge-raw-hit-guarantee]]"
  - "[[2026-08-22_retrieval-eval-round-2]]"
commit: [7364a07, be5efa5, bbd6421]
---

# Retrieval eval suite — durable fixture, 3-round stability, answer-level judge

## Context / problem
The Tier 0 measurement the core-pipeline analysis asked for
([[2026-08-17_core-pipeline-analysis]] §Tier 0) ran once on 2026-08-21 as a
40-question, single-round, scratchpad-only harness: 36/39 valid questions put
the right page in the top 4 (92.3%), the LLM rewrite carried 41 points of that
(raw-only 51.3%), zero zero-hits, one live R7 eviction, and the Sebastian
truncation case confirmed. Three things limited it: the question set lived in
a session scratchpad (gone with the session), 40 questions give ±9pp margins
(a change that loses one question reads as -2.5pp — noise), and hit@4 says
nothing about whether the final answer is true.

## Goal / non-goals
- Goal: a re-runnable suite in the repo (`eval/`) — a JSON fixture of ~200
  questions across the 15 reachable built-in/user wikis, a Node mirror of the
  retrieval phase with per-leg counterfactuals (raw-only / rewrite-only /
  merge), 3 rounds per run to measure rewrite non-determinism, and an
  answer-level judge on a ~50-question subset (expected fact supplied; judge
  labels correct / partial / wrong / abstain, plus a programmatic grounding
  check against the context actually sent).
- Goal: offline parity tests so the mirror can't silently drift from the Rust
  functions it copies (`merge_hits`, `preprocess_query`, `simplify_query`,
  `html::to_plaintext` against the insta snapshots).
- Non-goal: changing the pipeline. This measures; Tier 1–3 changes are gated
  on its numbers and land separately.
- Non-goal: mirroring the wikitext fallback (recorded as non-mirrored in the
  README; a rare path). The title-index rung was planned as a non-goal too,
  then mirrored (`titles.mjs`) once the dry run produced zero-hits.

## Approach
1. Branch `test/retrieval-eval-suite`; `eval/` at the repo root: `questions.json`
   (fixture: wikis with engine family + `builtin` flag; questions with id, game,
   style, source ∈ history|hand|synthetic, gold[], optional trunc[]/fact),
   `lib.mjs` + `html.mjs` (mirrors), `*.test.mjs` (parity, run by
   `npm run test:node`), `run-retrieval.mjs` (rounds, JSONL to the
   gitignored `eval/out/`), `run-answer.mjs`, `gen-synthetic.mjs`,
   `aggregate.mjs`, `README.md`. Rust offline test pins the fixture's
   integrity against the game registry.
2. Grow the set: keep the 40 (ids stable), add hand-written questions per wiki
   in player voice (subagent drafts for the lesser-known wikis, gold verified
   via `redirects=1`), plus a synthetic fraction generated from page content
   with forced styles and programmatic typos — flagged `synthetic` and read
   separately. Shared wikis constrained (Grounded 2 suffix, UESP ns 134,
   Fallout 4 relevance).
3. Run 3 rounds; retry transient errors once at end-of-round; aggregate
   overall / per-leg / per-wiki / per-engine / per-style / per-source /
   stability.
4. Answer subset: ~40 retrieval hits + ~10 deliberate hard cases
   (truncation flagship, known misses) to measure abstention vs.
   hallucination. Answer and judge both on the Default-mode model the user
   ships (Haiku 4.5).
5. Results: research doc in `vault/research/`, pt-BR visual report at the
   repo root (local-only, like the first edition), README/CLAUDE.md pointers.

## Decisions & trade-offs
- Node mirror over a Rust `--ignored` test: the harness needs per-leg
  counterfactuals, LLM judging, and JSONL/aggregation — awkward in cargo
  tests — and the mirror is pinned by parity tests against the Rust vectors.
- Synthetic questions are kept but quarantined (reported separately): they
  inflate hit rates; the hand/history buckets are the headline.
- Judge stays on the weak shipping model, with the expected fact supplied so
  the task is contain/contradict, not open grading.

## Status log
- 2026-08-22 — created; harness ported from the 2026-08-21 scratchpad run.
- 2026-08-22 — harness + 40-question fixture committed (7364a07); branch stacked on PR #76 (merge commit) so the research wikilinks resolve.
- 2026-08-22 — fixture grown to 221 (subagent drafts for 8 wikis + curated synthetic batch), title index mirrored, 87 facts verified; Grounded 2 golds corrected (be5efa5).
- 2026-08-22 — 3 rounds run (`eval/out/r2`), answer eval on 87 facts, merge-policy replay round; aggregator + replay tool (bbd6421). Results: [[2026-08-22_retrieval-eval-round-2]]. Done.
