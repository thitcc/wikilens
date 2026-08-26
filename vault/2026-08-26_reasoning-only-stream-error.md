---
title: Surface an answer stream that was all reasoning as an error
type: plan
status: idea
created: 2026-08-26
updated: 2026-08-26
tags: [llm, rust]
related: ["[[2026-08-24_local-ai-mode]]", "[[2026-08-26_local-thinking-off-switch]]", "[[2026-07-10_reasoning-skip-and-capability-tags]]"]
commit:
---

# Surface an answer stream that was all reasoning as an error

## Context / problem
The first Local AI test on the dev rig (Ollama 0.32.15, Qwen3.8-27B) produced
an ask whose stream carried 1024 output tokens and **zero content**: the model
spent the whole `MAX_TOKENS` budget on hidden reasoning, streamed as
`delta.reasoning` chunks that `parse_openai_sse_line` (`llm.rs`) correctly
ignores. Today that exit is invisible: `answer_streaming` returns
`Ok(StreamedAnswer { text: "" })`, `run_ask` records a blank history entry
and returns `Ok(AskResult { answer: "", sources })`, the UI shows the
onboarding placeholder above a live Sources list, and the debug table prints
"answered · no first token".

This is not a rig quirk. Over `/v1/chat/completions`, Ollama turns thinking
**on** for every model whose parser supports it whenever the client omits
`reasoning_effort` (`openai.go` maps `""` → nil; `routes.go` maps nil → true,
gated on `CapabilityThinking`). That covers every 2026 library model
(`gemma4:*`, `qwen3.5:*`, `qwen3.8:*`), and there is no server-side off
switch (no Modelfile `PARAMETER think`, no `OLLAMA_*` var; a derived
`TEMPLATE` is ignored for tags that ship a renderer/parser). Any open-source
user pointing WikiLens at a current Ollama model can hit this.

## Goal / non-goals
- Goal: turn the invisible empty-answer exit into one actionable error line,
  **protocol-level and model-agnostic** — no model names, no server names.
- Goal: keep the existing contract for everything else: reasoning deltas stay
  ignored (never shown, never stored), history skips the failed ask by
  construction (errors never reach the record tail).
- Non-goal: sending `reasoning_effort`/`think` on requests — that is a
  separate, opt-in decision ([[2026-08-26_local-thinking-off-switch]]).
- Non-goal: raising `MAX_TOKENS` — a request parameter can't react to deltas
  that arrive after it was sent, a global raise touches cloud cost/latency,
  and a Local-only raise hides the symptom while the answer still arrives
  after the entire think.

## Approach
- In `answer_streaming` (`llm.rs`), track two signals while draining the SSE
  body: `saw_reasoning` — any non-empty `delta.reasoning` **or**
  `delta.reasoning_content` (Ollama emits the former; llama.cpp the latter;
  vLLM renamed `reasoning_content` → `reasoning`; LM Studio moved to
  `delta.reasoning` at 0.3.23 — both keys are needed) — and
  `finish_reason == "length"` (currently unread; catches servers that keep
  thinking inside `<think>` tags in `content` or don't stream reasoning at
  all).
- On `[DONE]`/body close with an empty answer and either signal set, return
  an error instead of `Ok("")`: "The model spent its whole answer budget
  thinking and never wrote an answer. Turn thinking off on your server or
  pick a non-thinking model." Reuse an existing `AppError` variant (`Llm` /
  `LocalMode` / `Parse`) so `error.rs` stays untouched; the error flows
  through `streamed?` in `commands.rs`, the debug row's error branch, and the
  App error box with zero frontend change.
- Tests: 2–3 `parse_openai_sse_line` units (reasoning delta → the new signal;
  `finish_reason: "length"`), one wiremock stream in `llm.rs` `http_tests`
  (reasoning-only body → the friendly error; `test_support.rs`'s
  `openai_delta` needs a sibling helper), and the existing
  `openai_ignores_role_only_and_null_content` stays green.
- One README sentence in the Local AI section: a thinking model can burn the
  whole answer budget; turn thinking off on the server or pick a
  non-thinking tag.
- Delivery: its own PR off `main` after #79 merges — never into the reviewed
  Local AI PR.

## Decisions & trade-offs
- Protocol-level detection over id heuristics: the vault's earlier decision
  ([[2026-07-10_reasoning-skip-and-capability-tags]]) rejects fuzzy model-id
  matching; this plan keeps that line — it reads what the stream says, not
  what the model is called.
- The rewrite path already names this failure mode in its doc comment
  (`REWRITE_MAX_TOKENS`) and the breaker absorbs it; the answer path had no
  equivalent, which is the gap this closes.

## Status log
- 2026-08-26 — created as an idea from the Local AI rig test (empty answer,
  8778 in / 1024 out, "no first token"); root cause and the four options
  written up in the temporary root doc `ia-local-no-rig.html` (untracked).
