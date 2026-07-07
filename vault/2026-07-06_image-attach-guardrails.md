---
title: Guardrails for image attachments
type: plan
status: todo
created: 2026-07-06
updated: 2026-07-06
tags: [capture, frontend, llm]
related: ["[[2026-07-06_screenshot-capture-to-prompt]]", "[[2026-07-06_model-vision-badges]]"]
commit:
---

# Guardrails for image attachments

## In simple terms

Don't let the user walk into a wall: if the selected model can't read images,
the Capture chip is disabled with a short "text-only model" hint; if a
screenshot is already attached and they switch to a text-only model, submit
is blocked with a hint on the thumbnail instead of burning a request that
will fail. And if a bad request slips through anyway, the two known ugly
provider errors are translated into plain language.

## Context / problem

Failure behavior was probed live (2026-07-06): all three providers reject an
image sent to a non-vision model with a fast **pre-stream 4xx** — nothing
hangs, nothing is silently dropped, and the existing pre-stream error path in
`llm.rs` already surfaces it. But what it surfaces is hostile:

- **DeepSeek** (fully text-only): HTTP 400 whose body is a raw serde
  artifact — ``Failed to deserialize the JSON body into the target type:
  messages[0]: unknown variant `image_url`, expected `text` `` — reproduced
  on both `deepseek-chat` and `deepseek-v4-flash`, identical with
  `stream:true` (pure JSON, no SSE handshake).
- **OpenRouter**: routing-time validation, immediate 404
  `"No endpoints found that support image input"` — terse, doesn't name the
  model. Pre-first-token errors are documented to arrive as plain non-200
  JSON even on streaming requests.
- **Anthropic**: a 400 `invalid_request_error` in theory; practically
  unreachable — every active Claude model supports vision.

House convention says the provider is the authoritative validator (no
Rust-side model validation), so the answer is UI gating on the `vision` flag
from [[2026-07-06_model-vision-badges]] plus a friendly-message backstop —
never a hard block in Rust. Lands last of the three sibling plans: consumes
[[2026-07-06_screenshot-capture-to-prompt]]'s attachment state and the badges
doc's metadata.

## Goal / non-goals

- **Goal — capture gating:** the Capture chip is disabled with a visible
  "text-only model" hint whenever the active model's `vision` resolves false
  (DeepSeek: always); the global hotkey respects the same gate because it
  already routes through `handleCaptureRequest()`.
- **Goal — attached-then-switched edge:** an existing attachment plus a
  non-vision model blocks submit with an inline hint on the attachment row —
  nothing silently dropped, nothing knowingly sent to a failing API.
- **Goal — friendly error backstop:** the two known ugly pre-stream bodies
  map to plain-language messages when — and only when — an image was
  attached; everything else keeps the verbatim error-box behavior.
- **Goal — vision resolution without a fetch:** persisted pick's flag →
  `ProviderInfo.defaultModelVision` → one-time lazy patch of legacy picks.
- **Non-goal — auto-behavior:** no auto-removing the attachment, no
  auto-switching models, no capability probing of providers.
- **Non-goal — mid-stream error mapping:** the known failures are all
  pre-stream; the SSE loop is untouched.

## Approach

1. **src/App.tsx** — gating core.
   `activeModelVision(provider, pick)`: DeepSeek short-circuit false → pick's
   boolean `vision` → `provider.defaultModelVision` → false. Legacy-pick
   self-heal: if `pick.vision === undefined`, one `listModels` call (session
   cache or instant curated fallback — works offline) patches the flag and
   rewrites localStorage. `handleCaptureRequest` prepends the vision guard
   (covers the hotkey path; the button is already disabled). Capture chip:
   `disabled={busy || !vision}`, plus a `.chip-hint` "text-only model" beside
   it when gated. Submit guard: `if (attachment && !vision) return;`. Blocked
   attachment row gets `.attachment-row--blocked` and an `.attachment-hint`:
   "This model can't read images — remove it or pick one with the Image
   badge."
2. **src/styles.css** — `.chip-hint` (`--fg-muted`, `--text-label`),
   `.attachment-hint` (`--error-fg`, `--text-status`),
   `.attachment-row--blocked .attachment-thumb`
   (`opacity: var(--opacity-disabled)`). Existing tokens only.
3. **src-tauri/src/llm.rs** —
   `friendly_image_error(provider, status, body) -> Option<String>`, called
   in the pre-stream non-success branch **only when `image_png.is_some()`**:
   body contains ``unknown variant `image_url` `` → "DeepSeek models can't
   read images. Remove the screenshot or switch providers."; 404 + body
   contains "support image input" → "{model} on OpenRouter can't read images.
   Remove the screenshot or pick a model with the Image badge." No match →
   the verbatim `AppError::Llm` (backstop preserved).
4. **src-tauri/src/error.rs** — `VisionUnsupported(String)` variant,
   `#[error("{0}")]` — the message is complete user-facing text, per house
   rule.
5. **capture.rs / commands.rs** — re-verify the capture plan's failure paths
   as guardrails: `begin` rejects during an ask or pending capture; stale-id
   message ("That screenshot expired — capture it again."); xcap failures
   (secure desktop, DRM-protected content) surface as "Couldn't capture the
   screen: …" in the error box with the overlay restored.
6. **Doc note (no code):** the attachment persisting across hide/show and
   game-scene changes is by design (lifecycle decision); the always-visible
   thumbnail is the mitigation, and a replacement capture is one hotkey away.
7. **Verify** — `cargo test` (`friendly_image_error`: both live-captured
   bodies positive; same body with no image attached negative — must stay
   verbatim; unrelated 400s negative) · `npx tsc --noEmit && npm run build` ·
   manual: DeepSeek selected → chip disabled + hint, hotkey inert; Anthropic
   default with no localStorage → enabled via `defaultModelVision`;
   OpenRouter non-vision model → disabled; attach on a vision model → switch
   to DeepSeek → thumbnail dims + hint + Enter no-op; switch back → submit
   works; remove attachment while blocked → submit re-enables; legacy
   `{id,label}` pick self-heals after one provider load; capture attempted
   over the UAC secure desktop → friendly message, panel restored; a forced
   unmatched provider error confirms the verbatim backstop is still
   reachable.

Size estimate: **M** — ~150–200 LOC.

## Decisions & trade-offs

- **Block submit + inline hint** over send-without-image (silently discards
  content the user deliberately captured) and over let-it-error (burns 3–8 s
  and makes the backstop the primary UX). Same philosophy as the confirmed
  "disable capture + hint" decision: never knowingly dispatch a request we
  can predict will fail — while Rust still never validates model ids.
- **Gating is 100 % frontend** — possible only because the capture plan
  routed the hotkey through `capture://hotkey`; Rust holds no duplicate
  "is vision" state to drift.
- **Substring error mapping is brittle by nature** — acceptable because it's
  image-conditional (zero effect on text asks) and the verbatim error box
  remains the fallback for any provider drift.
- **Legacy-pick lazy patch via `listModels`** over treating unknown as
  non-vision (would wrongly disable capture for returning users on vision
  models) and over a dedicated migration command (surface growth for a
  one-time fix). `listModels` always resolves — curated fallback is instant
  offline.
- **Accepted friction:** a gated hotkey while the panel is hidden does
  nothing visible; the footer hint explains the state as soon as the panel
  opens. Alternatives (toast, auto-opening the panel to show an error) add
  surface for a rare case.

## Status log

- 2026-07-06 — created. Error bodies captured from live probes the same day
  (DeepSeek serde 400 on chat + v4-flash, streaming and not; OpenRouter
  routing-time 404).
