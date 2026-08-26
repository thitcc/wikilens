---
title: Local AI mode replaces Default mode
type: plan
status: done
created: 2026-08-24
updated: 2026-08-26
tags: [llm, rust, frontend]
related: ["[[2026-08-24_replace-default-mode-with-local-ai]]", "[[2026-07-26_default-mode-and-byo-api-keys]]", "[[2026-07-29_keys-are-not-a-mode-choice]]", "[[2026-08-04_default-mode-subscription]]", "[[2026-08-26_reasoning-only-stream-error]]", "[[2026-08-26_local-thinking-off-switch]]"]
commit: [d9a897f, d9bc2b6, 156130e, 6b0a65b, 029a8ad, bdcf6e1, 171cf3b, 5237c60, 451984c, 08d0f02]
---

# Local AI mode replaces Default mode

## Context / problem

Ahead of open-sourcing the repo at v0.2.0, Default mode ("Built In" — one
answer/rewrite target resolved from the `WIKILENS_DEFAULT_*` env family in
`target.rs`) stops making sense: it was the MVP for a hosted subscription tier,
and a public build has no one to subsidize it. What players without a cloud API
key actually have in 2026 is a locally running LLM server — Ollama, LM Studio,
llama.cpp server, vLLM — all speaking the OpenAI-compatible wire protocol. The
mode fork itself is recorded in [[2026-08-24_replace-default-mode-with-local-ai]].

## Goal / non-goals

- Goal: exactly two modes in Settings → Answers — **Custom API** and **Local
  AI**. Local AI talks to a user-editable OpenAI-compatible base URL (default
  `http://localhost:11434/v1`, Ollama), with an optional API key (LM
  Studio/llama.cpp `--api-key`) and a "Reads images" toggle standing in for the
  capability metadata local servers don't expose. Every `WIKILENS_DEFAULT_*`
  consumer is removed or retargeted (the eval suite gets its own
  `WIKILENS_EVAL_*` family), and the subscription explainer leaves the repo
  before it goes public.
- Non-goal: the 0.2.0 bump (release PR only, per release-scoped SemVer) and
  the open-source prep itself (LICENSE, CONTRIBUTING, `"private": true`,
  install-for-users README) — separate work after this lands.
- Non-goal: Anthropic-protocol local targets, presets per runtime, model
  capability probing, and any change to `http.rs` timeouts (the 60s read gap
  on cold model loads and the 4s rewrite timeout are documented, not tuned).

## Approach

Local AI is a first-class `Mode` — the architectural heir of the Default
resolver, with one structural simplification: Default's config came from env
(impure, re-sensed per command), Local's lives in the `SettingsStore`, so the
whole `sense_default_mode` half deletes.

1. `llm.rs` omits the `Authorization` header when the target's key is empty
   (keyless local servers); wiremock-pinned.
2. `models.rs` splits a `fetch_models_at` core that takes a runtime endpoint;
   `list_models` accepts the `"local"` id before the registry lookup, passing
   the stored local key when present. No session cache, no curated fallback —
   a down server surfaces "is it running?" copy.
3. `target.rs` gains `normalize_local_base_url` (scheme-less pastes get
   `http://`, bare hosts get `/v1`, custom paths kept verbatim) and
   `resolve_local_targets` (pure; answer == rewrite target; the URL may appear
   in error copy — user-entered config, not secret).
4. `settings.rs` stores `local: { base_url, vision }` (`base_url: None` = the
   baked Ollama default, so Local is always nominally configured); `Persisted`
   goes Copy→Clone. The local API key lives in the DPAPI store under `"local"`
   via dedicated `set_local_api_key`/`remove_local_api_key` commands (the
   registry-shaped `Vec<KeyStatus>` return of `set_api_key` can't carry it).
5. The atomic backend cut: `Mode { Custom, Local }`, the env resolver /
   `DefaultModeInfo` / first-launch auto-sense delete, `ask` gains the Local
   arm (reads the request's `model`, ignores `provider_id`), history records
   the local model id, `legacy_env_notices` gains all six `WIKILENS_DEFAULT_*`
   vars. Stored `"mode": "default"` degrades to unchosen (= Custom) with a
   removal notice.
6. The frontend cut: two-option Answers stepper, Local settings rows (server
   address, optional key, vision toggle), footer reuses ModelChip/ModelMenu
   with a synthesized "Local" group, all eight mode guards rewritten explicit.
7. Eval retarget (`loadEvalTarget`, `WIKILENS_EVAL_*`), docs sweep (README,
   smoke checklist, ai-workflow, CLAUDE.md, DESIGN.md, design.json,
   PRODUCT.md check), delete `docs/default-mode-subscription.html`.

One PR, nine commits, each green under `/check`; live smoke against a real
Ollama before merge.

## Decisions & trade-offs

- The mode fork (replace vs keep Built In alongside vs local-as-registry-
  provider) is split into [[2026-08-24_replace-default-mode-with-local-ai]].
- Configuring Local AI (URL, key, toggle) never flips the mode —
  [[2026-07-29_keys-are-not-a-mode-choice]] extended to a new mode.
- No first-launch probe of localhost: a running server auto-selecting Local
  would be the same silent mode flip the keys ADR bans. First launch lands on
  Custom.
- Existing Built-In installs land in Custom after the update (their stored
  `"default"` is unrecognized). Accepted: Default was env-configured and
  dev-only in practice; a stderr notice names the removal.
- Slow local models can trip the session rewrite breaker (4s timeout, Ollama
  idle-unload). Accepted and documented (warm-up + `OLLAMA_KEEP_ALIVE` in
  README) rather than special-cased.

## Status log

- 2026-08-24 — created; implementation starting on `feat/local-ai-mode`.
- 2026-08-25 — local-ai-rows proof sheet decided: **A2** vision toggle on the
  Local AI heading's right rail (the Position-padlock recipe — an own row was
  declined as one line too many for a held state), **B1** eye icon-button
  (accent while on; the On/Off chip and row+check voices declined), **C1**
  Save button on the address row (the shipped add-game anatomy; Enter-only
  declined as a hidden commit), **D1** "API key" + muted "Optional" rail note
  (label-embedded "(optional)" declined).
- 2026-08-25 — done: eight commits on `feat/local-ai-mode` (keyless auth →
  fetch core → resolver seam → settings store → backend cut → frontend cut →
  eval retarget → docs sweep), `/check` green throughout. Remaining pre-merge
  gate is human: the updated smoke §7 against a live Ollama.
- 2026-08-25 — adversarial review (10 verified findings) landed two more
  commits: the address/key hardening (same-URL field re-sync, one validated
  base for ask + model list, https default for routable scheme-less pastes,
  `http:/` typo repair, pasted-endpoint trimming, ghost-key re-paste copy,
  single-diagnosis menu error) and the dedup refactor (one keyLine renderer,
  one footer model surface, `fresh_settings`).
- 2026-08-26 — live smoke §7 walked on the dev rig against Ollama 0.32.15 with
  `qwen3-vl:8b-instruct`: model list + footer pick, text ask (rewrite 231 ms,
  first token 1.7 s, 9.0 s total for a 486-token answer), capture eye + image
  ask (image decoded server-side, answer 2.9 s), and a stale footer pick
  surfacing as the provider's 404 in the error box (the documented
  no-Rust-side-validation contract). Not walked: the down-server menu copy.
  The first rig test with Qwen3.8-27B exposed two follow-ups captured as idea
  docs — an answer stream that is all hidden reasoning ends as an empty
  answer ([[2026-08-26_reasoning-only-stream-error]]), and Ollama forces
  thinking on over /v1 for every 2026 library model unless the client sends
  `reasoning_effort` ([[2026-08-26_local-thinking-off-switch]]). Neither
  rides in this PR.
