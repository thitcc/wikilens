---
title: Guarantee the raw top hit survives the merge, and skip duplicate rewrite candidates
type: plan
status: todo
created: 2026-07-10
updated: 2026-07-10
tags: [rag, wiki, rust]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_rewrite-prompt-reword]]"]
commit:
---

# Guarantee the raw top hit survives the merge, and skip duplicate rewrite candidates

## Context / problem

`merge_hits` (commands.rs) ranks consensus → rewrite-only → raw-only under a cap of 4. Two
candidate searches can contribute up to 8 hits, so a confident-but-wrong rewrite can flood
the cap and **evict the correct raw #1** — the exact silent-failure mode the decision doc
flagged as the rewrite's worst risk.

**Live evidence (2026-07-10, first `WIKILENS_DEBUG` table):** "what gifts does Abigail
likes?" fetched `List of All Gifts | Friendship | Villagers | Caroline` — Abigail's own page
(the golden-query expectation for this question in `wiki/mod.rs`, and where her gift tables
live) was **absent** from the merged top-4, while keyword-soup candidates contributed 7 hits.

Secondary (LOW): the rewrite sometimes emits a candidate equal to the raw query — that wastes
a whole search round-trip and a merge slot on duplicate hits.

## Goal / non-goals

- Goal: `raw[0]` is always **included** in the merged result (when it exists); rewrite-only
  hits can fill at most `limit − 1` slots unless raw[0] already made it in via consensus.
- Goal: candidates case-insensitively equal to the raw query are dropped before searching.
- Non-goal: do NOT pin raw[0] to rank #1 — rank inside the top-4 doesn't matter (all four
  pages are fetched and handed to the model); **inclusion** does. Pinning would undo the
  feature's motivating win: the injected entity outranking junk raw hits.

## Approach

1. `merge_hits` phase-2 change: compute
   `raw_first_included = raw.first()` matched case-insensitively against what phase 1
   (consensus) already emitted; cap the rewrite-only fill at
   `if raw_first_included { limit } else { limit.saturating_sub(1) }`; phase 3 unchanged —
   raw[0] then fills its reserved slot first. Update the doc comment: inclusion matters,
   not rank.
2. Candidate dup-skip in the loop above the merge: filter
   `!rq.eq_ignore_ascii_case(&query)` **before** `.take(REWRITE_SEARCH_LIMIT)` (so a
   surviving second candidate still gets searched); trace a dropped-duplicate line when
   `WIKILENS_TRACE_RETRIEVAL` is on. A dropped duplicate still counts as rewrite *success*
   for the circuit breaker — the model parsed fine.
3. Tests: the 4 existing merge tests pass unchanged (traced during the first
   implementation pass). New: `raw_first_survives_a_rewrite_flood` (raw [R1,R2] + 5 rewrite
   hits, limit 4 → [A,B,C,R1]); `consensus_on_raw_first_frees_the_reserved_slot`;
   `limit_one_still_keeps_raw_first` (exercises the `saturating_sub`).

## Decisions & trade-offs

- Reserve-a-slot beats pin-first: keeps the entity-injection win while closing the eviction
  hole; the cost is at most one rewrite hit displaced.
- First execution step is a **confirmation experiment**: re-run the Abigail question with
  `WIKILENS_QUERY_REWRITE=0` — if her page appears in the raw-only top-4, the live table
  proved the eviction; keep both debug tables as the before/after record.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#8, MEDIUM+LOW) — promoted in
  priority by the first live debug-table evidence (Abigail page evicted).
