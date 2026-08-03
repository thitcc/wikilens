---
title: A keyed provider line is static — Set pill, trash-only, re-key = trash then add
type: decision
status: done
created: 2026-08-02
updated: 2026-08-02
tags: [frontend, overlay]
related: ["[[2026-07-29_keys-are-not-a-mode-choice]]", "[[2026-07-29_settings-modes-and-keys]]", "[[2026-08-02_keyed-line-set-pill]]"]
commit: c1e9d37
---

# A keyed provider line is static — Set pill, trash-only, re-key = trash then add

## Context
Real use falsified two Settings → Answers assumptions at once. The owner
keyed a provider and could immediately click the line open again to paste
over it — the "Replace the … API key" affordance contradicted both the
mental model (a stored key is a fact, not a draft) and the component's own
header comment ("a stored key's only action is the trash"). And DESIGN.md's
"quietest signal the system can make" — ink strength alone saying whether a
key is stored — proved illegible on the actual glass: the owner could not
tell set from unset. This decision reverses that documented ink-only choice
(DESIGN.md "Rows, the one exception") and narrows the key-line contract the
2026-07-29 settings split introduced.

## Decision
A stored key's line is static — a neutral "Set" Badge marks it, the trash is
its only control, and re-keying means trash, then the now-unkeyed line.

## Consequences
- Good: no accidental replace-field on a keyed line; key state is legible at
  a glance without a neighbor to compare inks against; the header comment's
  "only action is the trash" is finally true in code; the accent check stays
  unique to the mode rows (One Signal Rule — the pill is neutral).
- Cost / bad: re-keying is two gestures (trash, then paste); keyed rows lose
  a tab stop (deliberate — there is nothing to activate); a static div can't
  wear `:disabled`, so keyed lines no longer dim under `busyAll` (only the
  trash deadens — acceptable, a static line has no affordance to suspend).
- Follow-ups: none expected; focus after removal lands on the fresh "Add a
  key" button so keyboard re-keying stays one Enter away.

## Alternatives considered
- Guard `toggleKeyForm` but keep the keyed `<button>` — rejected: a button
  that ignores clicks with a frozen `aria-expanded` and a "Replace…" label
  is an accessibility lie.
- Accent pill — rejected: Moonlight Blue means interactive/selected (One
  Signal Rule); storage is not selection.
- Auto-open the empty field after trash — rejected by the owner: removal
  stays removal; an auto-opened field would also sit outside the layered-Esc
  contract.
