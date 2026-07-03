---
title: Read wiki content via raw wikitext, not prop=extracts
type: decision
status: done
created: 2026-07-02
updated: 2026-07-02
tags: [wiki, rag]
related:
  - "[[2026-07-02_scaffold]]"
  - "[[2026-07-02_multi-provider-llm]]"
commit: 82c1e8e
---

# Read wiki content via raw wikitext, not prop=extracts

## Context

A user asked Core Keeper *"best way to get wood"* and got: *"I found matching pages
but couldn't read their contents."* Search worked (pages were found); reading their
**text** failed. Root cause: content was read via `prop=extracts` (the MediaWiki
*TextExtracts* extension), which **many game wikis don't have installed** — Core
Keeper and Stardew both lack it, so it returned empty text.

Investigating the fix surfaced a second, latent bug: a whole-article
`prop=extracts` request is **capped to one page** (`exlimit` is silently lowered to
1, confirmed live via the API warning). So even **Conan Exiles** — which *does* have
TextExtracts — was only ever feeding the top 1 of N search hits into the answer.
The multi-page RAG was quietly broken for every game.

## Decision

Read page content as **raw wikitext** via `prop=revisions&rvprop=content&rvslots=main`
(a single batched request for all titles) and convert it to plaintext with a new,
unit-tested `src-tauri/src/wiki/wikitext.rs` module. Drop `prop=extracts` entirely.

## Consequences

- **Good:** works on **every** MediaWiki wiki (Core Keeper and Stardew now answer);
  returns **all** requested pages in one batched call (fixes the `exlimit=1` cap, so
  Conan now uses 2–4 sources instead of 1). Verified: Core Keeper "Wood" cleans to
  usable prose that answers the original question.
- **Cost / bad:** the cleaner keeps article **prose** but strips template/infobox
  **tables**, so a purely numeric stat that lives only in an infobox may be missing.
  The cleaner only strips *known* HTML tags (allowlist) so command placeholders like
  `<item>` survive.
- **Follow-ups:** roadmap item — rendered-HTML extraction (`action=parse&prop=text`)
  to also capture infobox/stat tables.

## Alternatives considered

- **Keep `prop=extracts`, one request per page** — rejected: still returns nothing
  on wikis without TextExtracts (Core Keeper, Stardew), and N parallel page fetches
  violate MediaWiki batching etiquette.
- **`action=parse&prop=text` (rendered HTML) now** — deferred to the roadmap:
  heavier to parse/clean; wikitext already restored correct prose for all three
  wikis, so it wasn't needed to unblock the user.
