---
title: True up the Claude Design mirror to the shipped overlay, second pass
type: plan
status: active
created: 2026-09-05
updated: 2026-09-05
tags: [frontend, overlay]
related:
  - "[[2026-07-30_design-mirror-true-up]]"
  - "[[2026-08-12_theme-switching-micrographics]]"
  - "[[2026-08-14_draggable-overlay-position-modes]]"
  - "[[2026-08-24_replace-default-mode-with-local-ai]]"
  - "[[2026-08-04_game-auto-detection]]"
commit:
---

# True up the Claude Design mirror to the shipped overlay, second pass

## Context / problem
The Claude Design project "WikiLens" (id `822cc090-f734-4977-8175-cbd1a60bd75f`)
mirrors the overlay's design system in the browser — token sheet, brief, and
preview cards rendering the app's real CSS over game-frame stand-ins. Its last
true-up pushed on 2026-08-02 ([[2026-07-30_design-mirror-true-up]]). The
Micrographics plan ([[2026-08-12_theme-switching-micrographics]]) ran its
design pass in the project and left the post-merge true-up as a follow-up
(proposal token sheet → shipped block, the 0.14em tracking fix, the
ruler/barcode marks dropped from the cards); it never ran. Since 08-02 about
thirty front-end commits shipped surface the mirror has never rendered: the
content-hugging panel and the header Start over (08-03), the History menu
(08-03), the version stamp on the Answers heading (08-04), the game-suggestion
chip and A-05 (08-05), the Theme picker and the Micrographics theme
(08-12/13), the settings steppers with popover, rail and frame plus A-06
(08-13), anchor-aware layout, the Position stepper, padlock and header drag
with the placement-aware A-01 (08-14), and Local AI replacing Default mode —
server rows, the vision eye, the `Local AI · model` footer chip (08-25).

Token drift, unlike 07-30, is real: the `:root` block's `--shadow-room-*`
moved with the symmetric apron, and the whole `:root[data-theme="micrographics"]`
block (incl. `--line`) is new. The remote
`tokens/wikilens-tokens-micrographics.css` is the 08-12 proposal
(`--tracking-brand: 0.12em`); the app shipped 0.14em.

`DESIGN.md` was swept with each feature and is nearly current; the audit found
two gaps, fixed here: §1 counted six admitted motions (the §6 inventory is
A-01…A-06 plus the debug shimmer — seven, six on the overlay; the sidecar
already said so), and §5's icon inventory lacked the padlock and the vision
eye.

## Goal / non-goals
- Goal: the mirror renders today's shipped UI in both themes — token sheet
  current (default + Micrographics blocks byte-equal to `src/styles.css`), the
  proposal sheet retired, every card re-inlined with a Default/Micrographics
  toggle, the settings/panel/controls/motion/colors/type cards carrying the
  surface listed above, a History card, the brief re-dated.
- Goal: `DESIGN.md` §1 and §5 corrected (this PR).
- Non-goal: reopening the design loop; any app code change; committing cards
  to the repo (restated from 2026-07-05 and 07-30 — they are generated
  artifacts of `styles.css`).
- Non-goal: theming the debug and capture pages in the mirror — they stay on
  the default appearance by design (DESIGN.md §8), and their cards say so.

## Approach
Two tracks, landing in different places:
1. **Repo (this PR):** this doc + the two `DESIGN.md` fixes. No token changes,
   so the frontmatter and `.impeccable/design.json` stand.
2. **Mirror (DesignSync, the 07-30 ritual):** the owner runs `/design-login`
   (the tool refuses even reads without design-system authorization) →
   `list_files` + `get_file` reconcile (settle whether the 08-03 "mirror
   updated" note was a push, and whether a `/design-sync` skill is available)
   → stage in the session scratchpad (`design-sync/` mirroring remote paths):
   token sheet from HEAD's `:root` + theme block, cards re-inlined with the
   `data-theme` toggle (Micrographics halves hardcode the JS-transformed
   labels; no ruler strip or barcode), new `previews/history.html`, brief
   re-dated → browser pass from localhost, both themes, replay buttons, zero
   console errors → `finalize_plan` (writes = tokens + cards + brief; deletes
   = the proposal sheet) → `write_files` in batches (tokens → updated cards →
   new cards → brief last) → `delete_files` → `list_files` + one `get_file`
   round-trip. The remote `CLAUDE.md` is a reserved path — never in the plan.

## Decisions & trade-offs
- Cards stay out of the repo (restated, not re-decided).
- Both themes on every card through the app's own `data-theme` mechanism
  rather than a separate Micrographics card: the cards inline the shipped
  CSS, so the toggle is the truthful mirror and costs no second markup.
- The proposal token sheet is deleted, not updated: the shipped block now
  lives in the main sheet, and a closed pass shouldn't keep a superseded
  proposal beside it (the 07-30 precedent for resolved exploration
  artifacts).

## Status log
- 2026-09-05 — created from the owner's "does the Claude Design mirror need
  an update?" audit. Track A (DESIGN.md §1 count, §5 icon inventory) landed
  in this branch; Track B waits on `/design-login`.
