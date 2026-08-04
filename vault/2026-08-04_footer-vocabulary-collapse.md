---
title: Footer vocabulary collapse — player language over provider jargon
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [frontend]
related:
  - "[[2026-07-18_impeccable-design-context]]"
commit:
---

# Footer vocabulary collapse — player language over provider jargon

## Context / problem

The footer chip speaks operator language — `provider · model` — at a player
who just wants answers. The 2026-07-18 design critique
([[2026-07-18_impeccable-design-context]]) called for player-language model
status in the footer, with the provider/model machinery pushed one level
down.

## Goal / non-goals

- Goal: the footer reads as status a player understands at a glance; the
  full provider/model detail lives one level down (the chip's menu), where
  the operator goes on purpose.
- Non-goal: changing which models are available or how picks work — copy and
  hierarchy only.

## Approach

Sketch: rework `ModelChip.tsx`'s label vocabulary (the menu itself keeps the
full detail). What "player language" means concretely — capability words?
plain "Default"? — needs a copy pass against `PRODUCT.md`'s register before
any code.

## Decisions & trade-offs

Default mode already collapses the footer to just "Default"; this extends
that spirit to Custom mode without hiding what the operator needs one level
down.

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
