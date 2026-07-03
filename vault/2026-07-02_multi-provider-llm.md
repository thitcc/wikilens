---
title: Support multiple LLM providers (Anthropic, DeepSeek, OpenRouter)
type: decision
status: done
created: 2026-07-02
updated: 2026-07-02
tags: [llm, rag, security]
related:
  - "[[2026-07-02_scaffold]]"
  - "[[2026-07-02_wikitext-extraction]]"
commit: 85aca6c
---

# Support multiple LLM providers (Anthropic, DeepSeek, OpenRouter)

## Context

The scaffold talked to a single LLM: Anthropic, with the key read from
`ANTHROPIC_API_KEY` (OS env only — no `.env` support). The user wanted the starter
version to support **three** providers (Anthropic, DeepSeek, OpenRouter) and asked
how keys should be supplied (a root `.env`, or something else). There are really
only **two wire protocols** in play: Anthropic's native Messages API, and the
OpenAI-compatible Chat Completions API — which **DeepSeek and OpenRouter share
byte-for-byte** (`Authorization: Bearer`, `{messages:[system,user]}` body,
`choices[0].delta.content` SSE with `data: [DONE]` and `:` keep-alive lines).

Locked decisions (from the user): provider chosen via a **dropdown in the overlay
header** (persisted to `localStorage`, passed to `ask` as `providerId`); keys via
**`.env` (dotenvy) OR OS env**; model = **per-provider default, overridable via
`WIKILENS_<PROVIDER>_MODEL`** (no model UI in the starter).

## Decision

Add a **data-driven provider registry** (`src-tauri/src/providers.rs`, mirroring
`wiki/games.rs`) plus **one shared SSE streaming loop** in `llm.rs`; only
request-building and per-line parsing branch on a 2-variant `ProviderKind`
(`Anthropic` vs `OpenAiCompatible`) — a closed enum, not a trait. Keys load via
`dotenvy::dotenv().ok()` at the top of `run()` (root `.env` for `tauri dev`) with
**OS env taking precedence**; read Rust-side only, never logged, never sent to the
frontend. Model resolves to the registry `default_model` unless
`WIKILENS_<PROVIDER>_MODEL` overrides it.

## Consequences

- **Good:** adding a provider = one `Provider` entry in `providers.rs`; API keys
  never cross IPC (`ProviderInfo` is `{id,name}` only, doesn't report which keys
  are set); model drift (DeepSeek v4, Haiku versions) handled by env override with
  no code change; missing key errors instantly (resolved before the first
  `ask://status` emit) instead of hanging on "Searching…".
- **Cost / bad:** two SSE parse paths (`parse_anthropic_sse_line` /
  `parse_openai_sse_line`) to keep correct — OpenAI-compatible needs to handle
  `:` comments, `[DONE]`, null `delta.content`, usage-only chunks, and mid-stream
  errors on HTTP 200. `.env` is unreliable for the **packaged** app (unpredictable
  cwd) → shipped installs must use OS env vars.
- **Follow-ups:** none pending; model UI intentionally deferred.

## Alternatives considered

- **A trait (`dyn Provider`) instead of the enum** — rejected: the provider set is
  closed and small, the data is `Copy`, and DeepSeek/OpenRouter are the *same*
  protocol, so dynamic dispatch buys nothing.
- **A model-selection UI in the panel** — deferred: the `WIKILENS_*_MODEL` env
  override covers model drift without shipping UI in the starter.
