---
title: Panel corner-pick — choose where the overlay docks
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [overlay]
related:
  - "[[2026-07-18_impeccable-design-context]]"
commit:
---

# Panel corner-pick — choose where the overlay docks

## Context / problem

The overlay always docks top-right — exactly where many games put their
minimap or quest tracker, so the panel can cover the HUD element the player
most wants visible. Raised by the 2026-07-18 design critique
([[2026-07-18_impeccable-design-context]]).

## Goal / non-goals

- Goal: the player picks which corner the overlay docks to, persisted like
  the other settings.
- Non-goal: free-form dragging or arbitrary placement — corners only, so the
  slide-in motion and gap geometry stay principled.

## Approach

Sketch: a corner setting in `settings.json` (Settings panel row);
`window.rs`'s `position_top_right` generalizes to a corner-aware placement
(monitor offset + scale factor rules unchanged); the CSS entrance motion and
shadow gap mirror horizontally/vertically to match. The paired
window.rs/CSS geometry constants must stay in lockstep for every corner.

## Decisions & trade-offs

Bottom corners interact with the height-follows-panel behavior (the window
grows downward today) — the growth direction has to flip when docked to a
bottom corner.

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
