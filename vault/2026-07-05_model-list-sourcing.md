---
title: Model-list sourcing — hybrid live fetch with curated fallback
type: decision
status: active
created: 2026-07-05
updated: 2026-07-05
tags: [llm, rust]
related: ["[[2026-07-05_model-picker-menu]]", "[[2026-07-02_multi-provider-llm]]"]
---

# Model-list sourcing — hybrid live fetch with curated fallback

## Context
The model-picker menu ([[2026-07-05_model-picker-menu]]) needs per-provider
model lists, which the app has never had. Live probes (2026-07-05):
Anthropic `GET /v1/models` and DeepSeek `GET /models` return 401 without an
API key; OpenRouter `GET /api/v1/models` is public (200, keyless), carries
400+ models at ~1–2 MB raw (~150–300 KB gzipped), and churns often enough
that OpenRouter publishes an RSS feed for additions. DeepSeek deprecates its
`deepseek-chat` alias — currently our `default_model` — on 2026-07-24. No
provider documents a cache TTL. All HTTP must stay Rust-side (the webview has
no network permissions).

## Decision
Fetch each provider's model list live from Rust (`list_models` command,
per-provider parsers normalizing to `{id, label}`, 8s timeout, gzip), cache
**live results only** for the session in AppState, and degrade to a small
curated hardcoded list (2–4 entries per provider in `providers.rs`) whenever
the key is missing, the fetch fails, or the list comes back empty — marked
`source: fallback` so the UI can show a muted "offline list" note. Keyless
providers whose endpoint requires auth skip the doomed request entirely.
Decided with the user on 2026-07-05 (options presented; hybrid chosen).

## Consequences
- Good: the menu always renders something selectable — no keys, no network,
  provider outage, all covered; live lists never rot in code; OpenRouter's
  400+ catalog is browsable without us maintaining it.
- Good: never caching fallbacks means a transient failure self-heals on the
  next menu open; session-only caching needs no invalidation logic (keys are
  read at process start, catalogs change on release cadence).
- Cost: curated fallback lists are code that can go stale — mitigated by the
  `default_model_is_in_curated_list` invariant test and by treating them as
  a degraded mode (tiny, current, not a catalog).
- Cost: two parsers to maintain (Anthropic shape vs OpenAI-style shape); the
  fixture tests pin both.

## Alternatives considered
- **Live fetch only** — simplest data flow, but Anthropic/DeepSeek 401
  without keys: a keyless install (or any offline session) gets empty groups
  in a menu whose whole point is choosing. Rejected.
- **Hardcoded lists only** — instant and offline-proof, but rots on every
  provider release; the DeepSeek alias deprecation landing *this month*
  proves the failure mode, and hand-curating OpenRouter's 400+ catalog is
  not maintainable. Rejected.
- **Persistent (disk) cache with TTL** — more machinery (no store plugin in
  the app today) for marginal gain over a session cache given the lists are
  fetched lazily and are small after Rust-side trimming. Rejected for v1;
  the roadmap's SQLite cache could absorb this later if wanted.
