---
title: True up the Claude Design mirror to the shipped UI
type: plan
status: done
created: 2026-07-30
updated: 2026-08-02
tags: [frontend, overlay]
related: ["[[2026-07-05_design-tokens-claude-design-sync]]", "[[2026-07-18_impeccable-design-context]]"]
commit:
---

# True up the Claude Design mirror to the shipped UI

## Context / problem
The Claude Design project "WikiLens" (id `822cc090-f734-4977-8175-cbd1a60bd75f`)
is the browser-side mirror of the overlay's design system — token sheet, brief,
and preview cards rendering the app's real CSS over bright/dark game-frame
stand-ins. The sync loop closed 2026-07-14 with a final true-up
([[2026-07-05_design-tokens-claude-design-sync]]); its brief says the app repo
is the source of truth and the project is a snapshot as of the close. Since
then ~40 code commits shipped major visual surface the mirror has never seen:
the Settings panel (answer-mode rows, provider key lines, shortcut recorders),
Default mode's footer chip, the admitted micro-motion inventory (A-01…A-04),
authored focus rings, menu keyboard highlight, the measured `.menu--down`
dropdown, the debug window (its own glass sheet and stylesheet), the status
row's Stop + slow-wiki hint, and copy retirements (Shift+C, the footer
`.chip-hint`). The user asked for the mirror to be analyzed and updated.

Key finding shaping the work: **zero token value drift** — diffing the 07-14
`src/styles.css` against HEAD, the `:root` block is byte-identical except one
rewritten comment. Every new surface is built from existing tokens, so the
true-up is preview cards + docs, not the token sheet.

## Goal / non-goals
- Goal: the mirror truthfully renders today's shipped UI — 7 existing cards
  updated, 4 new cards (settings, debug window, motion, capture region-select),
  brief + remote CLAUDE.md refreshed, token-sheet comments synced, two resolved
  exploration artifacts deleted.
- Goal: the paperwork trail — this plan doc, PR'd per the delivery ADR.
- Non-goal: reopening the design loop (it stays closed; this is a true-up).
- Non-goal: committing the cards to the repo — they remain generated artifacts
  of `styles.css` staged per-session (established 2026-07-05, restated here).
- Non-goal: any change to app code or `DESIGN.md` (already current).

## Approach
1. Reconcile-read the remote cards being edited; drop line items the 07-14
   cards already cover.
2. Stage everything in the session scratchpad (`design-sync/` mirroring remote
   paths); edit pulled remote copies, author new cards from the shipped CSS
   (`src/styles.css`, `src/debug/styles.css`, `capture.html`) with commit
   annotations. New pane group "Windows" for the debug + capture pages.
3. Sanity-check every staged card in a real browser (both halves, all states,
   reduced-motion emulation for the motion card); verify the staged token
   sheet's `:root` still normalizes byte-equal to the app's.
4. Push via DesignSync: finalize_plan (14 writes, 2 deletes) → write_files in
   four batches (tokens + CLAUDE.md → updated cards → new cards → brief last)
   → delete_files → post-push re-read.
5. Close this doc, `/vault-lint`, `/check`, PR.

## Decisions & trade-offs
- Cards stay out of the repo (restating the 2026-07-05 call, not re-deciding):
  a repo copy would be a second source of truth for `styles.css`.
- The resolved game-picker exploration files are deleted from the project —
  the decision is distilled in `previews/game-picker.html` and the full
  history lives in this vault; a closed-loop snapshot shouldn't carry resolved
  exploration artifacts. (User-approved 2026-07-30.)
- `previews/debug.html` inlines `src/debug/styles.css` verbatim rather than
  re-deriving from overlay tokens — the hand-copied subset IS the shipped
  sheet, and the mirror documents reality, duplication included.
- Motion ships as one animated card with replay affordances, honoring the
  shipped `prefers-reduced-motion` gates (static end states, replay no-ops).

## Status log
- 2026-07-30 — created; scope approved (full true-up incl. capture card,
  deletions, colors touch-up, vault doc + PR).
- 2026-07-30 — reconciled + staged the 7 card updates and the comment-only
  token-sheet sync; the 4 new cards were workflow-authored with adversarial
  verification (9 verifier findings, all applied — impossible settings scene,
  wrong game ids, shimmer phase name, capture radius-ladder/exclusivity copy).
- 2026-08-02 — browser pass over all 11 staged cards (served from localhost;
  the Chrome extension refuses `file://`): every state renders, the motion
  card's three Replay buttons each fire and settle their animation, zero
  console errors. Reduced-motion couldn't be OS-emulated from the automation
  tools; verified structurally instead (every animation rule sits behind the
  `no-preference` gate, so Replay re-adding classes is a no-op under reduce).
  Two gaps found and fixed: controls carried the three-anchor story and the
  `.menu--down` rule as CSS comments only (now a visible caption + the
  verbatim rule), and attachment's A-03 pointer to the motion card was
  comment-only (now a caption).
- 2026-08-02 — pushed: finalize_plan (14 writes, 2 deletes) → write_files in
  four batches (tokens → 7 updated cards → 4 new cards → brief last) →
  delete_files. Surprise: the remote `CLAUDE.md` is now a **reserved path**
  DesignSync refuses to write (agent-instruction protection added since the
  07-14 close), so its planned refresh was dropped — the truthful-mirror
  framing lives in `docs/brief.md`; the remote `CLAUDE.md` keeps its stale
  font note. Post-push: `list_files` matches the plan (new Windows-group
  cards present, both exploration files gone, `_ds_*`/fonts/add-game/error
  untouched) and a re-read of `previews/capture.html` round-tripped
  byte-exact. The type card's `../fonts/CascadiaCode.woff2` path is
  unverifiable inside the pane from here; its `@font-face` falls back to
  ui-monospace/Consolas if unresolved.
- 2026-08-02 — closed. `commit:` left empty on purpose: the deliverable
  landed in the Claude Design project (`822cc090-…`), not in repo code; this
  PR carries only the paper trail.
