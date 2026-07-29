---
title: A stored API key is not a mode choice
type: decision
status: done
created: 2026-07-29
updated: 2026-07-29
tags: [frontend, llm, overlay]
related:
  [
    "[[2026-07-29_settings-modes-and-keys]]",
    "[[2026-07-28_answers-come-from-source-list]]",
    "[[2026-07-26_default-mode-and-byo-api-keys]]",
  ]
commit: 03140b9
---

# A stored API key is not a mode choice

## Context

The Settings panel used to hold one exclusive list where every row was an answer
source: the built-in model, then one row per provider. In that world a key and a
choice were the same gesture — an unkeyed row opened a key field, and saving the
key both stored it and made that provider the live source. That coupling was
deliberate ([[2026-07-28_answers-come-from-source-list]] argued for it: pasting a
key is the strongest possible signal that you want to use it).

The two-level IA breaks the premise. The rows under **Answers** now pick a *mode*
— the built-in model or your own provider — and the provider lines beneath them
only set and clear keys, because *which* provider answers is picked from the
footer chip on the main panel. Once a key line is no longer a row you can choose,
the question is what saving a key should still do to the mode.

## Decision

Saving a key stores the key and changes nothing else: the mode stays where the
player put it, and the accent check does not move.

## Consequences

- Good: the panel has one control per job. The mode rows are the only thing that
  moves the check, so nothing in the key list can contradict what the check says.
- Good: a player on the built-in model can pre-load a provider key for later
  without being ejected from Default mode by a paste. Under the old rule that
  ejection was silent — the footer chip swapped vendors with no announcement.
- Cost: adding your first key no longer gets you a working custom setup in one
  gesture. You add the key, then click **Your own provider**. Two steps where
  there was one, on the least frequent flow in the product.
- Cost: `onPickProvider` loses its last caller and is deleted from
  `SettingsMenuProps` and `App.tsx`. The footer chip's provider selection now
  comes only from `App`'s own first-keyed-provider fallback.
- Follow-up: the Custom row's "Needs a key" note must clear the moment a key
  lands, or the panel reads as blocked right after you unblocked it. Handled by
  deriving it from `statuses` rather than from the mode.

## Alternatives considered

- **Keep the auto-switch as onboarding shorthand** — saving a key continues to
  commit Custom mode and select that provider. Rejected: it is the shortest path
  from "no key" to "working", but it makes a key line behave like a choice in a
  list whose whole point is that key lines are not choices, and it changes the
  answering model without saying so. The onboarding win is real; it is not worth
  a control that silently means two things.
- **Auto-switch only from a never-chosen mode** — flip to Custom on the first
  key, never afterwards. Rejected as the worst of both: the same hidden mode
  change, now also inconsistent between the first key and the second.
