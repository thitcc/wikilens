---
title: Model picker — footer chip and combined provider/model menu
type: plan
status: todo
created: 2026-07-05
updated: 2026-07-05
tags: [llm, frontend, rust]
related: ["[[2026-07-05_design-tokens-claude-design-sync]]", "[[2026-07-05_model-list-sourcing]]", "[[2026-07-02_multi-provider-llm]]"]
---

# Model picker — footer chip and combined provider/model menu

## In simple terms
Today you pick a *provider* (Anthropic, DeepSeek, OpenRouter) from a dropdown,
and the app quietly decides which model that means. The new design (Claude
Design exploration 3a) replaces that dropdown with a small text chip at the
panel's bottom — "Anthropic · Claude Sonnet 5 ▾" — that opens a menu listing
the *actual models* of every provider, with a filter box and collapsible
groups. Picking a row picks both provider and model at once. The model lists
come live from each provider's API when possible, with a small built-in list
as backup, so the menu always works — even with no API keys or no internet
(see [[2026-07-05_model-list-sourcing]]).

## Context / problem
Pulled from the Claude Design project on 2026-07-05: the provider `<select>`
leaves the header (game picker stays alone); a quiet footer chip opens a
combined model menu (filter input, uppercase collapsible group headers,
OpenRouter collapsed by default, accent check on the selected row; new tokens
`--surface-menu: rgba(24,24,32,0.97)`, `--shadow-menu: 0 16px 40px
rgba(0,0,0,0.45)`). This is net-new capability, not restyling: the app has no
model catalog anywhere — each provider resolves to exactly one model
(`WIKILENS_<P>_MODEL` env override else `default_model`), `ProviderInfo` is
`{id, name}` only, and `ask` takes no model argument. Research (live probes,
2026-07-05): Anthropic `GET /v1/models` and DeepSeek `GET /models` require a
key (401 without); OpenRouter `GET /api/v1/models` is public, 400+ models,
~1–2 MB raw. Urgency note: DeepSeek deprecates the `deepseek-chat` alias —
our current `default_model` — on **2026-07-24**.

## Goal / non-goals
- **Goal:** the footer chip + combined menu per design 3a, with model choice
  flowing into the ask request and persisting across restarts.
- **Goal:** per-provider model lists via the hybrid strategy (live fetch,
  curated fallback, session cache) decided in [[2026-07-05_model-list-sourcing]].
- **Goal:** migrate the DeepSeek default off the deprecated alias.
- **Non-goal:** exposing which API keys are configured to the frontend (the
  `source: fallback` marker signals "offline list" without saying why).
- **Non-goal:** arrow-key row navigation in the menu (v1 ships Esc/Tab/Enter;
  note for later), pricing/context-length display, model metadata.

## Approach
Rust first, then IPC types, then UI; each step compiles and tests green alone.

1. **Registry (`providers.rs`)** — `Provider` gains `models_endpoint`,
   `models_need_key: bool`, `curated_models: &[(id, label)]` (2–4 per
   provider; a degraded mode, not a catalog). Anthropic curated ids must be
   `// VERIFY at impl time` against the live list (needs a key); OpenRouter
   slugs spot-checked against the public endpoint. DeepSeek `default_model`
   → `deepseek-v4-flash`. New invariant tests:
   `default_model_is_in_curated_list`, curated ids unique, `label_for`.
2. **New `models.rs`** — `fetch_models(client, provider, api_key)` with an
   **8s timeout** (shorter than the wiki 12s: the fallback here is a fine
   experience, not a failed answer); pure parsers
   `parse_anthropic_models` (`data[].id` + `display_name`, `limit=1000`, no
   pagination loop) and `parse_openai_models` (DeepSeek bare ids; OpenRouter
   `id` + `name`) that trim to `{id, label}` Rust-side — OpenRouter's payload
   crosses IPC at ~30 KB; `resolve_model_list(live, curated)` — live wins
   when non-empty, error/empty degrades to curated. Add reqwest `gzip`
   feature (OpenRouter ~150–300 KB on the wire). Offline fixture tests per
   parser incl. malformed JSON; one `#[ignore]` live test on the keyless
   OpenRouter endpoint.
3. **Command + cache (`commands.rs`, `state.rs`, `lib.rs`)** —
   `list_models(provider_id) -> ModelList { models, source: live|fallback }`.
   AppState gains `models_cache: Mutex<HashMap<&'static str, Vec<ModelInfo>>>`
   (std Mutex, never held across await; dogpile accepted at ≤3 providers).
   **Cache live lists only — never fallbacks**, so transient failures retry
   on the next menu open; no invalidation within a session (keys are read at
   process start). Keyless + `models_need_key` short-circuits straight to
   the fallback (no doomed 401 round trip). OpenRouter is always called
   unauthenticated — never send a key where it isn't needed.
4. **Ask boundary (`commands.rs`)** — `ask` gains `model: String`; pure
   `effective_model(requested, fallback)`: trim, blank ⇒ `provider.model()`.
   **No validation against any list** — a cold cache or env override would
   make it wrong, and the provider is the authoritative validator (a stale
   id surfaces as the existing `AppError::Llm` message in the error box).
   `ProviderInfo` extends to `{id, name, defaultModel, defaultModelLabel}`
   (camelCase serde) so the chip can label itself before any fetch — still
   never reports which keys are configured.
5. **Precedence contract** — env override seeds the initial default; an
   explicit UI selection persists per provider
   (`localStorage["wikilens.selectedModel.<id>"]`, JSON `{id,label}` so the
   chip labels itself offline) and wins thereafter. The frontend always
   sends the effective model id; the backend blank-fallback is defensive.
6. **Frontend** — delete `ProviderPicker.tsx`; new `ModelChip.tsx` (footer
   quiet chip, `disabled` while busy — closes the "which model answered?"
   ambiguity) and `ModelMenu.tsx` (filter narrows label+id across expanded
   groups; collapsed set resets each open with OpenRouter collapsed; loading
   row; muted "offline list" note on fallback groups; Enter in filter picks
   the first visible row). **Lazy fetch:** Anthropic+DeepSeek on first menu
   open, OpenRouter only on first group expand. **Positioning:** the menu is
   a direct child of `.panel` (never inside `.content` — its
   `overflow-y: auto` would clip it), absolutely positioned above the
   footer, `max-height` calc'd so it always fits the content-hugging panel.
   **Esc layering:** the menu uses a *capture-phase* window keydown listener
   (App's Esc-hides-overlay is bubble-phase), so first Esc closes the menu,
   second hides the overlay. Outside-click close excludes the chip (else
   close+toggle double-fires). Menu also closes on submit and on
   `overlay://shown`.
7. **CSS (`styles.css`)** — tokens `--surface-menu`, `--shadow-menu`,
   `--footer-chip-line`, `--menu-clearance` (a paired-constants gotcha like
   the window.rs geometry: the calc's terms mirror the footer's metrics);
   `.panel { position: relative }` (explicit — not riding the
   backdrop-filter side effect); footer hairline, quiet chip, menu surface.
8. **Docs** — CLAUDE.md §2 map (models.rs, ModelChip/ModelMenu, list_models),
   §4 (ProviderInfo sentence, model-precedence bullet, provider-entry now
   includes the three new fields), §5 gotchas (capture-phase Esc,
   `--menu-clearance` pairing, OpenRouter payload + live-only caching).
   Close this doc + the ADR with the implementation commit.

**Manual verification matrix** (`npm run tauri dev`): keyless run (fallbacks +
"offline list", ask still fails with the clear MissingApiKey message);
offline run (OpenRouter falls back; restore + reopen ⇒ live, proving
fallbacks aren't cached); stale model id in localStorage (provider 4xx in
error box); short-panel menu fit + internal scroll; Esc layering; selection
persists across relaunch; `WIKILENS_ANTHROPIC_MODEL` seeds the chip until a
UI pick overrides it; submit-while-menu-open closes it.

## Decisions & trade-offs
- Model-list sourcing (fetch vs hardcode vs hybrid) was a real fork — decided
  2026-07-05 with the user, recorded in [[2026-07-05_model-list-sourcing]].
- No boundary validation of the model id: rejected as both wrong (cold cache,
  env overrides) and valueless (serde_json body, user's own key, provider
  validates authoritatively). Revisit only if a provider starts billing for
  malformed-model requests.
- Chip disabled during a busy ask mirrors the old picker convention and
  guarantees answer/model consistency; a mid-stream switch is a non-goal.

## Status log
- 2026-07-05 — created from the design pull (exploration 3a) after live API
  research and the hybrid decision; queued as todo. Reminder: land before
  2026-07-24 or bump the DeepSeek default separately (the alias dies then).
