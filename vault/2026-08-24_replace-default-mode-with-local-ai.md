---
title: Replace Default mode with a Local AI mode
type: decision
status: done
created: 2026-08-24
updated: 2026-08-25
tags: [llm, rust, frontend]
related: ["[[2026-08-24_local-ai-mode]]", "[[2026-07-26_default-mode-and-byo-api-keys]]", "[[2026-07-29_keys-are-not-a-mode-choice]]", "[[2026-08-04_default-mode-subscription]]"]
commit: [029a8ad, bdcf6e1]
---

# Replace Default mode with a Local AI mode

## Context

Default mode ([[2026-07-26_default-mode-and-byo-api-keys]]) was built as the
MVP for a hosted tier: one env-configured target (`WIKILENS_DEFAULT_*`) with
the vendor hidden, swappable for "our proxy" later. The subscription that
would have justified it ([[2026-08-04_default-mode-subscription]]) never left
`status: idea`, and the repo is about to go open source at 0.2.0 — a public
build has no one to supply the backing model, so a packaged install's Default
mode is dead on arrival. Meanwhile the players the mode was meant to serve
(no cloud key, no billing errand) are the exact audience running local LLM
servers — Ollama, LM Studio, llama.cpp, vLLM — which all speak the
OpenAI-compatible wire protocol Default mode's `openai` branch already
carries. Keeping three modes (Built In + Custom + Local) would preserve a
mode that cannot work for anyone but the original developer.

## Decision

Remove Default mode entirely and ship a Local AI mode in its place: exactly
two modes — Custom API and Local AI — where Local AI is a user-editable
OpenAI-compatible base URL (default Ollama, `http://localhost:11434/v1`) with
an optional API key and a manual "Reads images" toggle, all configured from
Settings instead of env.

## Consequences

- Good: every install can answer questions without a cloud key; the whole
  impure env-sensing half of the settings surface (`DefaultModeInfo`,
  `sense_default_mode`, first-launch auto-sense) deletes; the planted-`.env`
  config-injection surface from the 2026-08-15 security review retires with
  the env family; the subscription explainer leaves the repo before it goes
  public.
- Cost / bad: existing Built-In installs silently land in Custom (stored
  `"default"` becomes unrecognized — a stderr notice names the removal); the
  eval suite loses its target source and needs its own `WIKILENS_EVAL_*`
  family; a user-configurable endpoint means the "where does my question go"
  answer now lives in Settings rather than env (visible URL, user-entered —
  accepted).
- Follow-ups: [[2026-08-04_default-mode-subscription]] flips to `dropped`;
  open-source prep (LICENSE, CONTRIBUTING, `"private": true`) is separate
  work before the 0.2.0 release PR.

## Alternatives considered

- Keep Built In alongside (three modes) — rejected: a public repo has no
  built-in credential source, so the option would render as permanently "Not
  set up here" for every user except the original dev; a dead stepper option
  is UI debt, and the hosted tier it kept the seam warm for is dropped.
- Ship the subscription instead ([[2026-08-04_default-mode-subscription]]) —
  rejected for 0.2.0: no billing/proxy infrastructure exists, and open-sourcing
  the client is the current goal; the idea doc closes as dropped.
- Local server as a fourth registry provider inside Custom mode — rejected:
  the `providers.rs` registry is deliberately compile-time-static
  (`&'static str` endpoints), while a local endpoint is runtime-configurable
  per user; wedging a mutable URL into the registry breaks that invariant,
  and the Settings UI for URL/key/vision needs mode-level rows anyway. A
  mode-level seam (the Default resolver's shape) fits both.
