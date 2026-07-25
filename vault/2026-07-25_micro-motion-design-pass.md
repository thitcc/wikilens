---
title: Micro-motion design pass on the overlay
type: plan
status: active
created: 2026-07-25
updated: 2026-07-25
tags: [frontend, overlay]
related: ["[[2026-07-18_impeccable-design-context]]", "[[2026-07-05_design-tokens-claude-design-sync]]"]
commit:
---

# Micro-motion design pass on the overlay

## Context / problem
The design system's motion doctrine is a blanket refusal: "state changes are
instant", the debug shimmer is the product's only animation, and the Don't
list bans entrance choreography outright. That doctrine kept the overlay
quiet, but it has no admission path — a motion that would genuinely help the
player (confirming a keypress registered, showing where a panel came from)
has no rule to be judged against, only a wall. This pass promotes motion from
"instant states only" to rule-governed admission: a fourth Named Rule that
lets a motion in only when it answers a question the player already has, and
faster than a hard cut would.

DESIGN.md today has no standalone motion section — the doctrine lives in Key
Characteristics, the Do/Don't lists, and `.impeccable/design.json`'s
`extensions.motion` (`instant-state`, `debug-shimmer`) beside three Named
Rules in `narrative.rules`.

## Goal / non-goals
- Goal: admit micro-motions to the overlay only through the Named-Question
  Rule — each candidate proven in a live test over real game backdrops, each
  admitted animation carrying its ID and named question in a stylesheet
  comment, all gated behind `prefers-reduced-motion` with a static fallback.
- Goal: the rule lands as docs first (DESIGN.md + `.impeccable/design.json`),
  with the admitted inventory still truthfully "the debug shimmer alone" at
  that commit.
- Non-goal: motion outside the accepted set — no animation of the answer
  stream or focus indicators, ever; exits stay instant.
- Non-goal: committing the live-test backdrop shim — it is dev-only and
  uncommitted.

## Approach

**1. Docs commit first.** Amend DESIGN.md's motion doctrine with a fourth
Named Rule, verbatim below, and mirror it in `.impeccable/design.json`
(`narrative.rules` fourth entry + `extensions.motion`). At this commit the
admitted inventory still reads: the debug shimmer alone.

> **The Named-Question Rule.** Motion may exist only to answer a question the
> player already has — "did my keypress register?", "where did this come
> from?", "did that attach?" — and only when it answers faster than a hard cut
> would. Every animation names its question, in a stylesheet comment and in its
> PR; a motion whose question can't be named is decoration, and this register
> rejects decoration. Motions that pass are still bound: transform/opacity only
> (the glass floats over a GPU-saturated game — compositor work, never
> repaint); ≤200ms; ease-out on entry; exits are instant — leaving is the
> product; input readiness is never delayed (animation is paint, not gate); and
> everything sits behind prefers-reduced-motion with a static fallback. The
> answer stream and focus indicators never animate. Candidates enter through a
> live test over real game backdrops; admission updates this inventory in the
> same PR.

**2. Live test.** A dev-only, uncommitted backdrop shim puts a game
screenshot behind the panel in the browser (one normal scene, one bright
snow/desert scene); then `/impeccable live` on the Vite dev server with the
animate action. The user drives the picker. Candidates carry test IDs used in
all output and discussion; each has three live variants `.1/.2/.3`
(e.g. A-01.2):

| ID   | Motion                                                       | Named question                        |
| ---- | ------------------------------------------------------------ | ------------------------------------- |
| A-01 | summon slide (~150ms translateX+fade in; hide stays instant) | "the panel came from the right edge"  |
| A-02 | hover release (instant on, ~120ms fade off)                  | "the control heard you"               |
| A-03 | capture-attach confirmation (~150ms fade+rise)               | "your grab landed"                    |
| A-04 | menu open (~120ms fade + 4px rise)                           | "this came from that chip"            |

Judgment bar: *if you notice the panel while not asking it something, it's
too loud.* Decisions come back by ID (e.g. "accept A-01.2, reject A-04").

**3. Land accepted variants only.** CSS transform/opacity; a stylesheet
comment per animation carrying its ID and named question (e.g. "A-01: the
panel came from the right edge"); `prefers-reduced-motion` gate with a static
fallback (the debug shimmer in `src/debug/styles.css` is the template). The
summon animation must not delay focus or break the existing
focus/select-suppression pins (`overlay://shown` focus+select, the 2s
`overlay://hidden` suppression). The same PR updates the Named-Question
Rule's admitted inventory with each admitted ID. No motion outside the
accepted set.

**4. Delivery.** `feat/` branch off updated `main`, `/check`, then flag the
user for the manual smoke over a real game — motion is runtime-only surface.
PR with a real "⚠️ Behavior changes" section (visible motion is
player-facing), close this doc, `/vault-lint`.

## Decisions & trade-offs
Anything worth remembering. A real fork (a rejected alternative) → split into a
decision doc (`/decision-doc`) and link it in `related:`.

## Status log
- 2026-07-25 — created.
- 2026-07-25 — step 1 landed (`fe61a13`): DESIGN.md §6 Motion — the
  Named-Question Rule plus the admitted inventory, with the debug shimmer
  grandfathered ("predates the rule; its bounds and paperwork apply to
  motions admitted through it") — mirrored in `.impeccable/design.json`.
