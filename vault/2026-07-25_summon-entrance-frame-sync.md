---
title: Frame-sync the summon entrance so every summon animates
type: plan
status: done
created: 2026-07-25
updated: 2026-07-25
tags: [frontend, overlay, tauri]
related: ["[[2026-07-25_micro-motion-design-pass]]"]
commit: 62835df
---

# Frame-sync the summon entrance so every summon animates

## Context / problem

A-01 (panel-summon) played fully only on the first summon after launch; every
later summon popped in and showed a tiny shake at the end. Cause: `window.rs`
shows the window and emits `overlay://shown` immediately, but a hidden WebView2
suspends presentation and takes a few frames to resume after `show()` — the
120ms animation clock ran against unpresented frames, so only the tail of the
entrance ever reached the screen. A second, subtler artifact (a one-frame
tremble as the animation lands) traced to the compositor demoting the panel's
layer when the animation ends on the `backdrop-filter` element, re-rasterizing
the glass and text.

## Goal / non-goals

- Goal: every summon replays the full A-01 entrance with a clean landing — no
  pop-in, no end shake.
- Non-goal: changing the motion voice — duration, easing, and travel stay as
  admitted in DESIGN.md §6.

## Approach

- `will-change: transform` keeps `.panel` composited at rest, so the
  animation's end no longer demotes the layer and re-rasterizes the glass
  (the antialiasing snap).
- A `panel--pre-summon` hold keeps the panel in the keyframe's "from" state
  from hide (and mount — the window starts hidden) until the entrance arms,
  so a resuming webview can never present the at-rest panel early.
- `overlay://shown` arms the entrance frame-synced: two rAFs prove the webview
  is presenting again, a 100ms timer backstops a stalled rAF pump, and a hide
  mid-wait cancels the pending arm via a token.
- Dev-serve only: an HMR reload while the window is visible arms immediately —
  no `overlay://shown` is coming, and the hold would strand the panel
  invisible until the next hide+show.
- `App.summon.test.tsx` pins the new contract: hold at mount, frame-synced
  arm, name-filtered animationend clear, hide re-hold, mid-wait cancellation.

## Decisions & trade-offs

- The entrance now starts ~2 frames after the window shows — accepted;
  imperceptible next to the cured pop-in.
- Rejected alternative, kept in reserve: animating an inner wrapper so the
  `backdrop-filter` element never transforms. The live test showed the
  frame-sync plus the permanent layer already land clean.

## Status log

- 2026-07-25 — created; fix implemented and verified live on-device (the
  pop-in and the end shake are gone).
- 2026-07-25 — closed: `/check` green, landed as 62835df.
