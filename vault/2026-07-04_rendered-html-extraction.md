---
title: Rendered-HTML extraction for tables and infoboxes
type: plan
status: done
created: 2026-07-04
updated: 2026-07-04
tags: [wiki, rag, rust]
related: ["[[2026-07-02_wikitext-extraction]]", "[[2026-07-04_golden-query-retrieval-tests]]", "[[2026-07-04_rendered-html-fetch-strategy]]"]
commit: 659db04
---

# Rendered-HTML extraction for tables and infoboxes

## In simple terms
Right now we send the AI only the *paragraphs* of a wiki page — every stat box and
table is thrown away during cleanup, because in the raw wiki source they're just
template calls with no data in them. But that's where the hard numbers live: crop
prices, growth times, damage, drop rates. So even with perfect retrieval, "which
winter crop is most profitable?" can't be answered with real numbers. The fix is to
ask the wiki for the *rendered* page (what a browser sees, tables filled in) and
convert those tables into text the AI can read. This is the difference between "you
can grow Winter Seeds" and "Powdermelon: 7 days, 60g → 250g".

## Context / problem
The answer-influence audit identified template/table stripping in
`wikitext.rs:14-15` as the single biggest code-side ceiling on answer quality:
`remove_balanced` deletes every `{{…}}` and `{|…|}` region, so infobox stats and
wikitables never reach the model — for a whole class of numeric questions a
grounded answer is structurally impossible, and the model must refuse or
hallucinate. This was a known, accepted trade-off in
[[2026-07-02_wikitext-extraction]] (raw wikitext keeps prose; rendered data lives in
a backend the wikitext doesn't contain) and is roadmap §6's first item — promoted to
a plan now that the retrieval work makes it the binding constraint.

## Goal / non-goals
- **Goal:** infobox and table data reaches the model's context, so numeric/stat
  questions ("what does X sell for", "best winter crop by profit") can be grounded.
- **Non-goal:** SQLite page caching (separate roadmap item; more attractive after
  this plan since parsed pages are costlier to fetch).
- **Non-goal:** prompt-shape or frontend changes (GFM table *rendering* in the
  answer view is a separate, smaller improvement).

## Approach
- Fetch rendered HTML via `action=parse&prop=text&page=<title>` and reduce it to
  LLM-friendly plaintext: keep paragraph prose; convert `<table>` rows to compact
  `cell | cell | cell` lines and infoboxes to `key: value` lines; drop nav boxes,
  edit-section links, citation markers, and style/script cruft.
- Open questions to resolve while implementing (each may become the fork below):
  - `action=parse` cannot batch — one request per page vs today's single batched
    revisions call (`fetch.rs:40-55`, and the CLAUDE.md etiquette note). Options:
    sequential parse fetches for all 4, parse only the top-1/2 pages + wikitext for
    the rest, or lower the page count for parse mode.
  - HTML volume vs the 8,000-char/page cap: rendered pages are bigger; tables must
    survive truncation (consider placing infobox/table digest before body prose).
  - Failure fallback: keep `to_plaintext(wikitext)` as the degraded path when
    `parse` errors, so answers never get worse than today.
- Measure with [[2026-07-04_golden-query-retrieval-tests]] cases whose expected
  evidence is tabular (e.g. assert the fetched context for "best crops for winter"
  contains a known price/day figure), plus a manual before/after ask.

## Decisions & trade-offs
- This plan deliberately revisits the accepted trade-off in
  [[2026-07-02_wikitext-extraction]]; that decision stays valid history — the
  constraint it accepted is now the bottleneck worth paying requests for.
- The batching-vs-parse choice is a real fork (etiquette + latency vs completeness):
  split into a decision doc via `/decision-doc` when reached, and link it here.

## Status log
- 2026-07-04 — created by promoting roadmap §6's first item; queued as step 4 of 4
  — the big win once retrieval reliably surfaces the right pages.
- 2026-07-04 — done; landed in `659db04`. New `wiki/html.rs` reducer (tables →
  pipe rows, portable infobox → `Label: Value`, navbox/TOC/images dropped) driven
  by live probes of both wikis; fetch is now one `action=parse` request per page
  (sequential, 12s timeout) with the batched-wikitext path kept as fallback. The
  fork was recorded in [[2026-07-04_rendered-html-fetch-strategy]]. Verified: 53
  offline + 5 live tests green, including new assertions that Powdermelon's
  context contains "Growth Time"/"60g" and Copper Ore's contains
  "Rarity: Common". Cost: "Reading pages…" now ~1s per page. Roadmap §6 item
  removed from CLAUDE.md.
