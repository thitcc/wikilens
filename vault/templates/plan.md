---
title: {{title}}
type: plan       # plan | decision | research | fix | retro | note
status: idea     # idea | todo | active | blocked | done | dropped
created: {{date:YYYY-MM-DD}}
updated: {{date:YYYY-MM-DD}}
tags: []
related: []
commit:          # optional: single hash 85aca6c, or a list [h1, h2] for multiple
---

# {{title}}

## Context / problem
Why this exists; what triggered it.

## Goal / non-goals
- Goal:
- Non-goal:

## Approach
Steps, sequencing, open questions.

## Decisions & trade-offs
Anything worth remembering. A real fork (a rejected alternative) → split into a
decision doc (`/decision-doc`) and link it in `related:`.

## Status log
- {{date:YYYY-MM-DD}} — created.
