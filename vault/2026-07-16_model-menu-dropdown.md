---
title: Model menu as a measured dropdown below the chip
type: plan
status: active
created: 2026-07-16
updated: 2026-07-16
tags: [frontend, overlay]
related: ["[[2026-07-05_model-picker-menu]]", "[[2026-07-05_game-picker-owned-menu]]"]
commit:
---

# Model menu as a measured dropdown below the chip

## Context / problem

The model menu opens **upward** from the footer chip: `.menu` pins
`bottom: var(--menu-clearance)` and caps `max-height` against the panel
(`100%`) — but the panel hugs its content, so on an idle panel the menu is
strangled to ~2 visible rows. The game/add-game menus already escaped this
with `.menu--top` (viewport-relative cap), but the model menu can't reuse
that anchor's direction trick from the bottom: there is no window room
*above* the panel (12 px gap), and after a long streamed answer the panel
sits at its 70 % cap with only the 44 px shadow apron *below*.

## Goal / non-goals

- Goal: the model menu opens as a classic dropdown **below the chip** (below
  the panel's bottom edge, over the game) at a **fixed ~460 px height**, and
  **flips back to the upward anchoring** — same fixed height, capped by the
  room available — when a tall panel leaves no room underneath.
- Goal: keep the frame size stable while filtering / expanding groups.
- Non-goal: changing the game / add-game menus (top-anchored CSS cap stays).
- Non-goal: re-measuring while the menu is open (streaming growth is
  accepted; the next open corrects — menus remount per open).

## Approach

Placement decided per open by a pure helper (`src/menuPlacement.ts`,
unit-tested like `modelPick.ts`), fed by one `useLayoutEffect` measurement
of the `.panel` rect + `window.innerHeight` in `ModelMenu`:

```
roomBelow = viewportH − 44 (apron) − panelBottom − 6 (drop gap)
roomAbove = panelHeight − 52 (--menu-clearance) − 14 (--space-14)
direction = roomBelow >= roomAbove ? down : up      // pick the larger side
height    = max(min(460, chosenRoom), 120)
```

CSS gains a `.menu--down` variant (`top: calc(100% + var(--space-6));
bottom: auto; max-height: none`); the height is an inline style in both
directions. Constants live in the helper with pairing comments naming their
CSS/window.rs twins. Bonus: scroll-to-selected on open, reusing the game
menu's centering math. Docs: styles.css comment blocks + CLAUDE.md §5
menus gotcha updated for the trichotomy (top-anchored CSS cap / measured
drop / measured upward fallback).

## Decisions & trade-offs

- **Pick-the-larger-side flip** over "flip only when 460 doesn't fit":
  `roomBelow + roomAbove` is constant per monitor, so the rule is monotonic
  with a single flip point and always yields the biggest usable menu.
- **Truly fixed height** (user's pick over hug-up-to-cap): stable frame
  while typing in the filter; the short offline fallback list showing some
  empty card space is accepted.
- **JS consts with pairing comments** over reading CSS custom properties at
  runtime: `getComputedStyle` returns `""` for custom props in jsdom, and
  paired-constants-with-comments is the repo's existing convention
  (window.rs ↔ styles.css).

## Status log

- 2026-07-16 — created; placement + fixed height confirmed with the owner
  (dropdown below the chip, flip upward when the panel is tall).
