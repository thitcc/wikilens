---
title: Merge search hits by genuine consensus + round-robin, repealing the raw[0] reserved slot
type: decision
status: done
created: 2026-08-23
updated: 2026-08-23
tags: [rag, wiki, rust]
related:
  - "[[2026-08-23_port-gc-rr-merge-policy]]"
  - "[[2026-08-22_retrieval-eval-round-2]]"
  - "[[2026-07-10_merge-raw-hit-guarantee]]"
  - "[[2026-07-07_llm-query-rewrite-in-retrieval]]"
  - "[[2026-07-15_rewrite-bare-entity-candidate]]"
commit: e42c04e
---

# Merge search hits by genuine consensus + round-robin, repealing the raw[0] reserved slot

## Context

`merge_hits` ranked consensus (a title in both the raw and rewrite hit lists)
first, then rewrite hits in candidate order, with one slot reserved for the
raw search's top hit ([[2026-07-10_merge-raw-hit-guarantee]]). The retrieval
eval round 2 measured that rule at scale: 33% of misses were evictions of a
page already in hand. Two mechanisms, verified on records: *pseudo-consensus*
— the second rewrite candidate often paraphrases the question, returns the raw
search's own titles, and that echo fills the cap as "agreement" ahead of the
entity candidate's real hit; and *second-candidate starvation* — concatenated
candidate hits let a generic first candidate use every rewrite seat. The
merge-policy replay recombined the same recorded searches under ten rules to
compare them deterministically, outside the ~2.5-point live noise floor.

## Decision

Consensus counts only candidates whose hit list is not a near-copy of the raw
list (fewer than `CONSENSUS_COPY_THRESHOLD = 3` shared titles,
case-insensitive); genuine-consensus titles seat first in raw-canonical
casing, and the remaining seats round-robin over (cand1, cand2, raw)
index-major — replacing the reserved-slot rule.

## Consequences

- Good: best replay score of the ten policies on both datasets — 87.3%
  (+7/−1) on the 221 per-candidate records, 89.8% (+28/−0) on the 561
  unambiguous three-round records, vs 84.6% / 84.8% for the old rule. Both
  eviction mechanisms die: an echo candidate can't manufacture consensus, and
  every list seats its best hits before any list floods the cap. Genuine
  agreements the reserved-slot variants dropped (S9 Mermaid's Pendant,
  Ypalword-2 Junkyard) survive.
- Cost / bad: **the raw[0] inclusion guarantee is repealed** — raw takes its
  turn at round-robin index 0 instead of holding a reserved seat, so a large
  genuine consensus plus candidate tops can seat four titles without raw's
  first hit (pinned by `raw_top_hit_is_no_longer_guaranteed_a_seat`). The
  structural safety net is replaced by the eval cadence (measure before/after
  any retrieval change). A candidate's third-rank hit can lose its seat to
  the round-robin — the one replay loss (S15 "Gold").
- Follow-ups: live 3-round validation + answer-level judge per
  [[2026-08-23_port-gc-rr-merge-policy]]; the replay keeps the old rule as
  `old-production` and the `gc-rr` policy as a +0/−0 identity tripwire.

## Alternatives considered

- Keep production (reserved slot) — measured at 84.6% / 84.8%; leaves the
  33%-of-misses eviction class in place.
- entity-first (reserve cand1[0], then production) — +2/−0 and +4/−0, never
  loses but recovers only a fraction of the evictions.
- entity+raw-first (reserve cand1[0] + raw[0], round-robin the rest) — +7/−3
  and +26/−2; its losses were genuine consensus discarded (S9, Ypalword-2),
  which gc-rr keeps.
- interleave (pure round-robin raw/cand1/cand2, no consensus) — +9/−5 and
  +30/−3; drops rewrite-only hits such as "Abigail gifts" because raw junk
  takes seats with no consensus signal to counter it.
- consensus-guard (consensus counted only against cand1) — +3/−2 and +8/−5;
  blocks the echo but keeps concatenation, so starvation stays.
- Copy threshold ≥2 instead of ≥3 — tied ≥3 exactly on both datasets; 3
  chosen because it excludes less genuine two-title agreement.
