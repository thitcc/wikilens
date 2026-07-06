---
title: Design tokens and a Claude Design sync loop for the overlay UI
type: plan
status: blocked
created: 2026-07-05
updated: 2026-07-05
tags: [frontend, overlay]
related: []
commit: [65b7f4d, c59fda6, 2ae9c03, fb1fe67]
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
- 2026-07-05 — steps 2–3 (setup + sync up) done: Claude Design project
  "WikiLens" created (id `822cc090-f734-4977-8175-cbd1a60bd75f`) and seeded with
  `docs/brief.md` (constraints + deliverables + opening prompt),
  `tokens/wikilens-tokens.css` (the `:root` block verbatim from `65b7f4d`), and
  six preview cards (`previews/{panel,controls,answer,error,colors,type}.html`)
  rendering the app's real CSS over bright/dark gradient game-frame stand-ins —
  a light take on the deferred preview pages, not the composited-screenshot
  version. Cards were staged in the session scratchpad, deliberately not
  committed to the repo: they're generated artifacts of `styles.css`, and a
  repo copy would be a second source of truth (promote a `design/` folder plus
  a decision doc only if the sync loop matures). Still blocked: waiting on the
  user's browser iteration; step 4 = pull the agreed system back as token
  edits.
- 2026-07-05 — first full pull-back (step 4 for one decision) landed in
  `c59fda6`: the browser iteration produced a "Floating Panel Exploration"
  (variants 1a released / 1b centered card / 1c borderless) and chose **1a** —
  panel detached top-right, 12px gap, 70% height, radius on all corners,
  border on all sides, new `--shadow-panel: 0 12px 32px rgba(0,0,0,0.32)`; no
  other token values changed. Implementation splits across runtimes:
  `window.rs` (`position_top_right`, `PANEL_GAP`/`SHADOW_ROOM_*`/
  `PANEL_HEIGHT_FRAC`) sizes the window around the panel + shadow apron; CSS
  margins carve the same regions (new CLAUDE.md §5 gotcha). Verified: Vite +
  tsc clean, 53 offline tests pass. Pushed back to the project: updated token
  sheet, floating panel card, and `fonts/CascadiaCode.woff2` (SIL OFL, from
  microsoft/cascadia-code v2407.24) for the pane's missing-font warning. The
  loop stays open for the remaining deliverables (accent directions,
  high-contrast variant); user should smoke-test the float in-game — gap and
  height fine-tuning is now a constants edit.
- 2026-07-05 — in-game smoke test caught a float regression, fixed in
  `2ae9c03`: React's `#root` mount node sat outside the flex chain, so the
  panel collapsed to content height while loading and overgrew on long
  answers; `#root` now passes the stretch through and the panel holds 70% in
  every state.
- 2026-07-05 — refined after in-game use (`fb1fe67`): the panel now hugs its
  content and treats 70% as a cap (`align-self: flex-start` + `max-height`),
  so idle/loading states wrap their few lines instead of holding an empty
  sheet, and long answers scroll at the cap. Mirror updated in the design
  project (panel card + tokens comment).
- 2026-07-05 — second full pull-back: the user's exploration 3a (footer model
  chip + combined model menu) shipped in `3cef83e` — see
  [[2026-07-05_model-picker-menu]]. Mirror kept truthful: token sheet gained
  `--surface-menu` / `--shadow-menu` / `--menu-clearance` (a new
  paired-constants note) and the controls card is marked SHIPPED with the
  in-app offline-list state added. Loop still open for the remaining
  deliverables (accent directions, high-contrast bright-scene variant).
- 2026-07-05 — third mirror push, after the add-game flow shipped in
  `513c072` and passed the user's in-game smoke test — see
  [[2026-07-05_user-added-game-wikis]]. Token sheet gained
  `--menu-clearance-top` (the top-anchored paired-constant twin); the
  controls card's picker row became the `game-controls` group (picker + "+"
  trigger, normal/disabled); and a new `previews/add-game.html` card renders
  the popover's five states (suggestions found with host notes, probing,
  nothing-found + manual URL, probe error, your-games with remove). This was
  a code-first component (built on the shipped menu pattern, no browser
  exploration round) — the mirror now catches it up, and a design pass over
  its states can ride the still-open loop with the accent/high-contrast
  deliverables.
- 2026-07-05 — mirror fix + new exploration queued. The Glass-panel card
  renders its own header markup, so the third push had left it without the
  "+" trigger — `previews/panel.html` now shows `game-controls` (picker +
  "+") on both halves. In-game screenshot review surfaced a real defect the
  tokens can't fix: the game picker's **native `<select>` popup** renders as
  an unreadable white system list over the glass (WebView2 draws it outside
  the page — CSS can never reach it). Queued the fix as a browser
  exploration: `docs/game-picker-exploration.md` pushed to the project with
  constraints + an opening prompt asking for variants **4a** (single game
  chip opening a combined game menu: filter, Your-games section, "Add a
  game…" action row), **4b** (keep the two-control header, own only the
  dropdown surface), **4c** (a divergent presentation), each shown open over
  the bright and dark stand-ins. Pull-back lands as a GamePicker → GameMenu
  swap on the shipped menu pattern once the user picks a direction.
- 2026-07-05 — exploration 4 decided in the browser: **4c ships** ("game as
  identity" — quiet header chip mirroring the model chip, Recent pinned,
  monogram tiles, add-game as the pinned menu action). The design side
  already updated the mirror itself: `--menu-clearance-top` retuned 52 →
  42px in the token sheet (verified on pull), the menu height cap
  (`calc(100% - top - bottom clearance)`) documented, and the decision
  distilled into `previews/game-picker.html`. Code pull-back planned in
  [[2026-07-05_game-picker-owned-menu]]; panel/controls preview cards still
  show the old select+"+" header and get updated when it ships.
- 2026-07-05 — 4c pull-back shipped in `3ffc78d` (third full pull-back
  through the loop; details in [[2026-07-05_game-picker-owned-menu]]).
  Mirror trued up: panel + controls cards now render the chip-height header
  (select and "+" gone, `.is-open`/`.is-disabled` chip states added) and the
  game-picker card is marked SHIPPED. Loop still open for the original
  remaining deliverables (accent directions, high-contrast bright-scene
  variant).
