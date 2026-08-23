---
title: Retrieval eval round 2 — 221 questions × 3 rounds, answer-level judge
type: research
status: done
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
  - "[[2026-07-04_rendered-html-extraction]]"
commit: [7364a07, be5efa5, bbd6421]
---

# Retrieval eval round 2 — 221 questions × 3 rounds, answer-level judge

## Question

Round 1 (2026-08-21: 40 questions, one round, a scratchpad harness) said the
retrieval phase puts the right page in the top 4 for 92% of questions and
that the LLM rewrite carries ~41 points of that. What survives a bigger,
stratified set, three rounds (the rewrite is non-deterministic), and an
answer-level check? The suite itself (`eval/`, `eval/README.md`) is the
durable deliverable; this doc records what it measured on 2026-08-22.

## Method

- **Fixture** `eval/questions.json`: 221 questions across 15 wikis (gw2 is
  WAF-blocked from the author's network) — 16 verbatim from the real
  `history.json`, 145 hand-written in player voice (gold verified live,
  alternates allowed), 60 synthetic (generated from sampled page content by
  the Default-mode model, curated, flagged and reported separately). Styles:
  entity 32 · stat 52 · howto 60 · negation 30 · compare 25 · typo 20 ·
  control 2. Engine families: CirrusSearch 9 wikis, Fandom UnifiedSearch 5,
  MediaWiki default (Stardew) 1.
- **Mirror** (`lib.mjs`, `html.mjs`, `titles.mjs`), parity-tested against the
  crate's own vectors and insta snapshots (`npm run test:node`): preprocess,
  `merge_hits`, rewrite request + parser, the zero-hit ladder including the
  title index, `fetch_rendered_page` + 8000-char truncation,
  `build_user_message` + the answer request. Not mirrored: the wikitext
  fallback (2 degraded fetches in 87 were skipped) and streaming.
- **Runs**: 3 rounds over all 221 (run `eval/out/r2`), ≥300 ms pacing,
  raw-search errors retried once per round (0 occurred; 1 rewrite timeout
  in 663). Per question and round, three legs are judged against the same
  gold: raw-only (gold in the raw top 4), rewrite-only (gold in any
  candidate hit), and the real merge (gold rank). Titles redirect-resolved
  before judging.
- **Answer subset**: the 87 questions with a verified `fact` (evidence string
  literally on the gold page, `check-facts.mjs`; 9 of them deliberately
  beyond the 8000-char cap or on known misses). The answer phase ran with
  the production system prompt and cap on the round-1 merged titles; the
  judge is the same Haiku 4.5 given the expected fact (contain/contradict);
  grounding is programmatic (evidence in the truncated context sent? beyond
  the cap?) plus a "declared gap" regex on the answer text.
- **Merge-policy replay** (`merge-policies.mjs`): one extra round recorded
  per-candidate hits; the same searches are recombined under alternative
  merge rules and scored against the same gold.
- Cost: under US$2 of Haiku 4.5 for 4 rounds + answers + judge + synthetic
  drafting; ~45 min wall-clock per 3-round run.

## Results

**Hit@4 (the page is in the 4 given to the model).** 557 / 663 question-rounds
= **84.0%** (rounds: 84.2 · 85.1 · 82.8). Per leg, averaged: raw-only
**49.0%**, rewrite-only **83.0%**, merge **84.0%**. The rewrite rescued 243
hits the raw search lacked; the raw search saved 32 the rewrite lacked.
Rank of the gold when hit (round 1): 67% at rank 0, 84% in the top 2.

**Misses (106 question-rounds):** wrong-but-nonzero 104, zero-hit 2. In **35
of the 106 (33%)** one leg had the gold page and the merge dropped it (raw
had it in 11, the rewrite in 25) — the R7 class, an order of magnitude
bigger than round 1's 1-in-39. The zero-hit ladder fired 5 times in 663 and
rescued once (title index: "shock bangstik" → Shock Bangstick); "find robus"
reached the title index twice and didn't match (the index scores the whole
query, not the entity).

**By source:** hand 86.0% (374/435) · history 83.3% (40/48) · synthetic
79.4% (143/180) — the synthetic bucket is *harder*, not easier (the
generator targets details of obscure pages).
**By style:** typo 93.3% (raw 25% / rewrite 91.7%) · compare 93.3% · entity
88.5% · howto 83.3% · negation 80.0% · stat 75.6%.
**By engine:** CirrusSearch 85.8% (raw 36.6%) · Fandom UnifiedSearch 83.1%
(raw **81.1%**, rewrite 85.1%) · Stardew default engine 75.0% (raw 25%,
rewrite-only 65%). The rewrite's value is engine-dependent: ~2 points on
Fandom, ~49 on Cirrus, ~50 on Stardew.
**By game (3-round hit):** terraria 100 · conanexiles 100 · grounded 92.9 ·
skyrim 91.7 · fallout4 90.5 · poe 90.5 · minecraft 86.3 · warframe 86.3 ·
corekeeper 83.3 · abioticfactor 80.0 · stardew 75.0 · palword 74.1 ·
davethediver 73.3 · poe2 72.2 · grounded2 71.4.

**Stability (same question, 3 rounds):** 174 always hit · 26 never · 21
flaky (9.5%). The rewrite produced a different candidate list in 177/221
questions (80%); the final 4-title set differed in 110; the verdict flipped
in 21. Practical noise floor of the suite: ~2.5 points between rounds.
Three of round 1's history "wins" (robus, normal horns, Abigail gifts)
are flaky or never-hit here — round 1 was one good round.

**Skip-the-rewrite gate (Tier 1 #7 counterfactual):** 75 questions have a
≤2-word preprocessed query; 24–26 of them per round hit *only* through the
rewrite ("Abigail gift", "find obsidian", "find robus", "beacon pyramide",
"find chillet"…). As specified, the gate loses a third of the asks it
covers; the median saving would be ~0.9 s.

**Answer level (87 judged):** correct 57 (65.5%) · partial 21 · wrong 7 ·
abstain 2. Conditional on retrieval: hit (74) → correct 54 (73%), partial
17, wrong 2, abstain 1 — and the three non-partial failures are exactly the
beyond-cap facts (S5 Sebastian, X4 Thrall, A9 Flathill); miss (13) → correct
3 (another page in the pack carried the fact — one of them an April-fools
page), partial 4, wrong 5, abstain 1. Of the 7 wrong, 4 declare the gap in
the answer text (honest, useless); the 3 confidently wrong are P3 (Palworld
"horns": presents one pal as the list), G3 (Grounded 2 "ant mount": answers
with the Grounded 1 trophy recipe — wrong-game page), A9 (beyond cap). The
5 beyond-cap facts yielded **0 correct** (1 honest abstain, 2 partial, 2
wrong). No answer hit `max_tokens` (median 148 output tokens; context
median 18,500 chars, p90 31,000). The 3 "correct without the evidence in
context" were hand-checked: all grounded in another page of the pack — no
from-memory correct answer detected.

**Timings (round 1 medians / p90):** raw search 400 / 633 ms · rewrite 906 /
1152 ms · candidate searches 426 / 635 ms · page fetch (≤4, sequential,
incl. the harness' 300 ms pauses) 2924 / 5822 ms · answer (non-streaming)
2624 / 4584 ms.

## Findings

1. **Merge ordering is the largest avoidable failure class (R7, quantified).**
   33% of misses had the gold page in hand. Two mechanisms, both verified
   on records: *pseudo-consensus* — the second rewrite candidate is often a
   paraphrase of the question and returns the raw search's own titles,
   which the consensus step then promotes to fill the cap ahead of the
   bare-entity candidate's real hit (S4 Prismatic Shard, C2, F10, E1 "the
   hooded one" — raw rank 2 in all 3 rounds, dropped in all 3); and
   *second-candidate starvation* — candidate hits are concatenated in
   order, so a generic first candidate that returns 4 titles leaves no slot
   for the second ("Greybeards leader" vs "Paarthurnax"). `merge-policies.mjs`
   replays the recorded searches under alternative rules: reserving a
   slot for the entity candidate's top hit never loses (+2/−0; +4/−0 over
   three rounds); reserving entity + raw top hits and round-robining the
   rest recovers most evictions (+7/−3 on the per-candidate round, +26/−2
   on the flood cases) — table in the addendum.
2. **The rewrite is still the column, unevenly.** 83% → 49% without it;
   it fixes 92% of typos (the title index almost never gets to play). But
   on Fandom's UnifiedSearch the raw search already scores 81% and the
   rewrite adds 2 points for ~0.9 s — the R16 breaker is the single
   biggest availability risk, and a per-engine "raw is enough" mode is
   worth measuring.
3. **Truncation (R10) is the one systematic gap between the right page and
   the right answer.** 5/5 beyond-cap facts failed; everything else
   between hit and correct answer is partiality (incomplete list, an
   extra detail).
4. **Shared wikis produce the dangerous class.** The only confidently wrong
   answers came from wrong-game pages: Grounded 2 questions served Grounded
   1 wall-trophy pages ("ant eggs don't make ant mounts — you need
   mandibles"), and a Fallout 4 Fat Man question got four Fat Man pages
   from four games. Round 1 had scored G3 as a hit and G4 as its R7
   flagship on those trophy pages — corrected here (gold re-pointed to the
   Grounded 2 buggy pages; both are now never-hit: the player says
   "mount", the wiki says "buggy", and the rewrite doesn't know the
   sequel renamed them).
5. **Reducer losses, three kinds, all cheap to fix and snapshot-testable:**
   icon-only tables lose their data because `<img>` is dropped with its
   `alt` (Abiotic Factor recipes reduce to "1 1 2 Crafting Bench 1";
   Palworld work suitability to "Lv 0 / Lv 2"); `display:none` spans are
   kept (Stardew infobox `data-sort-value="2000 ">` text); minecraft.wiki's
   infobox JSON blobs are kept and eat the 8000-char budget (Iron Ore's
   useful sentence sits at char ~2,500 after ~2,000 chars of JSON).
6. **The Tier 1 #7 skip gate is refuted as specified** (a third of ≤2-word
   asks lost); a safer criterion (e.g. skip only when the raw top-1 is an
   exact title match) can be evaluated on this suite before it exists.
7. **Noise floor.** 80% of questions get different rewrite candidates
   between rounds, 9.5% flip outcome; rounds spread 2.3 points. Any Tier
   1–3 change must be measured over 3 rounds and compared per question.

## What this changes in the roadmap

- **New Tier 1 item, first in line:** fix `merge_hits` ordering — reserve a
  slot for the bare-entity candidate's top hit (free win), and evaluate the
  two-slot + round-robin rule per question before adopting it; small diff,
  offline-testable against the recorded searches, recovers up to a third
  of current misses.
- Keep **R16 (protect the rewrite)** as the top availability item; add a
  per-engine measurement before any "skip the rewrite" heuristic; **drop
  Tier 1 #7 as written**.
- **Tier 2 section-aware assembly (R10)** now has 5 proof cases (0/5).
- **New Tier 1/2 item:** a wrong-game guard for shared wikis (Grounded 2
  suffix/categories, Nukapedia, UESP namespace already handled).
- **New Tier 1 item:** reducer fixes (keep `alt`, drop `display:none`, drop
  mcwiki JSON blobs), each pinned by a snapshot.
- **Tier 0 is done:** the suite is re-runnable (`npm run eval:*`); this doc
  is the baseline to beat.

## Limitations

- One network, one day, one model family (Haiku 4.5 as rewriter, answerer
  and judge — what Default mode ships). Custom-mode models may rewrite
  differently.
- Hit@4 is retrieval; the answer subset is 87 questions and the judge is a
  model (given the expected fact, constrained to contain/contradict; the
  wrong/partial labels were read one by one).
- The "fact in context" check looks for the literal evidence string — it
  reports "absent" when another page carries the fact in other words (3
  cases, hand-verified).
- Synthetic questions are quarantined; the wikitext fallback and streaming
  are not mirrored; gw2 is excluded.
- The pt-BR visual report (`medicao-do-ask.html`, 2nd edition) stays local
  at the repo root, like the first edition.

## Addendum — merge-policy replay

One extra round (`eval/out/r2policy`, 221 questions, per-candidate hits
recorded) replayed through `eval/merge-policies.mjs`; the 3-round run
(`eval/out/r2`) replayed where the candidate split is unambiguous (561
records, biased toward the flood case where both candidates returned 4 hits).

| policy | r2policy (221) | vs prod | r2 (561, flood-biased) | vs prod |
| --- | --- | --- | --- | --- |
| production (`merge_hits`) | 84.6% | — | 84.8% | — |
| entity-first (cand1[0] reserved, then production) | 85.5% | +2 / −0 | 85.6% | +4 / −0 |
| consensus-guard (consensus vs cand1 only) | 85.1% | +3 / −2 | 85.4% | +8 / −5 |
| entity+raw-first (cand1[0] + raw[0] reserved, round-robin rest) | 86.4% | +7 / −3 | 89.1% | +26 / −2 |
| interleave (round-robin raw, cand1, cand2) | 86.4% | +9 / −5 | 89.7% | +30 / −3 |
| no-consensus | 84.2% | +3 / −4 | 84.7% | +8 / −9 |
| rewrite-only | 80.1% | +3 / −13 | 80.2% | +8 / −34 |
| raw-only | 49.3% | +5 / −83 | 52.2% | +9 / −192 |

Reading: a reserved slot for the bare-entity candidate's top hit is a free
win (never loses); reserving both that and the raw top hit and
round-robining the rest recovers most evictions (interleave gains a little
more but drops rewrite-only hits such as "Abigail gifts"). The per-question
gain/loss lists are in the replay output — the implementation PR should
diff them, not the headline.
