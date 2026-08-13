---
title: The settings stepper's option popover is a scoped third altitude
type: decision
status: active
created: 2026-08-13
updated: 2026-08-13
tags: [frontend, overlay]
related:
  - "[[2026-08-13_settings-stepper]]"
  - "[[2026-08-12_chrome-only-themes]]"
  - "[[2026-07-29_keys-are-not-a-mode-choice]]"
commit:
---

# The settings stepper's option popover is a scoped third altitude

## Context

The Settings enum rows become game-style steppers ([[2026-08-13_settings-stepper]]):
arrows cycle the value, the center click shows the option list. DESIGN.md §4's
Two Altitudes Rule admits exactly two floating surfaces — the panel over the
game, menus over the panel — and the settings card is already the second
altitude, with `overflow: hidden` clipping anything inside it. An option list
floating over that card is a third altitude the system deliberately forbids.

The register-compliant alternative (options disclose inline under the row,
keys-nest precedent) was offered to the user with the trade-offs; the user
explicitly chose the floating popover for the game-like dropdown feel.

## Decision

Settings enum rows are steppers whose center value opens a floating option
popover over the settings card — a user-approved, **scoped** exception to the
Two Altitudes Rule: the popover reuses Menu Glass and the existing menu float
shadow, lives as a direct child of the settings card, and nothing else may
cite it as precedent.

## Consequences

- Good: the requested in-game reading; selection anatomy (`.model-row`,
  accent check, `aria-current`) reused verbatim inside the popover; no new
  shadow or surface tokens.
- Cost / bad: DESIGN.md §4 gains an exception clause (amended in the same PR);
  Esc grows a fourth layer (recorder → popover → menu → overlay) in
  SettingsMenu's capture-phase handler; the popover needs measured placement
  with a bottom-edge flip and close-on-scroll, machinery inline disclosure
  would not have needed.
- Follow-ups: the stepper wraps at both ends — a deliberate deviation from
  `menuNav.ts`'s no-wrap clamp, whose rationale (scroll-viewport teleport) does
  not apply to a two-to-three-item cycle. Riding the same rework: the provider
  key lines render only while Custom API answers, so adding a key while
  Default answers is no longer possible (the fix path is switching to Custom
  API first) — visibility becomes a pure function of the mode, and the caret
  disclosure is deleted.

## Alternatives considered

- Inline disclosure (options unfold under the row, keys-nest precedent) —
  register-compliant and mechanically simpler; rejected by the user in favor
  of the floating dropdown feel.
- Native `<select>` — banned by the register (OS-drawn white sheet over the
  glass; DESIGN.md §5).
- Generalizing a third altitude for future surfaces — rejected; the exception
  is scoped to this one control so the Two Altitudes Rule keeps its teeth.
