---
title: Default mode and bring-your-own API keys
type: plan
status: active
created: 2026-07-26
updated: 2026-07-26
tags: [llm, security, frontend, rust, tauri]
related: ["[[2026-07-26_api-key-storage-dpapi]]", "[[2026-07-02_multi-provider-llm]]", "[[2026-07-19_hotkey-config]]", "[[2026-07-05_model-list-sourcing]]"]
commit:
---

# Default mode and bring-your-own API keys

## Context / problem

Keys today come only from env vars (`ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` /
`OPENROUTER_API_KEY`), every registry provider shows in the footer model menu, and
`ProviderInfo` deliberately never reports which keys are configured. That model serves
developers, not players: there is no in-app way to set up a provider, and no way to ship a
"just works" tier later.

This plan replaces it with two mutually exclusive modes, chosen in the config panel (the gear
popover grows from Shortcuts-only into a real settings panel) and persisted across restarts:

- **Default mode** — one provider configured entirely by `WIKILENS_DEFAULT_*` env vars. The
  footer chip reads **`Default`** and nothing else: no provider name, no model id, no menu.
  The player never learns two models are involved or which vendor is behind them. This is the
  MVP for a future tier where the same "Default" is served from our side — the credential
  source must be swappable without touching the UI.
- **Custom API mode** — the user pastes per-provider keys into the panel. A key crosses IPC
  exactly once (webview → Rust on save), is stored encrypted on this machine only
  (see "[[2026-07-26_api-key-storage-dpapi]]"), and is never displayed back in any form —
  the only action on a set key is Remove. The model menu behaves exactly as today except only
  keyed providers appear; the picked model drives both the answer and the rewrite.

Supersedes the key-and-provider story locked in "[[2026-07-02_multi-provider-llm]]" (env-only
keys, all providers always listed) and retires the explicit non-goal in
"[[2026-07-19_hotkey-config]]" ("no other settings tenant") — the settings surface it built is
exactly what grows here.

## Goal / non-goals

- Goal: mode choice persisted in `settings.json`; panel-entered keys encrypted at rest;
  key material never re-crossing IPC toward the webview, never rendered, never logged, never
  in the debug window/table or any error message; Default mode fully env-driven with the
  vendor invisible; every degenerate state defined.
- Goal: land as four independently shippable PRs (Approach below).
- Non-goal: any model catalog for the default target — `list_models`, `models_endpoint`,
  `curated_models` do not exist for it; there is no menu to feed in Default mode.
- Non-goal: masking the debug surfaces. `debug://ask-started` and the stderr table print
  `"provider / model"`; in Default mode that reveals the vendor to whoever sets
  `WIKILENS_DEBUG=1`. Accepted as a dev-tooling carve-out (flag off by default, exists to
  debug) — documented in CLAUDE.md rather than blinded.
- Non-goal: removing the `WIKILENS_<PROVIDER>_MODEL` overrides — they stay as the orthogonal
  escape hatch for model-id drift.

## Approach

The architectural centerpiece is a **resolved-target layer**: a new
`LlmTarget { kind: ProviderKind, endpoint: String, api_key: String, model: String }` that
`llm.rs` (`answer_streaming` / `rewrite_query`) consumes instead of `&Provider` + separate
key/model args. `provider.endpoint` has exactly four call sites in `llm.rs` and auth-header
branching already keys off `ProviderKind`, so the refactor is mechanical. Each ask resolves an
answer target + a rewrite target:

- Custom mode: registry `Provider` + `KeyStore` key + the picked model (rewrite = same model).
- Default mode: a pure function over an injected env lookup (`fn(&str) -> Option<String>`, the
  `resolve_model` purity precedent) — this function is the swappable boundary the hosted tier
  replaces later. `WIKILENS_DEFAULT_API_PROVIDER` selects the **wire protocol**
  (`anthropic` | `openai` → `ProviderKind`), never a registry/vendor id.

Env vars (all under one greppable stem): `WIKILENS_DEFAULT_API_KEY`, `_API_PROVIDER`,
`_API_URL`, `_ANSWER_MODEL` required; `_REWRITE_MODEL` optional (falls back to the answer
model); `_VISION` optional (falls back to text-only — no id heuristic; the repo precedent is
`reasoning_from_id` trusting only conclusive signals). Any required var missing → the
"unconfigured" state: a readable error naming the missing vars and pointing at Settings,
never a stuck status.

### Phases (one PR each, sequenced so the panel lands before the env path dies)

1. **Rust stores** — `keys.rs` (`KeyStore` trait + DPAPI-backed store per the ADR) and a
   `mode: "default" | "custom"` field in `SettingsStore` (its forward-compat `extra` maps were
   built for this), plus the new guardrail pins. Behaviorally inert.
2. **IPC + config panel** — commands `set_api_key` (the single webview→Rust key crossing),
   `remove_api_key`, `list_key_status` → `[{id, name, hasKey}]`, `set_mode` → `SettingsInfo`
   (the `set_hotkey` shape), `import_env_key` (legacy migration: Rust reads env → store, key
   never crosses IPC at all). `SettingsInfo` grows `mode` + `defaultMode: {configured, vision}`.
   The gear popover becomes one sectioned scrollable `menu menu--top` panel — Model source /
   API keys / Shortcuts under `.menu-heading` headings, key field on the `.menu-url-row`
   input+button pattern, gear label "Shortcuts" → "Settings". Existing rules hold: owned
   controls only, capture-phase Esc, one `openMenu` member, direct `.panel` child, no
   clearance retuning (`--menu-clearance-top` has no JS twin); no new DESIGN.md tokens
   expected — reuse `menu-row`/`hotkey-row`/input tokens, and document any that do appear.
   Transitional rule: "has key" = store **or** legacy env, so nothing breaks yet.
3. **Default mode** — the `LlmTarget` refactor, the `WIKILENS_DEFAULT_*` resolver, mode-aware
   `run_ask` (ignores `provider_id`/`model` args in Default mode), the static `Default` footer
   chip (single frontend constant; frontend skips `list_providers` entirely), model-menu
   filtering to keyed providers, and the degenerate states below.
4. **Legacy removal + docs sweep** — delete `Provider.api_key_env` / `Provider::api_key()` /
   `AppError::MissingApiKey.env_var` and the three vendor key vars plus
   `WIKILENS_REWRITE_MODEL` / `WIKILENS_REWRITE_PROVIDER`; add the returning-dev safety net
   (startup stderr warning when legacy vars are detected + the panel's per-provider
   "found in environment — Import" hint + README migration note). Docs: README "Setup: API
   keys" rewrite and retrieval-table row removals, `.env.example` rewrite, smoke-checklist
   prerequisites + §9 OS-env item replaced by key-store/mode items, CLAUDE.md §4,
   `.impeccable/design.json` mock error copy, `llm.rs:380` doc comment, and the circuit
   breaker's stderr notice (state.rs:97-99 names the removed vars as the fix).

### Consequences worked through (positions on the design review's 13 points)

1. **`ProviderInfo` inversion** — the invariant relaxes to: key *presence* booleans may cross
   IPC; key *material* never. `list_providers` returns keyed-only in Custom mode with an
   unchanged field set, so the `provider_info_serializes_exactly_the_known_fields` pin
   survives; `list_key_status` feeds the panel all three rows. CLAUDE.md §4 rewritten.
2. **One-way rule, testable** — key material appears in exactly one IPC direction: the
   `set_api_key` request. No command response, event payload, error string, debug row, or
   `debug://` payload may contain it. Enforced by field-set pins on every new response type
   plus the existing debug payload key-set pins.
3. **Key storage** — DPAPI blob; the fork and its rejected alternatives (Credential Manager
   via `keyring`, plaintext JSON) live in "[[2026-07-26_api-key-storage-dpapi]]".
4. **Legacy env removal** — staged (phases 2→4); dead-`.env` failure mode designed against
   with warning + Import + README note. New missing-key copy: "No API key for {provider} yet —
   add one in Settings → API keys" (no more "restart WikiLens": stored keys are read at ask
   time). dotenvy stays — it now serves `WIKILENS_DEFAULT_*` and the tuning vars in dev.
5. **Endpoint no longer compile-time** — solved by the target layer; `Provider` stays
   `&'static` declarative data, the registry untouched.
6. **`list_models` in Default mode** — never called; nothing built for it (non-goal above).
7. **Vision** — `WIKILENS_DEFAULT_VISION` only, default text-only; reaches the frontend as
   `SettingsInfo.defaultMode.vision`. The capture chip disables with the existing hint; the
   hotkey path shows the panel + message as today, but `CAPTURE_NEEDS_VISION` gets a
   Default-mode copy variant (the current copy says "pick one with the Image badge" and there
   is no menu to pick from).
8. **Rewrite auto-skip** — unchanged, judged on the effective rewrite model in both modes.
   Custom mode is behaviorally today's default (rewrite already falls back to the answer
   model; only the env overrides die), so the "ⓘ Fast models are recommended" note survives
   verbatim. Default mode: only `reasoning_from_id` applies (no curated overlay off-registry);
   the session circuit breaker backstops unknown models.
9. **Env naming** — `WIKILENS_DEFAULT_*` stem (repo convention; zero mid-migration collision
   with the dying `WIKILENS_REWRITE_MODEL`). Removed/surviving vars listed in phase 4;
   README's "Retrieval tuning (advanced)" table loses its two rewrite-pin rows.
10. **Persistence split** — `mode` in `settings.json` (plain setting); key material in the
    DPAPI store (secret) — never in the same file. Key presence queried live, never cached in
    settings.
11. **Panel** — one sectioned panel, decided; details in phase 2.
12. **Footer chip in Default mode** — a static, non-interactive `Default` label (one frontend
    constant; grep found no existing "Default" UI string to collide with). No menu or
    popover — the explanation lives in the panel's mode section. Nothing vendor-shaped
    crosses IPC in Default mode.
13. **Degenerate states** — unconfigured Default: error names the missing vars + Settings.
    Zero-keys Custom: the ModelChip (which today unmounts with no matching provider, a state
    the test harness's `^Model:` regex depends on) becomes a "Set up a model" chip opening
    Settings; submit stays blocked by the existing guard. Bad stored key: the vendor's 401 in
    the error box; `list_models` now can 401 live with a stored-but-rejected key → existing
    "offline list" degrade (new smoke item). Mode switch mid-ask: structurally prevented (the
    gear is `disabled={busy}` and submit closes menus); Rust's snapshot-at-ask-start is the
    backstop. Stale pick after key removal: the existing validate-against-list fallback
    handles it once the provider list re-fetches — key changes must trigger that re-fetch
    (App's validate effect runs once on mount today); the localStorage entry stays and
    revives with the key. First launch: auto-sense once (complete `WIKILENS_DEFAULT_*` env →
    `default`, else `custom`); the stored mode wins forever after.

### Testing

- Rust: `KeyStore` trait tests via an in-memory impl + real DPAPI round-trips as plain unit
  tests (CI's Rust job runs `windows-latest`); a raw-file assert that plaintext never lands on
  disk; `SettingsStore` mode round-trip on the tempdir pattern; Default-resolver pure-fn tests
  over an injected env lookup (no env mutation — the suite is multi-threaded); `LlmTarget`
  request building through wiremock + `test_support::mock_provider` (the refactor removes the
  `Box::leak` workaround).
- Guardrails: the `ProviderInfo` pin kept; new `KeyStatus`/`SettingsInfo` field-set pins; a
  keys-file no-cleartext pin; the `MissingApiKey` copy test rewritten (the `env_var` field
  dies); the `debug://` payload key-set pins unchanged.
- Vitest via `src/test/backend.ts` `installBackend` (new commands must join its handler map or
  every touching test throws): panel key states (paste+Save / "Key set"+Remove / Import hint),
  mode switch driving chip + menu, the static `Default` chip, the zero-keys empty state,
  capture gating on `defaultMode.vision`.
- Manual (smoke checklist): a stored key survives restart; packaged-app Default mode via OS
  env vars; a real vendor 401 on a bad stored key; first-launch auto-sense both ways; the
  capture hotkey in Default mode with `_VISION` unset and set.

## Decisions & trade-offs

- Key storage is a real fork with rejected alternatives → split into
  "[[2026-07-26_api-key-storage-dpapi]]".
- Wire-protocol selection over vendor selection for `WIKILENS_DEFAULT_API_PROVIDER`: a
  neutral "our proxy speaking the OpenAI protocol" must be expressible without impersonating
  a shipped vendor, and vendor ids must not flow through Default-mode internals.
- Debug vendor visibility in Default mode: accepted (carve-out above) — masking would blind
  the one tool used to debug Default mode.
- "[[2026-07-05_model-list-sourcing]]" justified session-only model-list caching with "keys
  are read at process start"; that premise dies here. Traced conclusion: still no
  invalidation needed (the cache holds public catalogs keyed by provider id; key changes only
  affect list *visibility*), but the rationale is now this plan, not process-start keys.

## Status log

- 2026-07-26 — created after the research + design pass; four forks settled with the owner
  (DPAPI storage, resolved-target layer, `WIKILENS_DEFAULT_*` naming, one sectioned panel).
