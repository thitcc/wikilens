---
title: Core ask pipeline — latency and truthfulness analysis
type: research
status: done
created: 2026-08-17
updated: 2026-08-17
tags: [rag, wiki, llm]
related:
  - "[[2026-08-04_wiki-page-cache]]"
  - "[[2026-07-07_retrieval-quality-improvements]]"
  - "[[2026-07-07_llm-query-rewrite-in-retrieval]]"
  - "[[2026-07-15_rewrite-bare-entity-candidate]]"
  - "[[2026-07-10_merge-raw-hit-guarantee]]"
  - "[[2026-07-10_ask-debug-instrumentation]]"
  - "[[2026-07-04_golden-query-retrieval-tests]]"
  - "[[2026-07-04_rendered-html-fetch-strategy]]"
commit: 27eb27d
---

# Core ask pipeline — latency and truthfulness analysis

Deep analysis of the ask pipeline (hotkey → wiki search → RAG → streamed
answer), prompted by two lived complaints: asks feel **slow**, and some answers
are **not true**. Analyzed at `27eb27d` (2026-08-17). All file:line citations
were spot-verified against source; live numbers come from a fresh probe run
(method in the appendix). A pt-BR visual companion page was delivered
alongside this doc as an uncommitted local preview at the repo root
(kept out of the repo by request).

## Summary

**"Slow" is structural, not incidental.** A typical ask is 4 serial phases and
7–9 network round-trips, nothing reused between asks: `join(raw search ‖ LLM
rewrite)` → ≤2 candidate searches → ≤4 *sequential* page parses → answer
stream. The dominant cost is the sequential fetch (live: 0.3–1.3s per page
typical, one 5.1s cold-render stall observed today; author-observed worst: a
56s ask with 46.6s of fetch). The rewrite (~2.4s cold, measured) frequently
paces the first phase, and buys an *extra* serial search phase after it. The
answer stream has no wall-clock ceiling at all. Nothing is cached across asks
— the page cache is a parked idea ([[2026-08-04_wiki-page-cache]]).

**"Untrue" has two root families.** (1) *Retrieval misses that don't look like
misses*: wrong-but-nonzero hits — already identified in July as the dominant
failure mode — put related-but-wrong pages in front of the model, and today's
probe reproduced two live (see appendix). (2) *Generation that fails silently*:
grounding is instruction-only with two prompt clauses in tension, truncation
(`max_tokens`, stream drop, 8000-char page cut, table-stripping fallback) is
never surfaced, and the Sources list renders identically under all of it — an
ungrounded answer looks exactly as sourced as a grounded one. 18 risk points
are cataloged below (R1–R18).

**The single highest-leverage next step is measurement**: the ~40-question
eval pass that `WIKILENS_TRACE_RETRIEVAL` was built for
([[2026-07-07_retrieval-quality-improvements]] phase 2) never ran. Every past
pivot in this pipeline came from live traces; the roadmap below starts there.

## How an ask actually runs

Pre-flight is local and fails fast: concurrency claim (`commands.rs:760`),
cancel token (`commands.rs:770-774`; cancel drops the future at its await
point), image resolution, game lookup, target resolution incl. DPAPI key read
(`commands.rs:905-931`). Then:

1. **`searching`** — `preprocess_query` strips a 60-word stopword list
   (`search.rs:26-51`), then **`join(raw search ‖ rewrite)`**
   (`commands.rs:1025-1026`). Raw search: 1 GET, `srlimit=4`, `srwhat=text`
   only when multi-word (`search.rs:10, 124-126`), 8s timeout
   (`search.rs:16`). Rewrite: non-streaming completion, 256 tokens out, 4s
   total-request timeout, ~2.4s cold measured (`llm.rs:387-397`). Phase cost =
   max of the two. Rewrite errors are swallowed into "no candidates"
   (`commands.rs:1011-1022`); a raw-search error kills the ask
   (`commands.rs:1057`) even when the rewrite succeeded.
2. **`understanding`** — candidates that echo the raw query are dropped, then
   `.take(2)` (`REWRITE_SEARCH_LIMIT`, `commands.rs:1066-1076, 1310`); the ≤2
   searches run concurrently (`commands.rs:1087-1092`) but form a **new serial
   phase** after the join. `merge_hits` ranks consensus → rewrite-only → raw,
   cap 4, with an *inclusion* (not rank) guarantee for raw[0]
   (`commands.rs:1326-1349`).
3. **Zero-hit ladder** (only when the merge is empty; strictly serial):
   MediaWiki's own suggestion (8s) → `simplify_query` retry (8s) → title-index
   fuzzy match (`commands.rs:1111-1169`). First use per game walks
   `list=allpages`: up to 20 sequential GETs, 8s each, 15s budget — but the
   budget is checked only at loop top (`titles.rs:150-153`), so worst case is
   ~23s. Still empty → canned answer, no LLM call (`commands.rs:1177-1186`).
4. **`reading`** — ≤4 **sequential** `action=parse` GETs, 12s each
   (`fetch.rs:27, 53-59`), deliberately serial for MediaWiki etiquette
   (`fetch.rs:40-41`). Failures pool into one batched `prop=revisions`
   wikitext request (`fetch.rs:61-68`) — prose only, all templates and tables
   deleted (`wikitext.rs:14-15`). Fetch ceiling: 4×12s + 12s = **60s**. Each
   page head-truncated at 8000 chars (`fetch.rs:20, 226-232`).
5. **`answering`** — 1 streaming call. Context = all pages as
   `<wiki_excerpt title="…">` blocks + the *original* question
   (`llm.rs:366-378`); no aggregate cap (worst ≈32k chars), `max_tokens=1024`
   (`llm.rs:23`), no temperature sent. **No request timeout** — bounds are the
   client-wide 60s read-gap, 1MiB stream cap (`http.rs:31`, `llm.rs:30`).
6. Post: sources built from *exactly* the pages sent to the model
   (`commands.rs:1255-1261`), history appended best-effort.

Session-cached: title index per game, HTTP connection pool, rewrite circuit
breaker (2 strikes → rewrites off for the session, `state.rs:16, 76-103`).
**Recomputed every ask: searches, pages, rewrite.**

## Where the time goes

Live probe (2026-08-17, 5 built-in wikis, sequential GETs, method in
appendix): search 0.73–1.32s; parse 0.31–1.31s typical, **5.07s** Terraria
cold render (0.53s warm on the immediate retry); parse payloads 228–614 KB.
Author-measured: rewrite ~2.4s cold (`llm.rs:394`); one real 56s ask with
46.6s fetch (four parses ~11.6s each, just under the 12s timeout —
[[2026-07-15_slow-wiki-status-hint]]); one 35s Fandom stall
([[2026-07-04_rendered-html-fetch-strategy]]).

A realistic decomposition of a typical healthy ask:

| Phase | Typical | Ceiling |
|---|---|---|
| raw search ‖ rewrite | ~0.7–2.4s (rewrite often paces) | 8s |
| candidate searches | ~0.3–0.8s | 8s |
| page fetch (4 pages, sequential) | ~1.5–6s | 60s |
| answer stream (TTFT + generation) | ~2–8s | unbounded |
| **total** | **~5–15s** | — |

Dominant contributors, ranked:

1. **Sequential page fetch** — the biggest typical cost *and* the worst tail.
   Parallelizing is off the table (etiquette, locked in
   [[2026-07-04_rendered-html-fetch-strategy]]); the sanctioned fix is the
   page cache, which amortizes it to zero for revisited pages.
2. **Answer stream** — scales with context (up to ~32k chars into a small
   default model) and has no total bound. TTFT is pure provider prefill.
3. **The rewrite join + candidate phase** — the "free because concurrent"
   framing is half true: the join costs `max(raw, rewrite)` where the rewrite
   (~2.4s) usually exceeds the raw search (~0.7–1.3s), and the candidate
   searches then add a whole serial phase.
4. **Title-index first build** — zero-hit path only, but ~23s worst case
   because the 15s budget is checked only before each request starts, never
   accounted against the 8s an in-flight request may still take
   (`titles.rs:150-153`).
5. **Zero-hit ladder stacking** — a bad query can serialize 8+8+23s before
   fetch even starts.

Caveat: the timing figures in `debug.rs` tests (rewrite 412ms / fetch 1893ms /
answer 6120ms) are **unit-test fixtures**, not measurements — they encode the
author's expected shape (answer ≫ fetch ≫ searches), which the live numbers
above independently confirm.

## Why answers can be untrue

Eighteen risk points, grouped by mechanism. The lead cluster explains the
"answers that are not true" reports most directly.

### Generation fails silently

- **R1 — `max_tokens` truncation is invisible.** The Anthropic parser reads
  `message_delta` only for usage and ignores `stop_reason`
  (`llm.rs:700-710`); the OpenAI parser only reacts to
  `finish_reason=="error"` (`llm.rs:752`) — `"length"` falls through. A
  1024-token cut mid-list renders as a complete-looking answer; omissions read
  as facts ("not a liked gift").
- **R2 — stream drop = success.** Anthropic needs no `message_stop`, OpenAI
  tolerates a missing `[DONE]` (`llm.rs:212-218`); a connection drop yields a
  partial answer presented as final.
- **R3 — reasoning models are unguarded on the answer path.** The skip
  protects only the rewrite (`commands.rs:878-888`). DeepSeek's default
  answer model `deepseek-v4-flash` *is* reasoning (`providers.rs:92-101`),
  and the OpenAI-path parser discards `reasoning_content` deltas
  (`llm.rs:759-767`): thinking burns the 1024-token budget and the visible
  answer arrives empty or as a stub — above a full, authoritative Sources
  list.
- **R4 — grounding is instruction-only, and the prompt fights itself.** "If
  the excerpts do not contain the answer, say so plainly" coexists with "Do
  not mention that you were given excerpts" (`llm.rs:36`). A model reconciling
  those can prefer answering from parametric memory over admitting a gap it
  can't name. There is no post-hoc grounding check anywhere; `run_ask` takes
  the streamed text verbatim (`commands.rs:1248`).
- **R17 — the answer model is unvalidated and defaults small** (Haiku 4.5 /
  V4 Flash / GPT-4o mini, `providers.rs:72, 92, 111`) — over up to 32k chars
  of pipe-delimited table soup whose header row appears once (see R9), the
  exact setup for label/value misattribution.

### Retrieval misses that don't look like misses

- **R12 — ranking is MediaWiki's, unweighted, no floor.** Any 4 hits become
  the context; there is no "these results are bad" gate (`search.rs:137-154`).
  Live today: Stardew `Abigail gifts` → Villagers / Leek / Tea Set / Minerals
  — the Abigail page absent (the known golden gap, `wiki/mod.rs:31`);
  Minecraft `iron ore fortune` → four pages *not* including Iron Ore.
- **R7 — rewrite-only hits outrank raw hits** (slots 1–3 possible,
  `commands.rs:1341-1343`): a confidently-wrong rewrite entity fills the top
  of context. Mitigated only by raw[0] *inclusion*
  ([[2026-07-10_merge-raw-hit-guarantee]]).
- **R8 — rewrite drift is bounded but real for selection**: intent-stripping
  collapses "avoid X" and "get X" to "X"; the answering model always gets the
  original question (`commands.rs:1226-1229`), so drift decides *which pages*
  it sees, not what it's asked.
- **R5 — the title-index fallback replaces the whole context with one
  fuzzy-matched page** (`commands.rs:1157`), Jaro-Winkler ≥0.92 with no
  disambiguation or second candidate — adjacent-but-wrong sibling titles
  clear the bar.
- **R6 — redirects sink the top hit.** `sort_by_relevance` matches the
  post-redirect `final_title` against the requested titles by exact string;
  misses sort to `usize::MAX` (`fetch.rs:99-108, 216-223`), so the #1 search
  hit can become excerpt #4.
- **R13 — the answer prompt never names the game.** `wiki.name` reaches the
  rewrite call only (`llm.rs:432`). On shared wikis (Nukapedia = every
  Fallout; Grounded 1/2 share a Fandom wiki) nothing tells the model which
  game's stats to answer with — the known fusion-core golden gap
  (`wiki/mod.rs:52-57`).
- **R16 — rewrite failure degrades silently for the whole session** (2-strike
  breaker, `state.rs:76-103`): retrieval quietly falls back to raw-keyword
  quality — exactly the quality the live probe shows failing above — with no
  user-visible signal.

### Extraction loses the facts

- **R10 — head truncation at 8000 chars cuts late-page content.** Gift
  tables, drop tables, and version notes live late on game-wiki pages; the
  `…[truncated]` marker gets no prompt instruction (`fetch.rs:226-232`).
- **R11 — the wikitext fallback deletes every template and table**
  (`wikitext.rs:14-15`): one 12s parse timeout on one page and that page
  arrives as stats-free prose — while still listed as a Source. Numeric
  questions get answered from model memory with a plausible citation under
  them.
- **R9 — the HTML reducer ignores colspan/rowspan and icon-only cells**
  (`html.rs:119-212`): pipe-rows can misalign against their header (stated
  once per table), and items identified only by an icon (`alt` text never
  read) vanish from recipe/drop rows.
- **R18 — pages that reduce to nothing are silently dropped**
  (`fetch.rs:56`): the model can receive 1 page where 4 were selected, with
  no thin-context signal to the model or the user.

### The trust surface amplifies all of it

- **Sources are structurally exact but semantically unattributed**: the list
  is precisely the pages handed to the model (`commands.rs:1255-1261`) — but
  nothing checks the answer *used* them. A fully parametric answer (R4) and a
  fully grounded one render identically, sources and all. Truncated (R10) and
  table-stripped (R11) pages are listed indistinguishably from complete ones.
- **R14 — no staleness signal**: no cache means content is always live-fresh,
  but the wiki itself may lag the game patch, and no date reaches the model
  or the user. The session-cached title index has no TTL.
- **R15 — excerpt fencing is unescaped by design** (`llm.rs:360-365, 36`);
  the accepted residual risk is a vandalized page steering the answer.

### Already mitigated (don't re-fix)

Zero hits produce a canned answer, never an LLM call (`commands.rs:1177`).
The answering model always receives the original question. raw[0] inclusion
is guaranteed in the merge. Known-reasoning models skip the rewrite; the
breaker bounds unknown ones at 2 strikes. The HTML reducer is
snapshot-tested; retrieval has a live golden suite (20 cases, hit@4, 2 known
gaps) — [[2026-07-04_golden-query-retrieval-tests]].

## Constraints any fix must respect

- **Sequential page fetches** — MediaWiki etiquette, locked
  ([[2026-07-04_rendered-html-fetch-strategy]]). Speed must come from caching
  or fetching less, not parallelism.
- **No silent cross-provider auto-pick, ever** — locked
  ([[2026-07-10_reasoning-skip-and-capability-tags]]). No fix may route the
  player's question to a vendor they didn't choose.
- **The recovery ladder stays zero-hit-only** — a wrong-but-nonzero ladder is
  explicitly parked as separate work
  ([[2026-07-15_rewrite-bare-entity-candidate]]); un-parking it needs its own
  decision doc.
- **Detection stays suggest-only; the registry stays Rust-side** — untouched
  by anything here.
- Semantic/vector retrieval is **gated on the page cache existing**
  ([[2026-07-07_retrieval-quality-improvements]]).

## Improvement roadmap

Tiered by prerequisite order; every item checked against the constraints
above. Impact/effort noted as (truthfulness | latency | effort).

### Tier 0 — measure first (do before building anything)

1. **Run the parked ~40-question eval pass** with `WIKILENS_TRACE_RETRIEVAL`
   across 4–5 games and question styles (entity lookup, stat lookup, how-to,
   negation, typo). It was designed in
   [[2026-07-07_retrieval-quality-improvements]] phase 2 and never ran; every
   prior pivot (lazy→eager rewrite, bare-entity prompt) came from live
   traces. Bucket misses: zero-hit / wrong-but-nonzero / truncation-downstream.
   (high | high | S)
2. **Extend the golden suite with wrong-but-nonzero cases** (assert the
   *right* page is present, not just any page) and add per-phase timing
   capture to the trace lines so latency claims stop depending on anecdotes.
   (med | med | S)

### Tier 1 — quick wins (small diffs, land independently)

1. **Surface truncation** (R1, R2): parse `stop_reason` / `finish_reason` in
   both SSE parsers; when the stop is `max_tokens`/`length` — or the stream
   ends without its terminator — mark the answer visibly incomplete in the
   UI. (high | — | S)
2. **Raise `MAX_TOKENS`** 1024 → 2048–4096 (`llm.rs:23`); with stop-reason
   surfacing, the residual cut is at least visible. (med | — | XS)
3. **Prompt surgery** (R4, R10, R13), one change, three edits: give the model
   a sayable refusal that doesn't require mentioning excerpts ("the wiki
   pages I checked don't cover this — try searching *X*"); state the game
   name in the answer system prompt (`wiki.name` is already in scope at the
   call site); instruct that `…[truncated]` means the page continues and
   absence-of-mention is not absence-of-fact. (high | — | S)
4. **Guard the answer path against reasoning models** (R3): read
   `reasoning_content` deltas as fallback text on the OpenAI path, or
   auto-warn/refuse when the picked answer model is tagged Reasoning — same
   classification the rewrite skip already uses. Respects the
   no-cross-provider rule (no routing change). (high | med | S)
5. **Fix redirect ordering** (R6): return the requested title alongside
   `final_title` from `fetch_rendered_page` and sort by the requested one.
   (med | — | XS)
6. **Fix the title-index budget check** (latency #4): account remaining
   budget before starting a request, capping the walk at ~15s true wall time
   (`titles.rs:150-153`). (— | med | XS)
7. **Skip the rewrite for entity-shaped queries** (latency #3): a 1–2-word
   preprocessed query *is* already the bare entity the rewrite exists to
   produce — skip the join's LLM leg and the candidate phase entirely for
   those. Saves ~2.4s + one serial phase on the most common quick lookups.
   Trace-verify the heuristic in Tier 0 first. (— | high | S)

### Tier 2 — medium (retrieval and extraction quality)

1. **Section-aware context assembly** (R10): `html.rs` already walks
   headings; keep per-section text, budget the 8000 chars across sections
   with priority to those matching query terms, always keeping the
   infobox/lede. Kills the "gift table at char 11k" class outright.
   (high | — | M)
2. **A minimal relevance floor + thin-context surfacing** (R12, R18): require
   minimal query-term overlap for a hit to enter the merge; when the context
   ends up thin (0–1 pages, or all-fallback), tell the model *and* the user
   ("answering from limited wiki text"). (med | — | M)
3. **Title-index disambiguation + rewrite validation** (R5, and the
   never-built mitigation from [[2026-07-07_llm-query-rewrite-in-retrieval]]):
   when fuzzy scores are close, take both candidates instead of one; validate
   rewrite entities against the title index when it's already built.
   (med | — | M)
4. **Table fidelity in the reducer** (R9): honor colspan (pad cells), read
   `alt`/`title` off icon-only cells, repeat header context on long tables
   (e.g. `Header: value` pairs instead of bare pipes for wide tables).
   Snapshot suite makes this safely reviewable. (med | — | M)
5. **Mark degraded sources** (R11 surface): when a page came via the wikitext
   fallback or was truncated, annotate it in the excerpt header (the model
   can caveat) — and optionally in the Sources list. (med | — | S)

### Tier 3 — big rocks (the "improved a lot" arc, in prerequisite order)

1. **SQLite page cache** — the parked [[2026-08-04_wiki-page-cache]] idea and
   the single biggest latency lever: repeat/nearby asks drop the entire fetch
   phase (typically 1.5–6s, tail 60s → ~0). Sequential etiquette unchanged on
   misses; TTL per wiki activity + manual refresh; also fixes R14 by storing
   fetch timestamps (surfaceable as "as of …"). Register the reserved `cache`
   tag when this starts. (med | very high | L)
2. **Chunk/section-level retrieval over the cached corpus**: retrieve wiki
   *sections* rather than whole pages — more distinct pages within the same
   token budget, less truncation, better grounding density. Requires 1.
   (high | med | L)
3. **Embeddings + re-rank** (explicitly gated on the cache by prior art):
   embed cached chunks locally or via the configured provider (never a
   second vendor silently — constraint above), re-rank merged hits and
   sections by similarity to the question; this is the durable fix for
   wrong-but-nonzero. (very high | med | XL)
4. **Un-park the wrong-but-nonzero recovery ladder** as its own decision doc
   *after* Tier 0 quantifies the miss rate — with embeddings (3) it may
   become unnecessary; without them it's the fallback plan. (med | — | M)

### Explicitly not recommended

- Parallelizing page fetches (locked constraint; the cache achieves the same
  end politely).
- A total wall-clock timeout on the answer stream (long generations are
  legitimate; the 60s read-gap already bounds true stalls).
- Auto-switching providers/models on failure (locked constraint).

## Appendix — live probe (2026-08-17)

Method: 15 sequential GETs (Node script, scratchpad-only; UA
`wikilens/0.1 … research-probe`; 300ms pauses) against 5 built-in wikis: one
`list=search` (`srlimit=4`, `srwhat=text`, multi-word query) + the same
`action=parse&prop=text` twice (cold, then immediately warm). GW2 skipped
(WAF-blocked on this network). Not a benchmark — one sample per cell, taken
from one residential connection; useful for orders of magnitude only.

| Wiki | search | parse cold | parse warm | parse bytes |
|---|---|---|---|---|
| stardewvalleywiki.com | 1316ms | 895ms | 314ms | 240KB |
| core-keeper.fandom.com | 893ms | 744ms | 604ms | 228KB |
| minecraft.wiki | 852ms | 951ms | 1306ms | 614KB |
| terraria.wiki.gg | 731ms | **5068ms** | 532ms | 479KB |
| wiki.warframe.com | 741ms | 391ms | 343ms | 493KB |

Retrieval spot-checks reproduced live (same probe):

- Stardew `Abigail gifts` → `Villagers, Leek, Tea Set, Minerals` — the
  Abigail page absent from top-4 (the known non-strict golden gap; the
  rewrite's bare-entity candidate `Abigail` is what rescues this ask when the
  rewrite is healthy — and R16 means it silently isn't always).
- Minecraft `iron ore fortune` → `Raw Iron, Potone Redstone Ore, Resin Ore,
  Poisonous Potato Ore` — Iron Ore absent from top-4.

## Status log

- 2026-08-17 — created; analysis of `27eb27d` via three parallel code/vault
  surveys + live probe. The pt-BR visual companion page is delivered as an
  uncommitted local preview at the repo root (kept out of the repo by
  request).
