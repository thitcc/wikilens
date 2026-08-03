---
title: Hug the window to the panel and add a header Start over button
type: plan
status: done
created: 2026-08-03
updated: 2026-08-03
tags: [overlay, frontend, tauri]
related: ["[[2026-08-03_window-follows-panel-height]]", "[[2026-08-02_keyed-line-set-pill]]"]
commit: [d08aa05, 35adf73]
---

# Hug the window to the panel and add a header Start over button

## Context / problem
Two owner reports, both landing on PR #55. (1) Playing Guild Wars 2 with the
overlay open and idle, a mid-screen click hit the overlay instead of the
game — the always-at-cap window's transparent remainder captures input.
(2) No one-gesture way to abandon the current request; wanted a whirl-arrow
reset returning the panel to its default state.

## Goal / non-goals
- Goal: idle overlay's dead zone gone (window follows the panel; menus
  still get the cap); a header Start over button that wipes draft, answer,
  sources, error, announcement, and screenshot, and stops a running ask —
  with an ask that settles after a clear unable to resurrect its result.
- Non-goal: per-pixel click-through, corner-pick, compose-while-streaming.

## Approach
Part A per the decision doc: `OverlayHeight` managed state + pure
`overlay_window_height` (5 unit tests) + `set_overlay_height` command;
App-side `reportOverlayHeight` (panel border box + `.content` scrollback,
±1px tolerance, 100ms trailing throttle, menu-open sentinel), a
`.content-sizer` observer target, ModelMenu re-measure on `resize` with a
cap-estimate first paint (`menuViewportHeight`). Part B: module-scope
whirl-arrow icon (Bootstrap arrow-counterclockwise), `icon-chip` seat next
to the gear (visible only when clearable), `handleClear` with an ask-epoch
guard in `handleSubmit`'s resolve/error paths (Stop's "never reset state"
rule preserved — the epoch decides publication instead).

## Decisions & trade-offs
Forks in [[2026-08-03_window-follows-panel-height]]. Smaller calls: hidden
overlay also closes menus (the shrink happens invisibly); Start over is not
disabled while busy (its job includes abandoning an ask); `clear_capture`
fires unconditionally on clear (idempotent Rust-side).

## Status log
- 2026-08-03 — created from the two owner reports; plan approved with the
  header placement picked by the owner.
- 2026-08-03 — implemented (d08aa05): 5 Rust + 13 new frontend tests, all
  gates green, docs/smoke/design mirror updated; CLAUDE.md byte budget
  rechecked against the pending PR #54 gotcha (+455B) — combined stays under
  Codex's 32,768 cap with ~35B to spare (a trim is overdue).
- 2026-08-03 — done after a 3-lens adversarial review (35adf73): the
  ask://delta/status listeners now sit behind the clear-epoch gate (a chunk
  in flight during Start over repainted the cleared panel); resize dedupe
  moved from the stored value to the window's real geometry (a failed
  resize could strand the store with no retry; streaming past the cap spammed
  same-size SetWindowPos); a swallowed height report re-arms the frontend
  dedupe; the epoch guard's error branch and the ±1px boundary gained tests.
