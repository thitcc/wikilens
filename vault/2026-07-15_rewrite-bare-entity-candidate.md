---
title: Guarantee a bare-entity rewrite candidate — intent words poison AND-semantics retrieval
type: plan
status: active
created: 2026-07-15
updated: 2026-07-15
tags: [llm, rag, wiki]
related: ["[[2026-07-10_rewrite-prompt-reword]]", "[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-04_query-preprocessing-zero-hit-retry]]", "[[2026-07-04_golden-query-retrieval-tests]]", "[[2026-07-10_merge-raw-hit-guarantee]]"]
commit:
---

# Guarantee a bare-entity rewrite candidate — intent words poison AND-semantics retrieval

## Context / problem

Live failure (2026-07-15, Warframe): "best strategy to get archon shards" answered
"I don't have information about Archon Shards in the provided wiki excerpts" with
sources Credits / Endo / Affinity / Nihil. Reproduced against wiki.warframe.com
(CirrusSearch — `srwhat=text` is a no-op there; identical results with/without):

- The Archon Shard page (43k chars of wikitext) contains **zero** occurrences of
  "strategy", "farming", or "farm". CirrusSearch ANDs every query term, so any
  query carrying one of those words **excludes the canonical page outright**.
- Raw query `strategy archon shards` ("strategy" survives `preprocess_query` —
  it's not in `STOPWORDS`, unlike "best"/"get"/"to") → Nihil, Archon Amar,
  Eidolon Gantulyst, Archon Boreal (boss pages with Strategy sections).
- The Haiku rewrite made it *worse*, not better: `Archon Shard farming` (added an
  intent word) → Credits, Endo, Affinity, Stela; `Zariman void cascade Archon
  Shard` (five AND'd terms) → Drifter/Quotes, Zariman Ten Zero, Helminth/Quotes.
- `Archon Shard` alone ranks the right page **#1** — the winning query was one
  word away, and no stage ever tried it.
- All three searches returned wrong-but-nonzero hits, so the zero-hit recovery
  ladder never fired; `merge_hits` merged junk with junk (no consensus → rewrite
  hits first, then raw[0]'s reserved slot = exactly the four fetched pages). The
  answer model then correctly refused to invent — retrieval failed, RAG
  discipline held.

Same root gap the non-strict Abigail-gift golden case documents (AND semantics +
an abstract term excludes the canonical entity page), but starker: the raw query
*and both* rewrite candidates all carried a poison word.

## Goal / non-goals

- Goal: the rewrite always yields one candidate that is the **bare entity name**
  from the question — the subject as the wiki would title it, stripped of
  action/intent words (e.g. "Archon Shard", not "Archon Shard farming"). Under
  `merge_hits` that candidate's top hit lands as consensus (rank #1) or the lead
  rewrite-only hit.
- Goal: `preprocess_query` drops the *safely generic* intent words the raw query
  can carry (candidates: "strategy", "strategies", "obtain", "acquire") so the
  raw hit list stops being poisoned too.
- Goal: a warframe golden case pinning this exact question, anchored on the
  stopword fix — the golden runner is deterministic (no LLM rewrite), so only
  the `preprocess_query` change can carry it.
- Non-goal: "farm"/"farming"/"guide"/"tips" as stopwords — they are real
  entities/pages on covered wikis (Stardew's Farming skill, Terraria's Guide
  NPC). Every stopword is global across all wikis; each addition must be vetted
  against covered games' titles.
- Non-goal: no `merge_hits` change, no JSON-contract change
  (`parse_rewrite_queries` untouched), no wrong-but-nonzero recovery ladder
  (that's a bigger, separate idea — the ladder stays zero-hit-only).

## Approach

1. `src-tauri/src/llm.rs` — `REWRITE_SYSTEM_PROMPT`: require candidate #1 to be
   the bare subject entity (proper noun / noun phrase, no action or intent
   words); candidate #2 optional — the wiki's own name for the paraphrased
   concept. Keep the compact-JSON contract, the parallel-raw-search framing, and
   the keep-correct-nouns guidance from [[2026-07-10_rewrite-prompt-reword]].
2. `src-tauri/src/wiki/search.rs` — `STOPWORDS`: add the vetted intent words;
   extend the `preprocess_*` unit tests ("best strategy to get archon shards" →
   "archon shards").
3. `src-tauri/src/wiki/mod.rs` — `GOLDEN_CASES`: `("warframe", "best strategy to
   get archon shards", &["Archon Shard"], strict)` — passes via the stopword fix
   (preprocess → "archon shards" → live #1 hit, verified 2026-07-15).
4. Live sanity of the new prompt (mirroring the 2026-07-10 reword's check):
   direct API call with the exact new prompt — archon-shards question must yield
   "Archon Shard" as candidate #1; the Abigail case must still keep "Abigail";
   the typo case must still correct it.
5. End-to-end: `WIKILENS_DEBUG=1` ask of the exact question; expect Archon Shard
   in the fetched pages and a real answer.

## Decisions & trade-offs

- Prompt-level bare entity vs. a Rust-side intent-word stripper on candidates:
  the model already holds the question and game context — instruct it rather
  than maintain a second word list that could mangle legitimate multi-word
  titles. The deterministic side gets the (small, vetted) `STOPWORDS` additions
  instead.
- Stopwords are shared cross-game, so every addition trades recall globally:
  dropping "strategy" loses text-matches on guide/boss-section pages, which are
  rarely the sought entity — acceptable; "farming" is not (Stardew). The vet
  rule: check each candidate word against covered wikis' page titles before
  adding.
- The golden case rides the stopword fix, not the prompt change — the golden
  runner deliberately never calls the LLM. That split is a feature: both the
  deterministic and the LLM stage get healthier, and the pinned case survives
  rewrite-model swaps.

## Status log

- 2026-07-15 — created from the archon-shards live-failure diagnosis (debug
  table + live reproduction against wiki.warframe.com).
