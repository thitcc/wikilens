---
title: Skip the rewrite for known-Reasoning models, and tag models Fast/Reasoning in the menu
type: plan
status: todo
created: 2026-07-10
updated: 2026-07-10
tags: [llm, frontend, rust]
related: ["[[2026-07-07_llm-query-rewrite-in-retrieval]]", "[[2026-07-06_model-vision-badges]]", "[[2026-07-10_rewrite-circuit-breaker]]", "[[2026-07-10_ask-debug-instrumentation]]"]
commit:
---

# Skip the rewrite for known-Reasoning models, and tag models Fast/Reasoning in the menu

## Context / problem

Locked design decisions (user, 2026-07-10):

- **No silent cross-provider auto-pick** for the rewrite — it would send the player's
  question text to a second vendor with a second bill, undocumented. The selected **answer
  model is also the rewrite model** by design; `WIKILENS_REWRITE_PROVIDER/MODEL` remain a
  documented power-user opt-in.
- Reasoning models are provably bad *as rewriters*: their reply lands in
  `reasoning_content`, the parser reads `content` → zero candidates after multi-second
  reasoning (live-verified for both DeepSeek v4 models — decision-doc Updates 3–4). So:
  **skip the rewrite automatically** when the effective rewrite model is known-Reasoning
  (user chose auto-skip explicitly over warn-and-attempt).
- The config that avoids the doomed call is invisible to users → add **Fast / Reasoning
  tags** to the model menu (cloning the vision "Image" badge pattern, commit 096ff29) plus
  an ⓘ recommendation to prefer Fast models.

## Goal / non-goals

- Goal: a known-Reasoning effective rewrite model never fires the rewrite call (zero
  latency, zero cost); unknown models attempt (the circuit breaker catches persistent
  failures); the menu shows Fast/Reasoning badges and a one-line recommendation.
- Non-goal: no cross-provider auto-pick, ever, without explicit env opt-in.
- Non-goal: unknown models get **no badge and no skip** — a false Reasoning label would
  silently disable a working feature.
- Non-goal: no answer-path behavior change — reasoning models are fine as *answer* models.

## Approach

Three-state classification `Option<bool>`: `Some(true)` = Reasoning (badge + skip),
`Some(false)` = Fast (badge), `None` = unknown (nothing). Conservative rules: never derive
`Some(false)` from an id alone; never derive `Some(true)` from `supported_parameters`
alone (hybrid models list `reasoning` but answer directly by default).

1. `src-tauri/src/providers.rs`: `CuratedModel.reasoning: bool` (the sanctioned extension
   point). Values: Anthropic curated 3× `false` (we never send a thinking param; content
   arrives directly); DeepSeek `deepseek-v4-flash`/`-pro` → `true` (live-verified);
   OpenRouter curated 3× `false`. New `Provider::model_reasoning(&self, id) -> Option<bool>`
   (clone of `model_vision`). Test: `model_reasoning_known_and_unknown`.
2. `src-tauri/src/models.rs`: `ModelInfo.reasoning: Option<bool>` with
   `#[serde(skip_serializing_if = "Option::is_none")]`.
   `pub fn reasoning_from_id(id) -> Option<bool>` — only
   `id.ends_with(":thinking").then_some(true)`; deliberately no fuzzy o1/o3/r1 matching.
   `parse_anthropic_models` → `reasoning: Some(false)` (provider-level fact, mirrors the
   vision precedent). `parse_openai_models` reads top-level
   `supported_parameters: Option<Vec<String>>` (OpenRouter only): `:thinking` id →
   `Some(true)`; else field present WITHOUT `"reasoning"` → `Some(false)` (absence proves
   fastness); else `None`. New pure `overlay_curated_reasoning(models, curated)` filling
   only `None` gaps after a live fetch (badges the live DeepSeek list);
   `resolve_model_list` fallback maps `Some(m.reasoning)` from curated. ~6 literal updates
   in existing tests + ~6 new tests (both parsers, id heuristic, overlay, fallback
   carry-through).
3. `src-tauri/src/commands.rs` — skip wiring on the **effective** pair (after env
   overrides): `rewrite_provider.model_reasoning(&rewrite_model).or_else(||
   models::reasoning_from_id(&rewrite_model)) == Some(true)` → skip. Closure order:
   off-switch → reasoning-skip → breaker-skip → attempt. Skips never touch the breaker
   counter. Debug row: `phase_skipped("rewrite", "skipped (reasoning model)")`, no usage
   set → `-/-`.
4. Frontend: `src/types.ts` `ModelInfo.reasoning?: boolean` (absent = unknown, matches the
   serde skip). `src/components/ModelMenu.tsx` — in `.row-side` after the Image badge:
   strict `model.reasoning === false` → `<Badge title="Answers directly — best for
   WikiLens's pre-search query rewrite">Fast</Badge>`; `=== true` → `<Badge title="Thinks
   before answering — slower, and WikiLens skips its pre-search query rewrite">
   Reasoning</Badge>`. ⓘ affordance between `.menu-search` and `.menu-list`:
   `menu-note menu-note--info` div, text "ⓘ Fast models are recommended", `title` tooltip
   explaining any model can answer / Fast also powers the quick pre-search rewrite.
   `src/styles.css`: `.menu-note--info { cursor: help; }` only.
   `ProviderInfo`/`StoredModelPick`/self-heal: **no change** (the skip is Rust-side;
   badges render only from menu rows).

## Decisions & trade-offs

- Auto-skip over warn-and-attempt: firing a call we *know* fails wastes seconds per ask;
  the ⓘ note plus badges carry the education, the debug table carries the visibility.
- Not consulting the live `models_cache` for unknown ids at ask time (rejected: lock
  traffic + nondeterminism); the breaker bounds the unknown-model worst case at 2 strikes.
- `Some(false)` from OpenRouter's `supported_parameters` absence is safe: the field is the
  gateway's own capability declaration; a model that cannot reason is Fast by definition.

## Status log
- 2026-07-10 — created from the rewrite-review findings (#3 + #4 + #5, priority 3); the
  user's transformed idea replacing the rejected auto-Haiku pick.
