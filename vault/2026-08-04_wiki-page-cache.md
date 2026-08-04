---
title: SQLite cache of fetched wiki pages
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [wiki, rust]
related:
  - "[[2026-07-04_rendered-html-fetch-strategy]]"
  - "[[2026-07-07_retrieval-quality-improvements]]"
commit:
---

# SQLite cache of fetched wiki pages

## Context / problem

Every ask re-fetches its pages live — sequential `action=parse` calls at
~1s/page, kept sequential for MediaWiki etiquette. A local page cache would
amortize that cost across asks, and other work keeps deferring to it:
[[2026-07-04_rendered-html-fetch-strategy]] names it as the follow-up that
absorbs the per-page cost, and [[2026-07-07_retrieval-quality-improvements]]
defers semantic retrieval until it exists.

## Goal / non-goals

- Goal: an SQLite cache in the app-data dir; page fetches consult it before
  going to the network, with a TTL/invalidation story so stale wiki edits
  don't linger forever.
- Non-goal: semantic/vector retrieval itself (a separate follow-up this
  merely unblocks); caching search results.

## Approach

Sketch: key by (game, title) with the fetched plaintext + fetch timestamp;
misses fetch live (sequential etiquette unchanged) and backfill. Decide TTL
per wiki activity, and a manual "refresh" escape hatch if staleness bites.

## Decisions & trade-offs

`vault/index.md` reserves a `cache` tag for when this work starts — register
it in the tag registry then (this doc wears `wiki, rust` meanwhile).

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
