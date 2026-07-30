---
title: Remove the "text-only model" hint from the overlay footer
type: plan
status: done
created: 2026-07-29
updated: 2026-07-29
tags: [frontend, capture, overlay]
related: ["[[2026-07-06_image-attach-guardrails]]", "[[2026-07-06_model-vision-badges]]"]
commit: 560f989
---

# Remove the "text-only model" hint from the overlay footer

## Context / problem
The footer's capture cluster shows a persistent muted "text-only model" label
beside the disabled Capture chip whenever the active model can't read images
(added by [[2026-07-06_image-attach-guardrails]]). It reads as clutter the
player can't act on from the footer; the decision is to not show it for any
model.

## Goal / non-goals
- Goal: the footer never renders the "text-only model" hint — the capture
  cluster is just the (possibly disabled) Capture chip, plus the Debug chip
  when available.
- Non-goal: changing the gating itself. The chip stays disabled with its
  "This model can't read images" title; the capture hotkey still surfaces the
  panel + copy; Enter with an attachment on a text-only model stays a no-op.

## Approach
Delete the hint span in App.tsx's footer; remove the now-dead `.chip-hint`
CSS rule and update the capture-cluster comment; retarget the two
App.defaultMode tests that asserted on the hint (the gated test now asserts
the hint is absent while the chip is still disabled).

## Decisions & trade-offs
The hover title (and the hotkey/attach error copy) becomes the only
explanation for the disabled chip — accepted: the hint duplicated it as
permanent footer noise.

## Status log
- 2026-07-29 — created.
- 2026-07-29 — landed in 560f989; closed.
