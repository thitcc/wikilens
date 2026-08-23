---
title: Port the gc-rr merge policy into merge_hits
type: plan
status: done
created: 2026-08-23
updated: 2026-08-23
tags: [rag, wiki, rust, testing]
related:
  - "[[2026-08-23_merge-genuine-consensus-round-robin]]"
  - "[[2026-08-22_retrieval-eval-round-2]]"
  - "[[2026-08-22_retrieval-eval-suite]]"
  - "[[2026-07-10_merge-raw-hit-guarantee]]"
  - "[[2026-07-15_rewrite-bare-entity-candidate]]"
  - "[[2026-07-07_llm-query-rewrite-in-retrieval]]"
commit: [e42c04e, 65a3430]
---

# Port the gc-rr merge policy into merge_hits

## Context / problem

The retrieval eval round 2 found that 33% of retrieval misses (35 of 106
question-rounds) are `merge_hits` evicting a page one of the search legs
already had — two verified mechanisms: *pseudo-consensus* (a paraphrase
candidate echoes the raw search's own titles and the consensus step promotes
them past the entity candidate's real hit) and *second-candidate starvation*
(candidate hits are concatenated, so a generic first candidate fills the cap
before the second gets a turn). The merge-policy replay recombined the same
recorded searches under alternative rules; **gc-rr** (genuine consensus +
round-robin) beat all ten policies on both datasets: 87.3% (+7/−1) on the 221
per-candidate records, 89.8% (+28/−0) on the 561 three-round records, vs
production 84.6% / 84.8%. The research doc mandates porting it and diffing the
per-question id lists, not the headline.

## Goal / non-goals

- Goal: `merge_hits` implements gc-rr — consensus counts only candidates that
  are not near-copies of the raw list (< 3 shared titles), then the open seats
  round-robin over (cand1, cand2, raw); the Node mirror, its parity vectors,
  and the eval runners move in lockstep in one commit.
- Goal: offline gate before any live run — replaying the recorded baselines
  with the new production must reproduce the gc-rr numbers exactly, and the
  registered `gc-rr` policy must show +0/−0 (identity tripwire).
- Goal: live validation per the eval cadence — 3 fresh rounds + answer-level
  judge, diffed per question id against the r2 baseline.
- Non-goal: no copy-threshold tuning beyond 3 (≥2 tied ≥3 exactly on the
  recorded data; 3 excludes less genuine agreement).
- Non-goal: no rewrite-prompt change, no `REWRITE_SEARCH_LIMIT` change, no
  zero-hit-ladder change.

## Approach

1. Vault docs first: this plan + the decision doc
   [[2026-08-23_merge-genuine-consensus-round-robin]] (the raw[0] reserved
   slot is repealed — a real fork with rejected alternatives).
2. One atomic parity commit across `src-tauri/src/commands.rs`
   (`CONSENSUS_COPY_THRESHOLD`, new `merge_hits(raw, candidates:
   &[Vec<String>], limit)`, call-site restructure keeping join_all order and
   the `"{n} queries -> {n} hits"` phase format, rewritten `merge_tests`),
   `eval/lib.mjs` (mirror + exported constant), `eval/lib.test.mjs` (same
   vectors), `eval/run-retrieval.mjs` (per-candidate call), and
   `eval/merge-policies.mjs` (`production` = new rule, legacy body kept as
   `old-production`, `entity-first` recomposed on it, `gc-rr` kept as
   identity check).
3. Offline gate: replay `eval/out/r2policy` and `eval/out/r2` — expect
   production 87.3% / 89.8%, `gc-rr` +0/−0, `old-production` the exact
   inverse gain/loss id lists. Mismatch = mirror bug; fix before going live.
4. Docs sweep: `docs/ai-workflow.html` merge paragraph, `eval/README.md`
   (`old-production` row), `llm.rs` consensus comment.
5. Live: `npm run eval:retrieval -- --out eval/out/r3-gcrr --rounds 3` →
   aggregate → per-id diff vs r2; `npm run eval:answer` on the new run.
   Abort criteria: offline mismatch; >~3 of r2's 174 always-hit ids flip to
   miss without a candidate-list explanation; headline < 84.0%; answer
   correct-rate on the hit subset regresses beyond noise.
6. `/check`, close the vault docs, PR per the delivery conventions
   (assignee @me, human merges with a merge commit).

## Decisions & trade-offs

The fork (reserved slots vs genuine consensus + round-robin, threshold
choice, the repealed raw[0] guarantee and the S15-class third-rank loss)
lives in [[2026-08-23_merge-genuine-consensus-round-robin]].

## Status log
- 2026-08-23 — created.
- 2026-08-23 — ported (e42c04e): the 5-file parity unit landed in one commit;
  11 Rust merge tests + 11 Node vectors, /check green (94 Node, 358 cargo).
- 2026-08-23 — offline gate passed exactly: production 87.3% / 89.8%, gc-rr
  +0/−0 on both baselines, old-production the predicted inverse id lists.
- 2026-08-23 — done. Live 3-round validation: 88.1% (196/193/195) vs the 84.0%
  r2 baseline; per-id 23 up / 6 down; 14 of 16 replay-predicted ids improved
  (E1, Ywarframe-2 stayed — candidate luck; C7 only to 1/3); the 3 always-hit
  regressions (W10, O2, G11) re-checked against records: the old rule misses
  those rounds too. Stability 174/26/21 → 186/20/15; evictions per round ~12 → 5–6.
  Answers (86 judged; S6 zero-hit that round, P14 hand re-judged partial after
  an unparseable judge reply): 54c/23p/6w/3a vs 57/21/7/2 — correct-on-hit
  69.7% vs 73.0%, within judge noise; G8 wrong→correct and A4 partial→correct
  are retrieval-driven.
