---
title: Overlay design follow-ups — accent direction, bright-scene variant
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-12
tags: [overlay, frontend]
related:
  - "[[2026-07-05_design-tokens-claude-design-sync]]"
  - "[[2026-08-12_theme-switching-micrographics]]"
commit:
---

# Overlay design follow-ups — accent direction, bright-scene variant

## Context / problem

Two explorations were deliberately dropped from the
[[2026-07-05_design-tokens-claude-design-sync]] scope and parked: an
accent-direction exploration (whether `#8ab4ff` is the right single accent,
and where it should land), and a high-contrast variant for bright scenes —
the glass panel is tuned for dark game footage and can wash out over snow,
sand, or menu whites.

## Goal / non-goals

- Goal: a considered accent direction, and a bright-scene variant that keeps
  the panel readable over light backgrounds.
- Non-goal: a general theming system or user-facing theme picker — this is
  design exploration, not a settings surface.

## Approach

Sketch: token-level work against `DESIGN.md` (its frontmatter is the
normative token source — new colors/values get documented there, never
silently). The bright-scene variant likely means a heavier panel fill or
scrim rather than new type colors, keeping the 14px ceiling and single
accent intact.

## Decisions & trade-offs

Whether the variant is automatic (sampling the screen) or manual is an open
fork for when work starts.

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
- 2026-08-12 — the "no theming system / no theme picker" non-goal above is
  superseded by [[2026-08-12_theme-switching-micrographics]]: a Settings
  theme picker and a second theme shipped, giving the parked bright-scene
  variant a natural home as a future theme (chrome-only, per
  [[2026-08-12_chrome-only-themes]]). The accent-direction exploration and
  the bright-scene variant itself stay parked here; the automatic-vs-manual
  fork for the variant remains open.
