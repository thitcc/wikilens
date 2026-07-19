---
title: Menu keyboard navigation, authored focus rings, and the answer-ready announcement
type: plan
status: active
created: 2026-07-19
updated: 2026-07-19
tags: [frontend, overlay]
related:
  - "[[2026-07-18_impeccable-design-context]]"
  - "[[2026-07-18_ask-lifecycle-fix-pass]]"
  - "[[2026-07-05_game-picker-owned-menu]]"
  - "[[2026-07-16_model-menu-dropdown]]"
commit:
---

# Menu keyboard navigation, authored focus rings, and the answer-ready announcement

## Context / problem

Promotes the §6 roadmap line "Menu keyboard navigation". The 2026-07-19 impeccable
critique (32/40, local snapshot) pins the three debts this closes:

- **Heuristic 5 / P2 #5** — Enter in a menu silently picks an *invisible* first
  match (`firstVisible` in both GameMenu and ModelMenu), and the owned menus
  dropped the native select's keyboard model entirely: no arrow keys, no visible
  cursor. Tab through 300 OpenRouter rows is not a substitute.
- **P1 #2** — the answer's *arrival* is silent to assistive tech: the
  `role="status"` region is gated on `busy && status`, so it unmounts in the same
  commit the answer mounts — and an unmounting live region can't announce.
- **Sam persona** — every focus ring is a UA default over arbitrary game frames:
  "visible but stylistically orphaned."

The branch name `feat/menu-keyboard-nav` was pre-committed by the critique itself.

## Goal / non-goals

- Goal: Up/Down move a **visible highlight** through the game/model menu rows,
  Home/End jump, Enter picks the highlighted row — never an invisible first
  match; typing keeps filtering (focus never leaves the filter input).
  Acceptance: a test seeded with a non-first selected model passes bare-Enter
  re-pick on the branch and fails on `main`.
- Goal: authored `:focus-visible` accent rings on chips, menu rows, and links,
  replacing the UA defaults; detector stays at zero findings.
- Goal: a one-shot polite "Answer ready — N sources" announcement through the
  existing `role="status"` region on the resolve path only.
- Non-goal: combobox/`aria-activedescendant` semantics — ModelMenu's interactive
  group-header buttons inside the list make a conforming listbox impossible
  as-built; queued as a §6 roadmap follow-up instead (user pick).
- Non-goal: PageUp/PageDown, hover↔highlight syncing, arrow access to group
  headers or the pinned "Add a game…" footer action (all stay Tab-reachable).
- Non-goal: any change to the §5 Esc layering or the focus-discipline system.

## Approach

1. Pure helpers first: `src/menuNav.ts` (`nextHighlight` — clamp, no wrap,
   −1 entry rules) and a `keepRowInView` second export in `src/menuScroll.ts`
   (minimal nearest-edge scroll, same coordinate conversion as
   `centerRowInList`), each with a unit suite.
2. GameMenu: per-render flattened `rows` (the existing section-prefixed keys
   solve the Recent/All duplicate), `highlightKey: string | null` state with the
   effective index **derived** (null + query → first match; null + no query →
   selected row, else row 0), extended filter-input `onKeyDown`
   (Arrow/Home/End/Enter), callback-ref row map merged with `selectedRowRef`,
   filter `onChange` resets highlight + `scrollTop = 0` (keeps the auto-picked
   first match visible — otherwise the invisible-pick bug returns taller).
3. ModelMenu: same pattern; the `rows` derivation replaces `firstVisible()`
   (same traversal generalized — collapsed-group skip inherited, not rebuilt).
4. Focus rings: accent outline (`1px var(--accent)`, 2px offset; −1px inset on
   in-list rows) + `.model-row.is-highlighted { background: var(--surface-control) }`;
   DESIGN.md §5 prose + `.impeccable/design.json` demos updated in the same
   commit (tokens ↔ stylesheet ↔ sidecar move together). No frontmatter change —
   all values are `var()` reuse or ungoverned outline geometry.
5. Announcement: restructure to a single **always-mounted** `role="status"`
   region (wrapper class swaps `status` ↔ `visually-hidden`; Stop stays a
   sibling outside it); `announcement` state set only on resolve, cleared on the
   next submit; no timer.
6. Docs: smoke-checklist §5/§6 rows; CLAUDE.md §2 map + §6 roadmap swap.
7. Gates: `graphify update .`, `/check`, re-run `/impeccable critique the
   overlay panel` (record the score delta here), `/vault-lint`; PR via
   `gh pr create --assignee @me`, human merges.

## Decisions & trade-offs

- **Accent ring over the sidecar's veil-fill focus model** (user pick): a
  veil-only focus style would be indistinguishable from hover *and* from the new
  keyboard highlight — weaker than the UA ring it replaces. DESIGN.md already
  reserves Moonlight Blue "for the interactive, the selected, and the focused";
  the sidecar demos are updated to match rather than waived.
- **Visual-only highlight now, combobox semantics later** (user pick): a
  conforming listbox can't contain ModelMenu's interactive group-header buttons;
  half-correct ARIA on one menu and not its twin would be worse than the honest
  Tab-through-buttons path screen readers keep today.
- **`highlightKey` + derived index over a stored index**: survives async list
  arrival, collapse/expand, and filtering with zero clamping effects; a vanished
  key degrades to "no highlight" instead of pointing at the wrong row.
- **Initial highlight = selected row**: bare Enter becomes a harmless *visible*
  re-pick, keeping the critique-praised type-then-Enter accelerator with a
  visible target. Clamp at the ends (no wrap) — Home/End cover the jumps.
- **No timer on the announcement**: stale visually-hidden text is harmless, and
  a timer would drag fake-timer discipline into every ask test for zero benefit.

## Status log

- 2026-07-19 — created; plan approved (plan mode), branch `feat/menu-keyboard-nav` opened.
