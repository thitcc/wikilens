---
title: Game-style stepper rows for the Settings enum choices
type: plan
status: done
created: 2026-08-13
updated: 2026-08-13
tags: [frontend, overlay]
related:
  - "[[2026-08-13_stepper-popover-third-altitude]]"
  - "[[2026-07-29_keys-are-not-a-mode-choice]]"
  - "[[2026-08-12_theme-switching-micrographics]]"
commit: [e41086b, 3c2765d]
---

# Game-style stepper rows for the Settings enum choices

## Context / problem

The Settings panel renders its two enum settings — Answers (mode) and Theme — as
mutually-exclusive row lists with an accent check, one row per option. The user
wants them to read like in-game settings (Cyberpunk 2077 reference): a
`◁  Value  ▷` stepper whose arrows cycle through the options with wrap, and
whose center value, on click, opens a floating popover listing the options
(check on the current one; picking closes). The stepper becomes the default
control for every applicable enum setting; Shortcuts stay recorder rows.

Riding along: the visible names shorten ("Built into WikiLens" → "Built In",
"Your own provider" → "Custom API"), and the provider key lines show only while
Custom API answers — the disclosure caret is deleted rather than migrated.

## Goal / non-goals

- Goal: Answers + Theme as parameterized `Stepper` rows (one component, two
  contracts: async IPC pick with error slot vs sync theme pick); keys nest
  derived from `mode === "custom"`; renames; DESIGN.md amendments (glyph band,
  scoped Two Altitudes exception, stepper spec); tests + docs swept.
- Non-goal: renaming the footer chip's "Default" (pre-existing naming split
  with Rust's target name — untouched); any Rust or App.tsx change; combobox
  ARIA semantics (repo-wide gap, tracked elsewhere).

## Approach

1. Vault docs (this plan + the decision doc) → `/vault-lint` → commit.
2. Proof sheet before implementation (samples-first workflow): one
   self-contained HTML in `.impeccable/critique/`, real tokens, both themes —
   variants for arrow glyph/size, value alignment, borderless vs hairline box,
   Cyberpunk position ticks on/off, popover instant vs 120ms arrive. User
   picks land in the status log below.
3. Implement: `src/stepper.ts` (pure cycle helper, wraps) + `stepper.test.ts`;
   `src/components/Stepper.tsx` (row + popover portaled to the settings card,
   sibling of the scrolling list; measured placement, flip near the card's
   bottom edge, close on list scroll/resize/outside-click; Esc handled by
   SettingsMenu's capture-phase listener — four layers exactly);
   SettingsMenu rewiring (modeRow/themeRow/caret machinery deleted, keys nest
   gated on the mode, conditional "Nothing can answer yet" copy).
4. Tests: caret suite dies, disclosure flows migrate to derived visibility,
   mode/theme picks go through arrows or popover; new stepper behavior suite.
5. Docs: DESIGN.md (+ sidecar regen), CLAUDE.md (+ sync-agents), README,
   smoke-checklist rewrite of the settings walk.

Full execution detail lives in the approved plan file (Claude session,
2026-08-13); this doc records shape and outcome.

## Decisions & trade-offs

- The floating popover over inline disclosure was a real fork — split into
  [[2026-08-13_stepper-popover-third-altitude]] with the rejected
  alternatives, the wrap deviation, and the keys-visibility behavior change.
- Behavior removal: a key can no longer be added while Default answers; the
  fix path is switching to Custom API first. Accepted to keep visibility a
  pure function of the mode.
- Aria-labels keep their sentence-case descriptive strings; only visible
  names change — the accessible-name contract survives the rework.

## Status log

- 2026-08-13 — created; plan approved in-session (branch `feat/settings-stepper`).
- 2026-08-13 — proof-sheet picks (interactive sheet, both themes, live steppers):
  arrows `◁ ▷` at 9px (the direction-hint precedent); value centered (game
  voice; the note keeps the right rail); borderless, no position ticks (the
  instrument-trim rule held); popover enters with 120ms fade + 4px drop —
  admitted as **A-06 · stepper-popover-arrive** (exit instant, reduced-motion
  guarded, direction-aware travel).
- 2026-08-13 — shipped: `e41086b` (Stepper + SettingsMenu rewiring + CSS +
  test sweep, 233 green) and `3c2765d` (DESIGN.md amendments + sidecar,
  CLAUDE.md/README/smoke-checklist). A-06 reuses A-04's keyframes. Closed.
- 2026-08-13 — width follow-up on the user's in-app review: full-width control
  rows read too big. Width sheet (100/90/85/80%, both themes) → pick **90%**,
  centered, applied to the steppers, the shortcut rows, AND the Custom API
  key lines so the whole control column shares one rail.
- 2026-08-13 — border follow-up: border sheet (none / hairline box / hairline
  + inset / underline, both themes) → pick **hairline box, steppers only** —
  no inset fill, shortcut rows deliberately bare (the accent border stays the
  armed recorder's one voice). Recorded in DESIGN.md §5 as the framed-control
  voice, revising the first sheet's borderless pick after the in-app look.
