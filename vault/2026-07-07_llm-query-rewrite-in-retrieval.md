---
title: LLM query rewrite in the retrieval path — lazy-first, and whether to add an LLM call to search
type: decision
status: active
created: 2026-07-07
updated: 2026-07-07
tags: [rag, llm, wiki, rust]
related: ["[[2026-07-07_retrieval-quality-improvements]]", "[[2026-07-04_query-preprocessing-zero-hit-retry]]", "[[2026-07-04_golden-query-retrieval-tests]]", "[[2026-07-02_multi-provider-llm]]"]
commit:
---

# LLM query rewrite in the retrieval path — lazy-first, and whether to add an LLM call to search

## Context
Rule-based keyword extraction (`preprocess_query`/`simplify_query`, `wiki/search.rs`) can't
fix spelling or vocabulary gaps — a misspelled or paraphrased query zeroes out and the one
retry re-sends the same miss. An LLM query-rewrite is the standard RAG remedy and the single
biggest lever, but it adds a model call to the retrieval path: latency (~300–600ms), cost,
a key dependency, and — most importantly — a new *silent* failure mode (a confident wrong
correction retrieves wrong pages, and the grounded answerer then answers confidently from
them). Today's failure is *loud* (0 hits → "rephrase"), which we don't want to lose. The
`2026-07-04_query-preprocessing-zero-hit-retry` plan explicitly deferred this as "a real
fork → decision doc first"; this is that doc.

## Decision
Proposed (pending Phase 2 eval in [[2026-07-07_retrieval-quality-improvements]]): **add an
LLM query-rewrite, but lazily** — invoke it only on the zero/low-hit retry path (after the
raw query and the free `searchinfo.suggestion` retry both fail), reusing the existing
provider/streaming infra (`providers.rs`, `llm.rs`). Always **merge** rewritten hits with
raw hits, **hard-fall-back** to the raw query on any failure, and **keep sending the
original question** to the answering model. Escalate to eager (every-query) rewriting only
if Phase 2 data shows wrong-hit-nonzero cases dominate.

**Update (2026-07-07):** implemented lazy-first ahead of the eval, on an explicit "execute the
full roadmap" directive — `llm::rewrite_query`, wired as the final zero-hit rung behind the
`WIKILENS_QUERY_REWRITE` kill-switch, after the deterministic local title index
([[2026-07-07_retrieval-quality-improvements]]). Offline-tested + fallback-safe but not yet
live-validated. The lazy→eager escalation remains gated on Phase 2 eval data.

**Update 2 (2026-07-07):** the Phase 2 traces came in and showed **wrong-hit-nonzero dominates**
(a chatty query matches tangential pages, which the lazy zero-hit rung never reaches), so escalated
to **eager**: the rewrite runs on every ask, concurrently with the raw search, and its hits are
merged (consensus → entity → keyword) rather than only rescuing an empty result. The deterministic
net (suggestion/simplify/title-index) now fires only when the merge is empty. Cost: a model call +
up to 2 searches per ask (kill-switch retained). Still pending live confirmation that DeepSeek's
non-streaming rewrite call actually returns candidates — the eager change makes the `wikilens.rewrite`
trace line print on every query, which will confirm or deny it on the next run.

**Update 3 (2026-07-07):** live diagnostic confirmed `deepseek-v4-flash` is a **reasoning model** —
the rewrite's answer went to `reasoning_content`, `content` came back empty (so the parse yielded
nothing), and it took ~13s. **Decoupled the rewrite from the answer model:** it now uses a fast
non-reasoning model via `WIKILENS_REWRITE_MODEL` when set, else the answer model; the token cap was
tightened to 256 so a mis-configured reasoning model fails fast instead of reasoning for seconds.
Same-provider only for now (the rewrite reuses the answer provider + key). A reasoning model as the
answer model is fine — the rewrite just needs its own fast one.

**Update 4 (2026-07-07):** both DeepSeek v4 models (flash + pro) are reasoning-only, so a same-provider
swap couldn't help — made the rewrite **cross-provider** (`WIKILENS_REWRITE_PROVIDER` + its own key +
`WIKILENS_REWRITE_MODEL`, falling back to the answer provider). **Validated:** answering with DeepSeek
and rewriting with Claude Haiku, a rambling "…i think it's wine" resolved to `["wine","preserves",
"Winter Seeds"]` → merged pages `Wine · Wine Table · Preserves Jar` → a real grounded answer in ~1.7s.
Decision **accepted & shipped** (eager, cross-provider). Caveat: requires a key for a fast non-reasoning
provider; a DeepSeek-only user gets the deterministic net but no rewrite.

## Consequences
- **Good:** fixes typos, vocabulary gaps, and stopword-name collisions; happy path stays
  fast/free; reuses infra with no new dependency; the loud-failure UX survives via fallback.
- **Cost / bad:** an extra model round-trip and cost on the retry path; a confident-wrong
  correction can fail silently (worst on post-cutoff patch content); one more moving part in
  the trust boundary.
- **Follow-ups:** validate rewrites against the Phase 4 local title index (kills the
  hallucination risk); reconsider lazy→eager once the eval quantifies wrong-hit cases.

## Alternatives considered
- **Eager rewrite (every query)** — catches vocabulary-gap wrong-hit cases lazy misses, but
  puts latency/cost on every ask and maximizes exposure to confident-wrong corrections.
  Deferred to a data-gated escalation, not the starting point.
- **Stay purely rule-based / tune stopwords** — free and deterministic but structurally
  cannot fix spelling or paraphrase (the current dead-end).
- **Local title index only, skip the LLM** — deterministic and hallucination-proof, but
  can't bridge pure vocabulary gaps ("thing that revives you" → "Last Gasp") and needs the
  unbuilt SQLite cache; treated as the complementary durable layer, not a replacement.
- **Semantic/vector retrieval** — highest quality, biggest lift; out of scope until caching
  exists.
