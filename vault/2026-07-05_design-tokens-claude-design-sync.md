---
title: Design tokens and a Claude Design sync loop for the overlay UI
type: plan
status: blocked
created: 2026-07-05
updated: 2026-07-05
tags: [frontend, overlay]
related: []
commit: 65b7f4d
---

# Design tokens and a Claude Design sync loop for the overlay UI

## In simple terms
The panel works, but its look was never *designed* — colors, sizes, and spacing
were typed in as the code was written. Step one gives every visual choice a name
(design tokens) without changing anything visible, so the look becomes a short
list of knobs instead of values scattered through a stylesheet. Step two connects
the repo to claude.ai/design: the design system lives there as something you can
browse and react to in the browser, and agreed changes sync back into the app as
token edits. We deliberately skip building preview pages for now — changes get
judged in the running overlay over a real game instead.

## Context / problem
With the retrieval series done (answer quality), attention turns to the UI
(2026-07-05 discussion). All styling is a single hand-written `src/styles.css`:
a few ad-hoc custom properties (`--fg`, `--fg-muted`, `--accent`, `--border`,
`--error-*`) but most values hard-coded — rgba() glass surfaces, radii of
5/8/10/12px, font sizes 12/12.5/13/14px, paddings and gaps per rule. Any restyle
today is a scattered edit with no single place that states the design. Three
constraints are specific to WikiLens and must shape any design work:
- The glass panel floats over an unpredictable game frame (bright snow one
  minute, a dark cave the next) — legibility at both extremes is the binding
  constraint.
- `html, body` must keep `background: transparent` or the window renders as a
  black rectangle (CLAUDE.md §5).
- The webview CSP blocks CDN assets — any custom font ships bundled, never
  fetched.

## Goal / non-goals
- **Goal:** every visual decision in `styles.css` reads from a named token layer
  (surface, ink, accent, status/error, border, radius, blur, spacing, type
  scale). The extraction itself lands with **zero visual diff**.
- **Goal:** a Claude Design design-system project holds the WikiLens system, with
  an incremental DesignSync loop so design iteration happens in the browser and
  lands back in the repo as token edits.
- **Goal:** one deliberate design pass through that loop, verified in the running
  overlay over bright and dark game scenes.
- **Non-goal (for now):** preview pages / state cards composited over game
  screenshots — deferred by choice (2026-07-05); revisit if judging changes
  in-app proves too slow.
- **Non-goal:** frontend behavior, hotkey, window, or Rust changes.

## Approach
1. **Token extraction** — inventory every literal in `styles.css`; promote to
   `:root` custom properties with small scales (spacing, radius, type). Verify
   zero visual diff in `npm run tauri dev`.
2. **Claude Design setup** — `DesignSync list_projects` (first call prompts to
   grant design-system access on the claude.ai login); create a "WikiLens"
   design-system project if none fits.
3. **Sync up** — push the token definitions and per-component specs (panel,
   pickers, prompt input, status line, answer markdown, error, sources) as
   project files; iterate on the system in the browser.
4. **Pull back + verify** — land agreed changes as token edits; check the overlay
   over a bright and a dark game scene; keep the transparent-body and
   bundled-font gotchas intact.
- Open questions: is a bundled display face worth the licensing/size cost over
  staying `system-ui`? Should panel opacity/blur get a high-contrast variant for
  bright scenes?

## Decisions & trade-offs
- Preview pages were part of the original recommendation and are deliberately
  deferred — iteration happens against the app itself.
- The Claude Design loop is chosen over a direct one-shot restyle for visual
  steerability and reuse, at the cost of project setup and sync overhead. If a
  real fork appears during execution (e.g. bundling a webfont vs `system-ui`),
  split it into a decision doc and link it here.

## Status log
- 2026-07-05 — created from the design-enhancement discussion; preview pages
  dropped from scope for now; queued as todo.
- 2026-07-05 — step 1 (token extraction) landed in `65b7f4d`: complete `:root`
  token layer (ink, glass surfaces, status, shape, type, rhythm, states); only
  hairline `1px` border widths stay literal. Zero visual diff verified by value
  tally on the git diff; Vite build + tsc clean. Status → blocked: steps 2–4
  (DesignSync project, sync loop, design pass) wait until the user returns with
  their Claude Design setup.
