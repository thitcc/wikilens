---
title: Improve wiki retrieval quality — suggestion-retry, eval logging, LLM query rewrite, title index
type: plan
status: active
created: 2026-07-07
updated: 2026-07-07
tags: [rag, wiki, llm, rust]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-04_query-preprocessing-zero-hit-retry]]", "[[2026-07-04_golden-query-retrieval-tests]]", "[[2026-07-04_rendered-html-fetch-strategy]]", "[[2026-07-02_multi-provider-llm]]"]
commit:
---

# Improve wiki retrieval quality — suggestion-retry, eval logging, LLM query rewrite, title index

## In simple terms
Search is the weakest link in the answer pipeline: we turn a question into keywords with
a hard-coded stopword list and pull the top 4 pages, and entity-style questions only land
~half the time. This plan fixes it in four staged steps — grab a free "did you mean" the
wiki already sends, measure where we actually lose, add an AI query-rewrite only where the
cheap steps can't help, and finally build a local index of page titles so typos resolve
instantly and offline.

## Context / problem
Retrieval quality is the ceiling on answer quality — if the right page isn't in the top 4
(`DEFAULT_SEARCH_LIMIT`, `wiki/search.rs:7`), the grounded model can't answer. The current
search reduces a question to keywords via a hard-coded English stopword denylist
(`preprocess_query`, `wiki/search.rs:24`). The failure is concentrated in the search stage,
not the architecture:
- **Typos are unfixable by the current design.** "arcanr persitance" has no stopword
  tokens → sent verbatim → AND-match → 0 hits; the one zero-hit retry (`simplify_query`,
  `search.rs:41`) keeps ≥4-char tokens, so both misspelled tokens survive and it re-sends
  an identical miss. Stopword tuning is the wrong tool class for spelling.
- **Vocabulary gaps** ("the thing that revives you" → "Last Gasp") and **name-vs-stopword
  collisions** (an NPC literally named "Will") are lexical-search ceilings.
- Today retrieval fails *transparently* (0 hits → "couldn't find anything, rephrase"). Any
  fix must not trade that for confident-wrong *silent* failures.

## Goal / non-goals
- **Goal:** raise entity-style retrieval hit-rate (right page in top 4) from ~50% toward
  85%+, without regressing transparent-failure UX or the fast/free happy path.
- **Goal:** make the cheap-vs-expensive-fix decision data-driven — measure before building
  the AI lever.
- **Non-goal:** semantic/vector retrieval (deferred until the SQLite cache exists — roadmap §6).
- **Non-goal:** changing what the answering model receives — it keeps getting the original,
  untouched question (`commands.rs:395-397`).

## Approach (staged; sequencing is deliberate)

**Phase 1 — Suggestion-retry (hours, free).** On 0/low hits, retry with MediaWiki's
`query.searchinfo.suggestion` ("did you mean") before falling to `simplify_query`. `srinfo`
defaults to `totalhits|suggestion`, so the suggestion is very likely already in the JSON we
fetch and simply discarded — `parse_search_response` (`wiki/search.rs:107`) extracts only
`query.search[].title`. Covers most built-ins (Fandom, wiki.gg, minecraft.wiki run
CirrusSearch); no-ops gracefully where absent (Stardew's default MySQL engine). Slots into
the existing zero-hit retry hook in `run_ask` (`commands.rs:400-410`).

**Phase 2 — Eval logging (a day). Decides the rest.** Log `question → search query → the 4
titles` and hand-score ~40 real questions (typos included) on two independent axes:
(1) was the right page in the top 4 (retrieval), (2) did the answer help (end-to-end).
Bucket retrieval misses into **0-hit / wrong-hit-nonzero / downstream-truncation** (right
page fetched but the 8k `MAX_PAGE_CHARS` cap, `wiki/fetch.rs:21`, sliced the relevant table
off a huge Warframe/PoE page). This split says whether retrieval is even the bottleneck and
whether Phase 3 needs to be lazy or eager. Builds on [[2026-07-04_golden-query-retrieval-tests]].

**Phase 3 — LLM query rewrite (a day or two), lazy-first, defensive.** Standard RAG query
rewriting: one call to a cheap/fast model, prompt includes the game name, strict JSON out
(`{"queries": [...], "entities": [...]}`), 2 variants merged. Reuses existing provider +
streaming infra (`providers.rs`, `llm.rs`) — no new dependency, keys already configured.
Constraints (see the decision doc):
- **Lazy first** — fire only on the zero/low-hit retry path (same hook as Phase 1); the
  happy path stays fast and free. Lazy catches the 0-hit class (typos); eager (every query)
  is only needed for the wrong-hit vocabulary-gap class — gate that escalation on Phase 2.
- **Merge, don't replace** raw-query hits; **hard fallback** to the raw query on any
  failure; **keep sending the original question** to the answering model.
- Risk: a confident wrong correction (esp. post-training-cutoff patch content) fails
  silently. Phase 4's title index is the validator that neutralizes it.

**Phase 4 — Local title index (durable fix).** Pull `list=allpages` + `list=allredirects`
once per game into the roadmap SQLite cache; fuzzy-match query tokens against titles locally
(Jaro-Winkler is fine at wiki scale, <5ms; or the `fst` crate's Levenshtein automata).
Deterministic, offline, hallucination-proof, current with the latest patch. High-confidence
title hit → fetch that page directly, skip search. Also validates Phase 3's rewrites (a
rewrite that maps to a real title is trustworthy). Depends on building the SQLite page cache
(roadmap §6, not yet built).

## Decisions & trade-offs
- **Measure before the big build** — Phase 2 precedes Phase 3 deliberately; if retrieval
  already hits ~90% and answers still fail, the culprit is 8k truncation, not search.
- **Deterministic before probabilistic** — suggestion-retry and the title index are
  preferred where they apply because they can't hallucinate; the LLM rewrite fills the
  vocabulary-gap cases they can't reach.
- **Lazy vs eager LLM rewrite** is a genuine architectural fork (latency/cost/silent-failure)
  → formalized in [[2026-07-07_llm-query-rewrite-in-retrieval]].

## Status log
- 2026-07-07 — created; captured as a 4-phase roadmap (todo) from the retrieval code review
  in `docs/ai-workflow.html` and the senior-dev discussion.
- 2026-07-07 — Phase 1 (suggestion-retry) + Phase 2 (opt-in retrieval trace) implemented and
  `/check`-green (110 Rust tests, tsc clean). `search_full` + `parse_search_suggestion` in
  `wiki/search.rs`; suggestion-then-simplify zero-hit recovery + `trace_retrieval`
  (`WIKILENS_TRACE_RETRIEVAL`) in `commands.rs`; `docs/ai-workflow.html` Stage 1 refreshed.
- 2026-07-07 — Phases 3 & 4 also implemented, on an explicit "execute the full roadmap" directive
  (ahead of the Phase 2 eval the plan originally gated them on). **Phase 3:** lazy LLM query-rewrite
  (`llm::rewrite_query` — non-streaming, strict-JSON out, tolerant parser; kill-switch
  `WIKILENS_QUERY_REWRITE`). **Phase 4:** local title index (`wiki/titles.rs` — bounded `list=allpages`
  walk + hand-rolled Jaro-Winkler fuzzy match, session-cached in `AppState::title_cache`; kill-switch
  `WIKILENS_TITLE_INDEX`), wired as the deterministic rung *before* the LLM rewrite in the zero-hit
  ladder (suggestion → simplify → title-index → rewrite). `/check`-green (122 Rust tests, tsc clean),
  adversarially reviewed. **Caveats:** the new network paths (allpages fetch, rewrite call) are
  offline-tested + fallback-safe but NOT yet live-validated (no keys/live wikis in the dev env); Phase 4
  cache is in-memory per session — cross-restart SQLite persistence and threshold/lazy-vs-eager tuning
  against real Phase 2 eval data remain the follow-ups.
- 2026-07-07 — **Pivoted to eager rewrite + merge** after live traces (`WIKILENS_TRACE_RETRIEVAL`)
  showed the dominant failure is **wrong-but-nonzero** hits, not zero-hit — a chatty question matches
  tangential pages ("Version History", "Pans"), and the lazy ladder (gated on `titles.is_empty()`) is
  structurally blind to that. `run_ask` now runs the raw search + `llm::rewrite_query` **concurrently**
  (`futures_util::future::join`), searches the top `REWRITE_SEARCH_LIMIT`=2 rewrite candidates, and
  merges via `merge_hits` (consensus → entity → keyword, deduped, top 4). The deterministic
  suggestion/simplify/title-index net now fires **only when the merge is empty**; the old lazy-rewrite
  rung was removed. `/check`-green (128 Rust tests). Because the rewrite now runs on every ask, the
  `wikilens.rewrite` trace line self-reports whether DeepSeek's non-streaming call actually returns
  candidates — **pending that live confirmation** (`stardew "…wine"` reworded to a wrong-hit, so the
  diagnostic hadn't printed yet). `docs/ai-workflow.html` Stage 1 rewrite deferred until eager is
  validated live. Trade-off accepted: eager adds a model call + up to 2 searches per ask (kill-switch
  `WIKILENS_QUERY_REWRITE`, or lower `REWRITE_SEARCH_LIMIT`).
- 2026-07-07 — **Validated live.** Both DeepSeek v4 models (flash + pro) turned out to be reasoning
  models — the rewrite's answer landed in `reasoning_content`, `content` came back empty, and it took
  5–13s — so decoupled the rewrite to a fast model on any provider (`WIKILENS_REWRITE_PROVIDER` +
  `WIKILENS_REWRITE_MODEL`, cross-provider, key required; see the decision doc). With the rewrite pinned
  to **Claude Haiku** while answering with **DeepSeek**, the eager flow works end-to-end: the rambling
  question *"that profitable winter thing you sell… i think it's wine"* → rewrite `["wine","preserves",
  "Winter Seeds"]` → merged titles `["Wine","Wine Table","Preserves Jar","Preserves Jar Productivity"]`
  (zero junk) → a real, grounded answer, in ~1.7s. The wrong-hit problem is solved. `docs/ai-workflow.html`
  Stage 1 updated to the eager flow. Open follow-ups: cross-restart SQLite persistence for the title
  index (roadmap §6), threshold/`REWRITE_SEARCH_LIMIT` tuning, and a real ~40-question eval pass.
