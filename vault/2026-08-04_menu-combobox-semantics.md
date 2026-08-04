---
title: Menu combobox semantics (aria-activedescendant)
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [frontend]
related:
  - "[[2026-07-19_menu-keyboard-nav]]"
commit:
---

# Menu combobox semantics (aria-activedescendant)

## Context / problem

The owned menus' arrow-key highlight ([[2026-07-19_menu-keyboard-nav]]) is
visual only — assistive tech never hears which row is active. Conformant
combobox semantics (`aria-activedescendant` on the filter input pointing at
the highlighted option) would expose it, but ModelMenu's interactive group
headers block the path: a valid listbox can't contain them.

## Goal / non-goals

- Goal: the arrow-key highlight is announced by screen readers via a
  conformant combobox/listbox pattern across the game, model, and add-game
  menus.
- Non-goal: changing the menus' visual behavior or keyboard model — this is
  a semantics-only pass.

## Approach

Sketch: restructure ModelMenu so the collapsible group headers live outside
the listbox element, then wire `role="combobox"` +
`aria-activedescendant` from each menu's filter input to the highlighted
row's id. The restructure lands first; it's the prerequisite, not the
garnish.

## Decisions & trade-offs

Queued as a follow-up in the keyboard-nav plan rather than shipped with it —
the header restructure was judged too invasive to ride along (user pick).

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
