---
title: Foreground-window game auto-detection
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [overlay, rust]
related:
  - "[[2026-07-02_scaffold]]"
commit:
---

# Foreground-window game auto-detection

## Context / problem

The player picks the game by hand via the header chip on every switch. The
overlay already knows what it's floating over — the foreground window is the
game — so summoning could preselect the right wiki. Parked on the roadmap
since the scaffold.

## Goal / non-goals

- Goal: summoning over a known game auto-selects it in the game chip.
- Non-goal: auto-adding unknown games (the add-game probe flow stays
  deliberate); detecting anything while the overlay itself is foreground.

## Approach

Sketch: on summon, read the foreground HWND's process/exe name Rust-side and
match it against a per-game process-name field on the registry entries
(built-in and user-added). Manual pick via the chip stays as the override —
detection preselects, never locks.

## Decisions & trade-offs

Needs a name→game mapping that doesn't exist yet, and a rule for ties or
unknowns (fall back to the last-used game, exactly as today).

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
