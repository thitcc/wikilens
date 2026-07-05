---
title: Rendered-HTML fetch strategy and reducer approach
type: decision
status: done
created: 2026-07-04
updated: 2026-07-04
tags: [wiki, rag, rust]
related: ["[[2026-07-04_rendered-html-extraction]]", "[[2026-07-02_wikitext-extraction]]"]
commit: 659db04
---

# Rendered-HTML fetch strategy and reducer approach

## Context
Getting infobox/table data to the model requires `action=parse&prop=text`, which
cannot batch — the single-batched-request etiquette from
[[2026-07-02_wikitext-extraction]] can't carry over unchanged. Live probes
(2026-07-04) measured 0.7–2.7s per parse on both wikis with one observed 35s
Fandom stall, 100–320KB rendered payloads, and confirmed the data exists only in
the rendered form (Fandom's Copper Ore wikitext is a 641-char template call).
The shared reqwest client has no global timeout (known ops gap), so a stalled
parse would wedge the ask. Rust-side HTML handling needs either a parser
dependency or a hand-rolled reducer.

## Decision
Fetch all ranked pages as rendered HTML, one sequential `action=parse` request
per page with a 12-second per-request timeout, falling back to one batched
`prop=revisions` wikitext request for any failures; reduce the HTML with a
hand-rolled single-pass tokenizer (`wiki/html.rs`), not an HTML-parser crate.

## Consequences
- Good: infobox stats, prices, and drop tables reach the model on every page;
  redirects resolve to final titles; a stalled wiki degrades to today's
  prose-only behavior instead of hanging; no new dependencies.
- Cost / bad: "Reading pages…" now costs ~1s per page (typically ~4s for a full
  top-4 fetch) instead of one round-trip; the reducer must track MediaWiki/
  Fandom markup conventions (navbox ids/classes, portable-infobox classes) and
  may need upkeep if those change.
- Follow-ups: the SQLite page cache (roadmap) would amortize the per-page cost;
  the golden suite's known-gap case may now be closeable since gift tables are
  in context.

## Alternatives considered
- **Parse only the top-1/2 pages, wikitext for the rest** — cheaper, but grounds
  different pages with different quality tiers; the model can't tell which
  sources carry data, and stat questions about the 3rd-ranked page still fail.
- **Parallel parse requests** — fastest, but violates the MediaWiki etiquette
  this project committed to (and Fandom bot-throttling risk).
- **Stay wikitext-only** — status quo; rejected because the data simply isn't in
  the source text (template/Lua-generated), which was the audit's biggest
  answer-quality ceiling.
- **`scraper`/html5ever dependency for the reducer** — a real DOM and CSS
  selectors, but a heavy dependency tree for well-formed machine-generated HTML;
  the codebase already has the hand-rolled-cleaner idiom (`wikitext.rs`) with
  the same fixture-test style, and the reducer needs only tag/id/class checks.
