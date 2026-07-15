---
title: Run candidate searches concurrently and add an "understanding" ask status
type: plan
status: done
created: 2026-07-10
updated: 2026-07-15
tags: [wiki, rag, frontend]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_merge-raw-hit-guarantee]]"]
commit: "4952495"
---

# Run candidate searches concurrently and add an "understanding" ask status

## Context / problem

The rewrite-candidate searches run **sequentially** (commands.rs loop over
`REWRITE_SEARCH_LIMIT = 2`), making their latency strictly additive — live debug table:
608ms for 2 queries. The MediaWiki-etiquette rule that mandates sequential requests covers
the heavy `action=parse` page fetches, not two cheap search GETs (a no-op concern for
Elasticsearch-backed wikis, and small even for default-engine ones). Separately, the eager
rewrite stretches the "Searching…" phase; without a status change the added work reads as
*slower*, not *smarter* (second-AI finding, verified new).

## Goal / non-goals

- Goal: candidate searches run concurrently (`join_all`), cutting the phase from sum to max
  (~608 → ~300ms live); results zip back in candidate order so the merge input stays
  deterministic.
- Goal: a new `"understanding"` status between `searching` and `reading`, shown only while
  candidate searches actually run.
- Non-goal: page fetches stay sequential — the etiquette rule stands for `action=parse`.
- Non-goal: no status flash on skip/empty paths (rewrite disabled/skipped/zero candidates →
  the status must never appear).

## Approach

1. `src-tauri/src/commands.rs`: replace the sequential loop with
   `futures_util::future::join_all` (already a dependency) over the filtered candidates
   (post dup-skip from [[2026-07-10_merge-raw-hit-guarantee]], whichever lands first — the
   changes compose); zip hits back in candidate order; keep the per-candidate
   `retries.push(("rewrite", …))` bookkeeping for non-empty results. Emit
   `app.emit("ask://status", "understanding")` immediately **before** the `join_all`, only
   when the to-search list is non-empty. Update the ask-status ordering contract comment
   and the `REWRITE_SEARCH_LIMIT` doc ("sequential" → concurrent).
2. `src/types.ts`: add `"understanding"` to the `AskStatus` union + doc comment.
3. `src/App.tsx`: `STATUS_LABEL` gains `understanding: "Understanding your question…"` —
   the `Record<AskStatus, string>` is exhaustive, so tsc enforces the pair.
4. `CLAUDE.md` §4 events list: add `"understanding"` AND the pre-existing missing
   `"retrying"`; §5 MediaWiki-etiquette bullet: clarify the sequential rule covers page
   fetches — the ≤2 candidate search GETs are deliberately concurrent.
5. Debug table: no change — the existing `cand search` timer wraps the whole block, so the
   row simply shows the reduced (max-of) time; detail stays `"{n} queries -> {m} hits"`.

## Decisions & trade-offs

- Bundled #9 + #10 because they land in the same loop and the same UX moment: the moment
  the searches get faster is the moment the status can honestly say what the wait is.
- "Understanding your question…" (not "Rewriting…"): player-facing language, no jargon.
- Concurrency stays capped at 2 by `REWRITE_SEARCH_LIMIT` — no new load concern.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#9 + #10, priority 8), with the
  608ms live baseline from the first debug table.
- 2026-07-15 — done. Landed as planned: `join_all` over the filtered candidates (zip
  keeps candidate order for the merge), `"understanding"` emitted only when the
  to-search list is non-empty, `AskStatus`/`STATUS_LABEL` pair enforced by tsc, ask-flow
  test covers the new label, CLAUDE.md events + etiquette bullets updated. Debug table
  untouched — the `cand search` row format survives, its golden test unchanged.
