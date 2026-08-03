---
title: Make keyed provider lines static and mark them with a Set pill
type: plan
status: done
created: 2026-08-02
updated: 2026-08-02
tags: [frontend, overlay]
related: ["[[2026-08-02_keyed-lines-are-static]]", "[[2026-07-29_settings-modes-and-keys]]", "[[2026-07-30_design-mirror-true-up]]"]
commit: [c1e9d37, 496773f, e480ebe]
---

# Make keyed provider lines static and mark them with a Set pill

## Context / problem
Owner bug report with screenshot: a keyed provider line (OpenRouter) still
opened its paste field on click, and set-vs-unset was unreadable — the muted
unkeyed names were "really light" with nothing but ink strength to compare.
Wanted behavior: a saved key is immutable until the trash removes it, and
keyed lines carry an explicit "Set" label.

## Goal / non-goals
- Goal: keyed lines render static (Set pill + trash, no button); unkeyed
  lines keep click-to-open; suite green; DESIGN.md, its `.impeccable`
  mirror, CLAUDE.md, and the smoke checklist all tell the new truth;
  CLAUDE.md stays under Codex's 32,768-byte project-doc cap.
- Non-goal: Rust/IPC changes (KeyStatus.hasKey already crosses), Badge API
  changes, footer-chip work.

## Approach
Conditional render in `SettingsMenu.tsx#keyLine`: `hasKey` → static div
(`.model-row--static`, hover veil and pointer stripped in styles.css — the
shared `:hover:not(:disabled)` rule matches divs) with a neutral
`<Badge>Set</Badge>`; unkeyed → today's button, plus a callback ref that
catches focus after a removal (the focused trash unmounts; the ref beats the
dialog's focus-recovery effect because both land in one React commit).
Tests: keyed-line suite reworked around "no button, pill, trash-only";
`settleKeyedAnthropic` settles on the trash; the old click-to-replace test
inverted into the full trash → add → empty-field loop pin.

## Decisions & trade-offs
Forks split into the decision doc ([[2026-08-02_keyed-lines-are-static]]):
conditional render over a click guard, neutral over accent pill, no
auto-open after trash (owner's call). Ink promotion kept as reinforcement.

## Status log
- 2026-08-02 — created from the owner's bug report; plan approved.
- 2026-08-02 — done: implemented, all gates green, docs and design mirror
  updated together.
- 2026-08-02 — owner review: the inline pill read ~2px high (box-centering
  vs Segoe's line box, measured off the screenshot); moved to the row's
  right rail, the model menu's badge position (496773f).
- 2026-08-02 — same-PR follow-up: the header gear's quiet-chip padding made
  a landscape veil around a circular glyph (the gear itself measured seated);
  squared to a 20×20 `icon-chip` seat, token added to DESIGN.md + mirror
  (e480ebe).
