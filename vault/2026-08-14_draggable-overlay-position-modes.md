---
title: Draggable overlay with anchored and manual position modes
type: plan
status: done
created: 2026-08-14
updated: 2026-08-14
tags: [overlay, frontend, rust, tauri]
related:
  - "[[2026-08-04_panel-corner-pick]]"
commit: [920fccc, 02b63ba, 40641eb, 9afcad4]
---

# Draggable overlay with anchored and manual position modes

## Context / problem

The overlay always docks top-right — exactly where many games put their
minimap or quest tracker. [[2026-08-04_panel-corner-pick]] sketched a
corner-pick setting but non-goaled dragging ("corners only"); the feature the
user actually wants supersedes that: five anchored placements *and* a manual
mode where the panel stays exactly where it was dragged, across sessions.

## Goal / non-goals

- Goal: a **Position** stepper in Settings with six values — Top Right
  (default), Top Left, Bottom Right, Bottom Left, Center, **Manual**. Anchors
  are placements the app computes; Manual is the position the player put
  there by hand.
- Goal: the panel drags by its header. Dragging while anchored switches the
  setting to Manual — drag expresses intent.
- Goal: independent memories — the last anchor choice and the last dragged
  position both persist in `settings.json`, and stepping between them
  restores each (vice-versa).
- Goal: a **padlock** on the Position row (persisted, default unlocked).
  Locked = the header never drags, in every mode Manual included; the
  stepper keeps working.
- Non-goal: panel resizing; per-monitor position profiles; clamping the
  shadow apron's adjacent-monitor overhang.

## Approach

- **Settings schema** (`settings.rs`, the `set_mode` mutator shape):
  `position: { mode: anchored|manual, anchor, manual: {x, y}, locked }` —
  the lock gets its own single-purpose mutator (`set_position_locked`), the
  house shape.
  Lives in `settings.json`, not localStorage — Rust places the window before
  the webview boots. Stored coordinates are physical virtual-screen
  outer-position px and never cross IPC; `SettingsInfo.position` carries only
  `{mode, anchor, locked}`. Picking Manual with no stored position snapshots
  the current window position. A stored position on no connected monitor
  falls back to the remembered anchor for that show without rewriting
  settings (docking changes are often transient).
- **Layout** (`window.rs`): one anchor-aware `layout_overlay` replaces
  `position_top_right` at both call sites (every show, every height report) —
  the old positioner's dedupe would snap a dragged window back within ~100ms
  of any content change. Growth direction falls out of recomputing the rect
  per call: top anchors keep the top edge, bottom anchors pin the bottom
  edge (grow upward), center pins the center; in Manual, height reports set
  **size only** so nothing fights a drag.
- **Apron**: symmetric always (the debug window's 20/32/44/32 scheme — a
  draggable window has no screen edge to hide a clipped shadow behind).
  Anchors keep the 12px visual gap by letting the outer apron hang
  off-screen, so corner dead zones stay exactly today's. CSS margins/tokens,
  `menuPlacement.ts` constants, and entrance-motion direction (per-anchor
  travel; rise for Center/Manual) update in lockstep via root data
  attributes.
- **Drag**: `core:window:allow-start-dragging` joins
  `capabilities/default.json` (silently inert without it) and
  `data-tauri-drag-region="deep"` goes on `.panel-header` (debug-window
  precedent; chips still click) — rendered only while unlocked. A
  `WindowEvent::Moved` arm with an **applied-target guard** detects real
  drags: our own `set_position` fires Moved too, and bottom/center anchors
  move `y` on every height report — without the guard a streaming answer
  would flip the mode to Manual. A real drag flips effective placement to
  Manual immediately, debounce-persists, and emits `settings://position`
  (payload pin-tested, no coordinates) so the frontend tracks the flip; when
  locked, a foreign Moved re-applies the layout instead — the lock really
  pins.
- **Settings UI** (`SettingsMenu.tsx`): the Position stepper follows the
  Answers IPC template (busyAll gating, error slot, one-open-popover union),
  placed between Theme and Shortcuts; the padlock rides the Position
  heading's right rail by default (the `menu-version` precedent) — final
  affordance decided by proof sheet.
- **Delivery**: one PR, ordered commits — settings schema → anchor-aware
  layout + apron/CSS lockstep → Position stepper + lock → drag capture →
  docs/close. Guardrail pins trip by design and get updated deliberately:
  `settings_info_serializes_exactly_the_known_fields`, styles
  `INVARIANT_TOKENS`, SettingsMenu's sections-in-order test. Manual smoke
  additions: real drag (chips still click, double-click doesn't maximize),
  each anchor's gap at 100%/150% DPI, growth direction under a streaming
  answer, drag → restart → restored, monitor unplug fallback, the lock.

## Decisions & trade-offs

- **Single 6-value stepper** over a mode toggle + anchor row: dissolves the
  fixed/free naming problem — no mode label exists, only the sixth value
  needs a name. User-picked 2026-08-14.
- **"Manual"** for the dragged mode (over Dynamic/Custom/Free/Floating):
  anchors = the app places, Manual = the player placed; doesn't collide with
  Custom API or the padlock. User-picked 2026-08-14.
- **Drag-while-anchored auto-switches to Manual**, and the padlock exists
  precisely so that can't happen by accident. User-picked 2026-08-14.
- **Symmetric apron always** vs per-anchor asymmetric CSS flips: symmetric —
  one geometry, shadows never clip, corner dead zones unchanged (the extra
  apron hangs off-screen). Revisit via `/decision-doc` if implementation
  contradicts.
- Center-anchor growth rule (symmetric wobble vs top-pinned stability) and
  the 12px optical-center bias: mid-implementation calls, proof sheet if
  visual.

## Status log

- 2026-08-14 — created; supersedes [[2026-08-04_panel-corner-pick]] (flipped
  to dropped) — this plan covers its goal and removes its no-dragging
  non-goal.
- 2026-08-14 — position-lock-motion sheet: **A1 heading rail** for the
  padlock (the version-stamp precedent; zero Stepper changes) — declined A2
  beside-the-stepper (crowds the frame, breaks the heading→stepper
  adjacency); **B1 icon-only** locked state (closed padlock in accent ink;
  open muted when free) — declined B2's LOCKED pill (second voice on the
  heading); **C1 rise** (8px up, 120ms ease-out) for the Center/Manual
  entrance — declined C2 sink. Corner anchors keep the edge slide (left
  mirror settled by symmetry, not asked).
- 2026-08-14 — done. Landed as planned in four feat commits (schema →
  layout → stepper/padlock → drag) on `feat/overlay-position-modes`; the
  center-anchor growth rule resolved to center-pinned symmetric growth (no
  wobble surfaced worth a decision doc), the apron went symmetric as
  designed. Deferred, deliberately: no clamp for the apron's
  adjacent-monitor overhang, and the drag-persist debounce stays at 500ms
  until real use argues otherwise. Manual smoke (drag/DPI/multi-monitor/
  lock) is §3b of docs/smoke-checklist.md — not yet run on-device this
  session.
