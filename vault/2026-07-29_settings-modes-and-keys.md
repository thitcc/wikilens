---
title: Settings, split in two — answer modes above, provider keys below
type: plan
status: active
created: 2026-07-29
updated: 2026-07-29
tags: [frontend, overlay, llm, testing]
related:
  [
    "[[2026-07-29_keys-are-not-a-mode-choice]]",
    "[[2026-07-28_answers-come-from-source-list]]",
    "[[2026-07-26_default-mode-and-byo-api-keys]]",
  ]
commit:
---

# Settings, split in two — answer modes above, provider keys below

## Context / problem

[[2026-07-28_answers-come-from-source-list]] merged Model source and API keys
into one exclusive list where every row was an answer source. It fixed the
vocabulary problem it set out to fix, but it left one list doing two jobs: a row
meant both *this is what answers* and *this is where a key lives*. The tell is
that a provider with no key had to appear in a list of things that can answer,
wearing a note explaining that it can't.

An `/impeccable live` session (2026-07-29) reshaped the panel over six accepted
rounds. The result is two levels: **Answers** holds two mode rows — the built-in
model and your own provider — and exactly one wears the check; the provider
lines below, disclosed by a caret, only set and clear keys. Which provider
answers is picked from the footer chip, where it already was.

The redesign landed in the working tree with `tsc` green and `npm test` red at
12 failures, and a review of it found three real defects rather than only stale
assertions: `keysOpen` was mount-only state that went stale on every mode flip
(stranding a Default-mode player in Custom mode with the key list still shut),
two error paths set a `sourceError` that nothing rendered, and the "Needs a key"
note flashed on every open because it was derived from a boolean that is false
while the fetch is in flight. Two CSS rules meant for the settings rows were
written against `.menu-list` / `.model-row`, which are shared by the game, model
and add-game menus — so the whole product's row density moved.

## Goal / non-goals

- Goal: land the two-level IA with a green suite, the three defects fixed, the
  density contained to Settings, and README / CLAUDE.md / DESIGN.md /
  smoke-checklist telling the truth about the new panel.
- Goal: cover the affordances the redesign introduced and shipped untested — the
  disclosure caret, the "Replace the … API key" line, and the mode↔disclosure
  reconciliation in both directions.
- Non-goal: revisiting the footer chip. The provider pick lives there and this
  plan does not move it.
- Non-goal: the Rust side. `set_mode`, `set_api_key` and `remove_api_key` are
  unchanged; only their callers move.

## Approach

1. Branch `feat/settings-modes-and-keys` off `main` — the redesign was made
   directly in the working tree and must not be committed there.
2. `SettingsMenu.tsx`: strip the mode change out of `handleSaveKey` (see the
   decision doc); reconcile `keysOpen` inside `pickMode`'s `try` so a rejected
   `set_mode` cannot move the disclosure; unify `modeRow` to render both rows,
   which also gives the Custom row the error slot it never had; retag the
   key-status failure so it renders under the heading instead of behind the
   caret; derive `needsKey` from `statuses !== null` so neither note can speak
   before the fetch lands.
3. `styles.css`: scope the airy padding to `.source-row` / `.key-line` (both
   SettingsMenu-only — `.menu--top` is *not*, it is shared with GameMenu and
   AddGameMenu); fix the two icon buttons whose `line-height: 0` collapses them
   while their pending "…" shows; move the caret's focus ring to the inset
   group, since it paints on top of a row; bound the caret rail.
4. Tests: 13, not 12 — `App.zeroKeys.test.tsx` is currently green *because* of
   the auto-switch being removed. Repair what is a stale label, rewrite what
   pinned the old contract, delete what is structurally obsolete, and add the
   missing coverage.
5. Docs, then `/check` + `/vault-lint`, then one `feat:` commit and the
   `docs(vault):` close in the same PR.

Open question: both notes can show at once on a fresh install — "Needs a key" on
the Custom row and "Nothing can answer yet" under the heading. They say different
things and the panel-level one only appears in the truly-dead state, so both
stay for now; suppressing the row note while the lead note shows is a one-clause
change if it reads as noise on device.

## Decisions & trade-offs

- Keys never change the mode → [[2026-07-29_keys-are-not-a-mode-choice]].
- The `keysOpen` seed is `settings.mode !== "default" || !defaultMode.configured`
  rather than the narrower mode check: on a stale-default install the guidance
  says "add a key **below**", and the narrow predicate mounted the list closed,
  pointing at nothing. `defaultMode.configured` is known synchronously at mount;
  `statuses` is not.
- The airy row density from the live session is kept but confined. Applying it
  product-wide would have cost the model menu roughly three visible rows inside
  its fixed 460px drop and contradicted DESIGN.md's normative `menu-row.padding`.
- Defect fixes ride in the same commit as the redesign rather than a follow-up
  `fix:`. The redesign is uncommitted, so splitting them would pin a knowingly
  red commit into permanent history — and history here is merge-commits only.

## Status log

- 2026-07-29 — created; branch cut, work starting.
