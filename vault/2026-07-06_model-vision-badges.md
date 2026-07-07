---
title: Vision badges in the model menu
type: plan
status: todo
created: 2026-07-06
updated: 2026-07-06
tags: [capture, llm, frontend]
related: ["[[2026-07-06_screenshot-capture-to-prompt]]", "[[2026-07-06_image-attach-guardrails]]", "[[2026-07-05_model-picker-menu]]"]
commit:
---

# Vision badges in the model menu

## In simple terms

Some models can read screenshots, some can't. Once
[[2026-07-06_screenshot-capture-to-prompt]] lands, the user needs to see
which is which without trial and error: every vision-capable model gets a
small "Image" badge on the right of its row in the model menu. The badge is a
new reusable `<Badge>` component, so future capability tags (tools,
reasoning) reuse it.

## Context / problem

The model menu ([[2026-07-05_model-picker-menu]]) shows only id + label.
Whether a model accepts image input is knowable — mostly for free, from
endpoints we already fetch (verified live 2026-07-06):

- **OpenRouter** `GET /models` carries `architecture.input_modalities`
  (168 of 342 models list `"image"`) — our parser currently discards it.
- **Anthropic** `GET /v1/models` returns `capabilities.image_input.supported`
  (since 2026-03-18, no beta header) — and every active Claude model is
  vision-capable anyway.
- **DeepSeek** `GET /models` returns only `{id, object, owned_by}`, and no
  DeepSeek API model accepts images at all (live-probed; the chat-app-only
  "image recognition" test has no API surface — re-check the DeepSeek
  changelog when the V4 full release ships ~mid-July 2026).

This is the metadata layer for the capture feature: purely informational
here; [[2026-07-06_image-attach-guardrails]] consumes the flag for gating.
Independent of the capture plan — the two touch disjoint files and can land
in either order.

## Goal / non-goals

- **Goal — vision metadata end-to-end:** `ModelInfo` gains `vision: bool`,
  populated from the live catalogs and from vision flags on the curated
  fallbacks, so offline/keyless sessions badge correctly too.
- **Goal — default-model vision:** `ProviderInfo` gains `defaultModelVision`
  so the guardrails doc can resolve the never-opened-menu case with no fetch.
- **Goal — reusable Badge component:** net-new `<Badge>` (children +
  variant), token-only styling; first consumer = an "Image" badge
  right-aligned in each vision-capable model row.
- **Non-goal — any gating or error behavior:** that is
  [[2026-07-06_image-attach-guardrails]].
- **Non-goal — other capability badges:** the Badge API must permit them;
  only "Image" ships here.
- **Non-goal — localStorage migration:** old `{id, label}` picks must parse
  fine with `vision` absent; resolution/self-healing is the guardrails doc's
  concern.

## Approach

1. **src-tauri/src/models.rs** — `ModelInfo.vision: bool`.
   `parse_anthropic_models`: optional `capabilities.image_input.supported`,
   **default true** — every active Claude model is vision; a schema hiccup
   must not silently un-badge the whole catalog. `parse_openai_models`
   (shared DeepSeek/OpenRouter): optional `architecture.input_modalities`
   contains `"image"`, **default false** — DeepSeek's bare entries naturally
   parse to `vision: false`, no special-casing. `resolve_model_list` maps the
   curated vision flags through in fallback mode.
2. **src-tauri/src/providers.rs** — new
   `CuratedModel { id, label, vision }` struct replaces the
   `(&'static str, &'static str)` tuples (a 3-tuple with an anonymous bool is
   unreadable). Flags: Anthropic ×3 → true; DeepSeek ×2 → false; OpenRouter
   curated (gpt-4o-mini, claude-sonnet-5, gemini-2.5-flash) → true. New
   `model_vision(&self, id) -> Option<bool>` curated lookup (`None` for
   unknown env-override ids). Update `model_label` and the existing tests
   (`default_model_is_in_curated_list`, uniqueness) mechanically.
3. **src-tauri/src/commands.rs** — `ProviderInfo.default_model_vision`,
   computed in `list_providers`: curated lookup; unknown env-override ids
   default true only for Anthropic (all-vision catalog), false elsewhere.
4. **src/types.ts** — `ModelInfo.vision: boolean`;
   `ProviderInfo.defaultModelVision: boolean`; new
   `StoredModelPick { id: string; label: string; vision?: boolean }` for the
   localStorage shape (old entries lack the field).
5. **src/App.tsx** — `storedModel()` currently *strips* parsed entries back
   to `{id, label}` on read — update it to return `StoredModelPick`,
   carrying a boolean `vision` through when present and tolerating its
   absence. `handleModelSelect` already stores the full `ModelInfo`, which
   now includes `vision`, so new picks persist the flag for free.
6. **src/components/Badge.tsx (new)** —
   `Badge({ children, variant = "neutral" | "accent", title })` →
   `<span className={"badge badge--" + variant} title={title}>`. Children
   over a label prop (future icon+text); `title` gives hover context.
7. **src/components/ModelMenu.tsx** — row becomes an ellipsizing
   `.row-name` + a `.row-side` right cluster:
   `{model.vision && <Badge title="Can read screenshots">Image</Badge>}`
   immediately before the `.check`.
8. **src/styles.css** — `.badge` (`--pad-chip`, `--border`, `--radius-chip`,
   `--surface-raised`, `--fg-muted`, `--text-label`), `.badge--accent`
   (accent text), `.row-name` (flex 1, ellipsis), `.row-side` (inline-flex,
   `--space-6` gap). Existing tokens only.
9. **Verify** — `cargo test` (Anthropic parser: supported true/false/absent;
   OpenRouter entry with `["text","image"]` / `["text"]` / no architecture;
   DeepSeek bare entry → false; curated flags per provider; `model_vision`
   known/unknown; fallback list carries vision; the ignored live OpenRouter
   smoke test asserts `any(|m| m.vision)`) · `npx tsc --noEmit && npm run
   build` · manual: live Anthropic/OpenRouter lists badge correctly; curated
   fallback badges with the "offline list" note; DeepSeek group unbadged; an
   old pre-existing `wikilens.selectedModel.*` entry doesn't crash the chip;
   long labels ellipsize without pushing the check out of the row.

Size estimate: **M** — ~200–250 LOC.

## Decisions & trade-offs

- **Asymmetric parser defaults by design:** Anthropic missing-capabilities →
  true (all-vision catalog; a schema change must not kill every badge and,
  transitively, capture gating); OpenAI-style missing-architecture → false
  (DeepSeek genuinely has no such field and is text-only; wrongly badging is
  worse than missing a badge).
- **`CuratedModel` struct over a 3-tuple:** one-time mechanical churn buys
  self-documenting flags; future capability fields extend the struct, not
  tuple arity.
- **Neutral badge variant for "Image":** roughly half of OpenRouter's catalog
  is vision-capable — an accent badge on half the rows would drown the accent
  selection check. `variant="accent"` exists for rarer future badges.
- **`defaultModelVision` on `ProviderInfo`** rather than a new command: the
  chip already labels itself before any fetch via `list_providers`; vision
  rides the same rail.
- **`ModelInfo` stays lean:** one boolean added to the deliberately trimmed
  `{id, label}` IPC shape — OpenRouter's 300+-model list grows by a few KB,
  not a new nested object.

## Status log

- 2026-07-06 — created. Modality sources verified live the same day
  (OpenRouter `input_modalities`; Anthropic `capabilities.image_input`;
  DeepSeek none — API fully text-only).
