---
title: A Local AI "thinking off" switch that sends reasoning_effort none
type: plan
status: idea
created: 2026-08-26
updated: 2026-08-26
tags: [llm, rust, frontend]
related: ["[[2026-08-24_local-ai-mode]]", "[[2026-08-26_reasoning-only-stream-error]]"]
commit:
---

# A Local AI "thinking off" switch that sends reasoning_effort none

## Context / problem
Ollama's OpenAI-compatible endpoint enables thinking by default for every
model whose parser supports it when the request carries no
`reasoning_effort` — and every 2026 library model does (`gemma4:*`,
`qwen3.5:*`, `qwen3.8:*`), even where the vendor documents thinking as off
by default. There is no server-side lever: no Modelfile `PARAMETER think`
(Ollama PRs #14108/#14630 unmerged as of 2026-08), no `OLLAMA_*` variable,
and a derived `TEMPLATE` is ignored for tags with a renderer/parser. The
**only** switch is the request field `reasoning_effort: "none"`, honored by
Ollama, llama.cpp and vLLM (vLLM's Qwen3 path also needs
`chat_template_kwargs.enable_thinking=false`; LM Studio: unverified).

WikiLens deliberately stays model-agnostic (no per-model tailoring, the app
is going open source at 0.2.0), so the field cannot be sent unconditionally:
it would override players who want their hybrid model thinking, and Ollama's
reaction to the field on a model without the thinking capability is
unverified. The fit is a **mode-level, opt-in** setting.

## Goal / non-goals
- Goal: a "Thinking" switch on the Local AI section of Settings (default off
  = nothing changes in the request); when the player turns it on, Local-mode
  answer and rewrite requests carry `reasoning_effort: "none"`.
- Goal: keep the rule that a stored setting is storage only — the switch
  never changes which model answers ([[2026-07-29_keys-are-not-a-mode-choice]]).
- Non-goal: any model-name heuristic; any default other than "send nothing".
- Non-goal: Custom-mode providers — cloud reasoning models keep their
  `:thinking`-id contract.

## Approach
- Settings: `LocalAi` gains a `thinking_off: bool` (name to settle) persisted
  next to `vision`; a `set_local_thinking_off` command returning fresh
  `SettingsInfo`; `LocalModeInfo` carries it (pin the field set in
  `config_guardrails.rs`).
- Request build: `LlmTarget` (or `AskTargets`) carries the flag; the
  OpenAI-compatible body builder in `llm.rs` adds `reasoning_effort: "none"`
  only when set — answer and rewrite alike (the rewrite is where a thinker
  fails first: 256 tokens, 4 s).
- UI: a row on the Local AI nest beside the vision eye (same padlock/eye
  recipe), copy along the lines of "Thinking off — for models that reason
  before answering (Ollama turns it on by default)". Proof sheet for the row
  anatomy if there's a visual fork.
- Live probe first (execution of this plan starts with it): a
  `/v1/chat/completions` POST with `reasoning_effort: "none"` against a
  non-thinking tag (`qwen3-vl:8b-instruct`) and against a thinking one — does
  Ollama error, ignore, or honor? Record the result in the status log.
- Docs: README Local AI section gets the switch and the Ollama-default note.

## Decisions & trade-offs
- Opt-in switch vs unconditional field: unconditional would be tailoring by
  another name and reaches into models the player didn't ask to reconfigure;
  opt-in keeps the default request byte-identical to today's.
- Pairs with [[2026-08-26_reasoning-only-stream-error]]: that safety net
  tells the player *why* the answer was empty; this switch is what they'd
  flip in response. The net ships first (one file, no UI); this waits for
  real use to show it's needed.

## Status log
- 2026-08-26 — created as an idea from the Local AI rig test; the
  Ollama-forces-thinking finding is documented in the temporary root doc
  `ia-local-no-rig.html` (untracked) with primary sources (`openai.go`,
  `routes.go`, `parsers.go`, docs.ollama.com).
- 2026-08-26 — probe run on the rig (Ollama 0.32.15): `reasoning_effort:
  "none"` on `/v1/chat/completions` against a non-thinking tag
  (`qwen3-vl:8b-instruct`, capabilities `completion, vision, tools`) returned
  HTTP 200 and a normal answer — Ollama ignores the field where thinking is
  unsupported. Earlier in the same session the field disabled thinking on the
  Qwen3.8-27B HF import. Both directions are safe on Ollama; LM Studio still
  unverified.
