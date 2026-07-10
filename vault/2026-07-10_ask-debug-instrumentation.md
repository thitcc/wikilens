---
title: Per-ask debug table — phase timings, models, and token counts behind WIKILENS_DEBUG
type: plan
status: done
created: 2026-07-10
updated: 2026-07-10
tags: [llm, rag, rust]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-07_retrieval-quality-improvements]]"]
commit: 2916c71
---

# Per-ask debug table — phase timings, models, and token counts behind WIKILENS_DEBUG

## Context / problem

The rewrite-review findings (root `FINDINGS-2026-07-10-rewrite-review.md`, kept out of the
vault) are about to be fixed step-by-step, and each fix needs before/after evidence: how long
did the rewrite take, did the candidate searches add latency, where did the ask actually spend
its time. Today `run_ask` has exactly one timer (the combined retrieval span surfaced as
`search_ms` in the `WIKILENS_TRACE_RETRIEVAL` trace), token usage is discarded at every layer
(Anthropic `message_start`/`message_delta` events are ignored, OpenAI usage chunks dropped,
`rewrite_query` reads only the text), and `fetch_pages` / `answer_streaming` are untimed —
there is no way to see live, per prompt, what each part cost.

## Goal / non-goals

- Goal: with `WIKILENS_DEBUG` set truthy, every ask prints one ASCII table to stderr —
  per-phase timings (rewrite, raw search, candidate searches, retries, fetch, answer +
  time-to-first-token), answer/rewrite model, token counts separated (rewrite vs answer,
  input vs output), the question / preprocessed query / rewrite candidates, page titles with
  char counts, and an outcome line — on **every** exit path, including errors.
- Goal: default OFF; zero new output when the flag is unset.
- Non-goal: never print the wiki text sent to the answer model (titles + char counts only),
  never keys.
- Non-goal: no frontend changes; no change to the `WIKILENS_TRACE_RETRIEVAL` output.

## Approach

1. `llm.rs`: `TokenUsage {input, output}` + `SseLine::Usage` variant; parse Anthropic
   `message_start`/`message_delta` usage and OpenAI-compatible usage chunks (both the DeepSeek
   finish-chunk shape and the spec `choices: []` shape); `build_openai_request` gains
   `stream_options: {"include_usage": true}` (both curated OpenAI-compat providers support it);
   `answer_streaming` returns `StreamedAnswer {text, usage, ttft}`; `rewrite_query` returns
   `RewriteOutcome {queries, usage}`.
2. New `debug.rs`: `DebugReport` collector — collects always, prints only when enabled, and
   prints on `Drop` so early returns and `?` propagation still emit a partial table (an
   `outcome` field set at the happy/soft-fail exits distinguishes aborted asks). Pure
   `render()` for unit tests; hand-rolled ASCII alignment, no new crate.
3. `commands.rs`: create the report after rewrite resolution; time inside the joined
   rewrite/raw-search futures (the `join` makes one outer timer undecomposable) and record
   after the join; per-stage timers on the retry ladder, fetch, and answer.
4. Docs: `.env.example` commented block + CLAUDE.md §4 bullet.

## Decisions & trade-offs

- `WIKILENS_DEBUG`, not bare `DEBUG` — matches every other backend flag and avoids npm
  tooling's `DEBUG` convention. Truthy check defaults OFF (deliberately not `stage_enabled`,
  whose default is ON).
- Print-on-Drop over explicit prints at each exit: ~4 explicit exits plus five `?` points, and
  errors are exactly where a partial table matters most.
- `stream_options` sent unconditionally, not debug-gated: one canonical request shape, no
  debug-only heisenbugs; rollback is to gate it if a gateway ever 400s on it.
- TTFT measured inside `answer_streaming` (send → first delta), not via the `on_delta`
  closure — captures connect+queue+prefill and avoids fiddly closure state.

## Status log
- 2026-07-10 — created; implementation starting (post-revert of the first hardening pass).
- 2026-07-10 — implemented: `TokenUsage`/`StreamedAnswer`/`RewriteOutcome` in `llm.rs`,
  `debug.rs` collector with golden render test, `run_ask` instrumented on every phase.
  138 offline tests green (+10 new); two new `#[ignore]` live tests
  (`*_live_streaming_reports_usage`) passed against the real Anthropic and DeepSeek
  gateways — `stream_options` accepted, usage + TTFT reported.
- 2026-07-10 — first live table on a real ask (Abigail gifts, 10.0s total) immediately
  localized the cost: rewrite 2432ms serialized into the join (raw search only 464ms),
  fetch 3842ms, and a likely finding-#8 eviction (Abigail's page missing from the
  merged top-4). Done; shipped as 2916c71.
