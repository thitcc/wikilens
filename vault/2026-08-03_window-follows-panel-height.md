---
title: The overlay window follows the panel's reported height
type: decision
status: done
created: 2026-08-03
updated: 2026-08-03
tags: [overlay, tauri]
related: ["[[2026-08-03_overlay-hug-and-clear]]"]
commit: [d08aa05, 35adf73]
---

# The overlay window follows the panel's reported height

## Context
Field report over Guild Wars 2: with the overlay open and idle, a click in
the middle of the screen hit WikiLens and stole game focus. The window was
always sized to 70% of the monitor (`PANEL_HEIGHT_FRAC`) while the glass
hugs its content, and Tauri transparent windows are not click-through — the
CLAUDE.md gotcha called the input-eating ring "kept small on purpose", which
an idle panel falsified: most of the window was invisible dead zone. Menus
complicate the fix — they render into the free window space and their
placement math (`menuPlacement.ts`) assumed the full-cap viewport.

## Decision
The window is sized to the frontend's reported panel height (ResizeObserver
→ `set_overlay_height`, Rust clamps to the 70% cap and owns the DPI math),
and opening any menu pins the window at the cap via an unbounded sentinel.

## Consequences
- Good: clicks below the idle glass reach the game (the original bug);
  Rust stays the single geometry authority (`position_top_right` is still
  the only sizer, so every show path inherits the stored height); the
  frontend never learns monitor dimensions.
- Cost / bad: a webview→Rust report per content change (throttled to ≤10/s
  while streaming; ±1px tolerance + `ceil` DPI conversion + same-value
  dedupe kill the fractional-DPI ping-pong); ModelMenu re-measures on window
  `resize` while open because the cap expansion lands async (its first
  paint uses a cap estimate, `menuViewportHeight` — paired constants with
  window.rs); one new DOM wrapper (`.content-sizer`) so growth into
  `.content`'s scrollback stays observable.
- Follow-ups: none planned; the smoke checklist gained the click-below,
  jitter, shadow-apron, and idle-menu items.

## Alternatives considered
- Per-pixel click-through — not offered by Tauri/WebView2 (no hit-test
  surface for transparent regions).
- `SetWindowRgn` input region around the glass — clips the drop shadow the
  apron exists to hold, and hands Win32 a second geometry to keep in sync.
- Status quo ("kept small on purpose") — false on idle panels; the dead
  zone was most of the window.
