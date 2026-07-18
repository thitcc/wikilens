# Manual smoke checklist

Run this before each release, and after touching `window.rs`,
`debug_window.rs`, `hotkey.rs`, `tray.rs`, the overlay CSS, or
`capabilities/*.json`. It is the compensating
control for the surface automation can't reach — a real overlay over a real
game on a real monitor (see
`vault/2026-07-13_manual-smoke-checklist-live-cadence.md` for the rationale).
Copy the boxes into the release notes/log and tick them as you go.

## Prerequisites

- A game running in **borderless/windowed** mode (exclusive fullscreen is only
  needed for item 7).
- At least one provider API key configured; for item 6 you need both a
  vision-capable and a text-only model reachable from the model menu.
- If available, a second monitor at **>100% DPI scaling** for item 3 — if not,
  note "single monitor" in the run log.
- Items 1–8 run on `npm run tauri dev` (item 8 needs `WIKILENS_DEBUG=1` set);
  item 9 runs on the packaged installer from `npm run tauri build`.

### 1. Hotkey + tray

- [ ] Shift+C over the borderless game toggles the overlay: panel slides in
      from the right edge and the prompt takes focus; Shift+C again hides it.
- [ ] Tray icon is present; Show/Hide works; Quit exits the app (the panel's
      close paths only hide it).

### 2. Hotkey swallow (expected behavior)

- [ ] While the app runs, a capital `C` (Shift+c) cannot be typed into the
      prompt — expected, the global hotkey swallows it system-wide. Lowercase
      `c` types fine and search is case-insensitive.

### 3. DPI / multi-monitor placement

- [ ] The panel floats top-right with the correct gap on each monitor,
      including the >100% scale display — no drift off the edge, no clipping.

### 4. Transparency

- [ ] No black rectangle behind the glass panel (transparency regression).
- [ ] The drop shadow is fully visible, not clipped at the window edge.

### 5. Menus + Esc layering

- [ ] Game, model, and add-game menus open and close; only one menu is open at
      a time.
- [ ] With a menu open, the first Esc closes the menu (panel stays); the
      second Esc hides the panel.

### 6. Ask round-trip

- [ ] Status walks Searching → Reading → Answering; the answer streams in as
      markdown with 2–4 source links.
- [ ] Clicking a source opens it in the system browser (not inside the panel).
- [ ] Screenshot attach works on a vision-capable model, and is blocked with a
      clear message on a text-only model.

### 7. Exclusive fullscreen (expected failure)

- [ ] An exclusive-fullscreen game covers the overlay. Verify this is still
      documented (README caveats, CLAUDE.md gotchas) — do **not** log it as a
      bug.

### 8. Debug window (`WIKILENS_DEBUG=1`)

- [ ] With the flag set, an opaque "WikiLens Debug" window opens top-left at
      launch, above the borderless game. Without the flag: no window, and no
      "Show debug panel" tray item.
- [ ] An ask streams in live: the provisional row's bar animates, completed
      phases settle into relative-width bars, the outcome badge lands — and
      the stderr table still prints, unchanged.
- [ ] Clicking, scrolling, and expanding groups in the debug window never
      steals keyboard focus from the game (keep typing in the game while
      interacting with it).
- [ ] Closing the window hides it; tray → "Show debug panel" re-shows it
      **without activating it**, with the session history intact.
- [ ] Drag it to the >100% scale monitor: text renders sharp and the window
      geometry stays sane.

### 9. Packaged build

- [ ] The `npm run tauri build` installer runs and the installed app launches.
- [ ] With no `.env` present, API keys resolve from OS environment variables
      and an ask succeeds (`.env` is dev-only; a packaged app's cwd is
      unpredictable).
- [ ] Source links still open in the browser (the opener URL scope is present
      at runtime — this fails as `ForbiddenUrl`, not at compile time).

## Live test suite cadence

`cargo test -- --ignored` from `src-tauri/` runs the 10 network-hitting
`#[ignore]`d tests: the golden-query hit@4 suite and the wiki round-trips in
`wiki/mod.rs`, probe validation in `wiki/probe.rs`, and the OpenRouter catalog
parse in `models.rs`. Run it:

- **(a) before each release**, alongside this checklist;
- **(b) when adding or changing a game or provider** — a new game's golden
  case is also enforced offline (`wiki/mod.rs::tests`);
- **(c) roughly monthly** otherwise, so wiki drift (a dead endpoint, a changed
  `articlepath`, a ranking shift) surfaces between releases.

A **strict** golden case missing means real drift — fix the registry/retrieval
or document why. A known-gap case (`strict: false`) flipping either way is
recorded in `vault/2026-07-04_golden-query-retrieval-tests.md`'s status log.
CDN-fronted wikis occasionally throw a transient 403 mid-suite — rerun once
before treating it as drift. This suite is deliberately **not** CI-scheduled:
it needs the network and polite request pacing, and a monthly habit here buys
more than a cron hitting fan wikis.
