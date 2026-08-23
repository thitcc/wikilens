---
title: Retrieval eval round 2 — 3 rounds over the grown fixture, answer-level judge
type: research
status: active
created: 2026-08-22
updated: 2026-08-22
tags: [rag, wiki, llm, testing]
related:
  - "[[2026-08-22_retrieval-eval-suite]]"
  - "[[2026-08-17_core-pipeline-analysis]]"
  - "[[2026-07-10_rewrite-circuit-breaker]]"
  - "[[2026-07-10_merge-raw-hit-guarantee]]"
  - "[[2026-07-15_rewrite-bare-entity-candidate]]"
  - "[[2026-07-04_golden-query-retrieval-tests]]"
commit:
---

# Retrieval eval round 2 — 3 rounds over the grown fixture, answer-level judge

_Results placeholder — filled when the runs complete. Method and scope are
final; every number below is replaced by the run's `summary.json`._

## Question

Round 1 (2026-08-21, 40 questions, one round, scratchpad harness) said the
retrieval phase puts the right page in the top 4 for 92% of questions and that
the LLM rewrite carries ~41 points of that. What survives a bigger, stratified
set, three rounds (rewrite non-determinism), and an answer-level check?

## Method

- **Fixture** `eval/questions.json` (`eval/README.md`): TODO questions across
  15 wikis (gw2 is WAF-blocked from the author's network) — history (verbatim
  `history.json`), hand (player-voice, gold verified live), synthetic
  (generated from page content; quarantined in every headline figure).
  Styles: entity / stat / howto / negation / compare / typo / control.
- **Mirror**, parity-tested against the crate (`npm run test:node`):
  `preprocess_query`, `merge_hits`, the rewrite request + parser,
  `html::to_plaintext` (byte-equal on the insta snapshots), the zero-hit
  ladder incl. the title index, `fetch_rendered_page` + truncation,
  `build_user_message` + the answer request.
- **Runs**: 3 rounds, same questions, Default-mode model from `.env`
  (rewrite / answer / judge all Haiku 4.5 — what the user ships); ≥300 ms
  pacing; raw-search errors retried once per round.
- **Per-leg counterfactuals** per question: raw-only, rewrite-only, merge.
- **Answer subset**: every question with a verified `fact` (evidence string
  literally on the gold page); judge = same model, constrained to
  contain/contradict against the supplied fact; grounding is programmatic
  (evidence in the truncated context sent? beyond the 8000-char cap?).

## Results

TODO — overall, per leg, per source, per style, per engine family, stability,
skip-gate counterfactual, answer subset, timings.

## Findings

TODO

## What this changes in the roadmap

TODO

## Limitations

- One network, one model family, three rounds; synthetic questions are
  reported separately because the generator saw the page.
- The wikitext fallback is not mirrored (failed parses are recorded as
  `degraded`), and the answer call is non-streaming.
- Hit@4 says the page reached the model; only the answer subset says whether
  the fact did.
