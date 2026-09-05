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
  needed for item 9).
- At least one provider keyed from **Settings → Answers**; for item 6
  you need both a vision-capable and a text-only model reachable from the model
  menu. Item 7 additionally needs a running **Ollama** (or another local
  OpenAI-compatible server) with at least one model pulled (see README's
  "Local AI mode" section).
- If available, a second monitor at **>100% DPI scaling** for item 3 — if not,
  note "single monitor" in the run log.
- Items 1–10 run on `npm run tauri dev` (items 8 and 10 need
  `WIKILENS_DEBUG=1` set); item 11 runs on the packaged installer — for a release, the one downloaded
  from the CI draft Release (`docs/release.md` §5); otherwise a local
  `npm run tauri build`.

### 1. Hotkey + tray

- [ ] **Ctrl+`** (the key left of 1) over the borderless game toggles the
      overlay: panel slides in from the right edge and the prompt takes
      focus; Ctrl+` again hides it.
- [ ] The **first** summon after a fresh app launch plays that same
      right-edge slide — no extra fade + upward rise layered over it (that
      would be Windows' one-time window-open transition, force-disabled in
      `window.rs`).
- [ ] Tray icon is present and its tooltip names the current summon combo;
      Show/Hide works; Quit exits the app (the panel's close paths only hide
      it).
- [ ] With the panel open and **idle**, click the game just below the glass
      (where the full-cap window used to sit): the click reaches the game and
      focus leaves WikiLens — the window now hugs the panel
      (vault/2026-08-03_window-follows-panel-height.md).

### 2. Hotkey configuration

- [ ] A capital `C` **types into the prompt** (regression check: the old
      Shift+C default swallowed it system-wide — that trap must stay dead).
- [ ] Header gear → **Settings** → Shortcuts → **Change** on Summon, press a test combo
      (e.g. Ctrl+Alt+P): the row updates, the old combo stops toggling, the
      new one toggles — and still does after an app restart. **Reset**
      restores Ctrl+` (and the tray tooltip follows).
- [ ] While the recorder is armed, pressing the current summon combo does
      **not** toggle the panel (registrations are suspended); after Esc
      cancels, it toggles again.
- [ ] Recording a combo another app owns (e.g. one registered by a running
      tool) is refused with the "another app may already be using it"
      message; the previous combo keeps working.

### 3. DPI / multi-monitor placement

- [ ] The panel floats top-right with the correct gap on each monitor,
      including the >100% scale display — no drift off the edge, no clipping.
- [ ] On the >100% scale display, stream a long answer: the window grows to
      the 70% cap with no 1px grow/shrink jitter (the DPI report ping-pong
      guard).

### 3b. Position: anchors, drag, padlock

- [ ] Settings → **Position**: each of the five anchors places the panel with
      the same 12px visual gap it has top-right today — on the primary AND
      the >100% scale monitor (the apron overhang math is per-monitor).
- [ ] On a **bottom** anchor: open a menu — the panel must not jump (it stays
      pinned to the bottom edge while the window expands), and the game/model
      menus open **upward**. Stream a long answer: the panel grows upward,
      bottom edge glued.
- [ ] On **Center**: stream a long answer — the panel grows symmetrically
      around its center, no walk toward either edge.
- [ ] Entrance motion follows the placement: right anchors slide from the
      right, left anchors from the left, Center and Manual rise — and the
      first summon after launch plays the same motion as later ones.
- [ ] Drag the panel by the WikiLens row and by the band above it (up to the
      window's top edge) — the grab hand shows across that whole band. The
      glass below the header, the section gaps, and the apron at the sides
      must NOT drag. The chips riding the header still click — gear, history,
      game chip, the suggestion chip — buttons/text inputs everywhere keep
      working, answer text still selects, and a double-click on the header
      does **not** maximize.
- [ ] Drag while anchored: the Settings stepper reads **Manual** on its next
      open (the auto-flip), and the remembered anchor is still one step away
      — stepping back to it snaps the panel home; stepping to Manual again
      returns to the dragged spot.
- [ ] Drag mid-stream: the panel must NOT snap back while the answer grows
      (the applied-target guard) — the window only resizes under the cursor.
- [ ] Drag the panel near the screen **bottom** and open Settings/game
      menus: they open **upward** and nothing clips at the screen edge (the
      low drop pins its bottom edge — the bottom-anchor behavior); stream
      an answer there and the panel grows upward. A **high** drop keeps
      menus opening downward.
- [ ] Drop the panel partway **off any screen edge**: on release it snaps
      fully back on-screen. Then open Settings there and let an answer
      stream: the panel must not move at all (the teleport-to-anchor
      regression — an off-screen spot used to fail validation on the next
      height report).
- [ ] With two monitors, drop the panel straddling the seam with the bigger
      share (~60%) on the second monitor: it pops fully onto the second
      monitor, not back to the primary — and it lives there freely
      afterwards.
- [ ] Quit (tray) and relaunch on Manual: the panel summons at the dragged
      spot. Drag it to the second monitor, restart: same. Unplug that
      monitor, summon: the panel falls back to the remembered anchor —
      re-plug and it returns to the dragged spot (settings were not
      rewritten).
- [ ] **Padlock** (Position heading's right rail): locked = the header
      offers no drag (arrow cursor, drag does nothing) in every mode,
      Manual included; the stepper still switches placements. Unlock
      restores the drag. The lock survives a restart.
- [ ] Capture (Ctrl+Shift+C) while Manual on the second monitor: the
      snapshot freezes the monitor under the cursor as always, and the
      capture flow's re-show restores the dragged position.

### 4. Transparency

- [ ] No black rectangle behind the glass panel (transparency regression).
- [ ] The drop shadow is fully visible, not clipped at the window edge —
      including below a **short, idle** panel (the hugged window must still
      hold the 44px shadow apron).

### 5. Menus + Esc layering

- [ ] Game, model, and add-game menus open and close; only one menu is open at
      a time.
- [ ] From an **idle** panel, every menu opens full-size (the window expands
      to the cap on open, shrinks back on close) — and afterwards a click
      below the panel reaches the game again.
- [ ] With a menu open, the first Esc closes the menu (panel stays); the
      second Esc hides the panel.
- [ ] Opening a menu highlights the selected row (veil fill); Up/Down move
      the highlight, Home/End jump to the ends, and Enter picks the
      highlighted row — typing meanwhile keeps filtering, with the highlight
      re-anchoring on the first match.
- [ ] In a long model list (expand OpenRouter), holding ArrowDown scrolls the
      highlight into view (nearest-edge, no recentring) — and reopening the
      menu still centers the selected row (the center-on-open regression
      jsdom can't catch).
- [ ] Tab walks the chips, menu rows, and group headers with the authored
      1px Moonlight Blue focus ring (no UA default ring anywhere over the
      glass); pointer clicks never show the ring.

### 6. Ask round-trip

- [ ] Status walks Searching → Reading → Answering; the answer streams in as
      markdown with 2–4 source links.
- [ ] Clicking a source opens it in the system browser (not inside the panel).
- [ ] Screenshot attach works on a vision-capable model, and is blocked with a
      clear message on a text-only model.
- [ ] With Narrator or NVDA running, the phases are spoken and the resolve
      announces "Answer ready — N sources" once; a cancelled (Stop) or failed
      ask announces no answer-ready.
- [ ] The header's whirl-arrow (**Start over**) appears once there's a draft,
      answer, error, or screenshot; clicking it returns the default panel
      (placeholder, empty prompt, no attachment strip) with focus in the
      prompt — and mid-ask it also stops the stream, with nothing
      reappearing when the ask settles.
- [ ] Ask two questions, then open the header's clock (**History**) and pick
      the older one: its answer, sources, and question reappear **instantly**
      (no status phases), the game chip switches to that ask's game, and
      Enter re-asks it (vault/2026-08-03_answer-history.md). **Clear
      history** empties the menu and the chip disappears — the answer on the
      panel stays.

### 7. Answers: the stepper, the key lines, and Local AI

- [ ] Settings → **Answers** is a game-style stepper: **◁ Value ▷**. Each
      arrow flips between **Custom API** and **Local AI** — and keeps going
      past the end (the cycle wraps; no dead arrow at either edge). Clicking
      the center value opens a floating popover listing the options with the
      check on the current one; picking one closes it, and it arrives with a
      small drop (flip near the card's bottom edge rises instead). Esc closes
      the popover first, the menu second, the overlay third.
- [ ] The key lines exist exactly while **Custom API** answers: cycle to
      **Local AI** and they leave, cycle back and they return. In Custom API,
      clicking an **unkeyed** provider line opens its key field in place (only
      one field open at a time; clicking another line collapses it and drops
      what you typed). **Save** collapses the field and moves **nothing** —
      the stepper's value stays exactly where it was.
- [ ] The keyed line then wears a neutral **Set** pill at full ink, no longer
      opens anything when clicked (no hover veil, arrow cursor), carries a
      trash icon that turns rose on hover, and still answers after an app
      restart (the DPAPI store survives; no re-paste) — the key itself never
      shows anywhere again. Trash it: the line returns to click-to-add (no
      field auto-opens) and keyboard focus lands on that line, not the panel.
- [ ] Click the trash and watch the button, not the row: it holds its box while
      the "…" shows. (jsdom can't see a collapsed line box; this needs eyes.)
- [ ] With Ollama running, cycle to **Local AI**: the section shows the
      server address (prefilled `http://localhost:11434/v1`), the optional
      **API key** line ("Optional" on its rail), and the eye on the **Local
      AI** heading. The footer chip reads **Local · Choose a model**; its menu
      lists the models `ollama list` shows, and picking one relabels the chip.
      An ask streams end-to-end from the local model (debug table: `local /
      <model>`; the wire never leaves the machine).
- [ ] Saving a scheme-less address (`localhost:11434`) round-trips to
      `http://localhost:11434/v1` in the field; clearing the field and saving
      restores the default. Stop the Ollama service and open the model menu:
      the group shows "Couldn't reach your local AI server at … — is it
      running?" instead of a silent empty list (no "offline list" badge).
- [ ] The capture chip follows the Local AI eye: disabled with the "can't
      read images" copy while the eye is off (the copy names the eye, not the
      Image badge), armed after flipping it on — and the flip survives a
      restart.
- [ ] Cycling back to **Custom API** restores the provider/model chip with
      the provider you had picked before (a Local pick never clobbers it),
      and the Local pick is still there when you return.
- [ ] The trash on the answering provider's key line (one click, no
      confirmation) drops it from the model menu; with zero keys the footer
      shows **Set up a model** and it opens Settings.
- [ ] Paste a garbage key and ask: the vendor's 401 shows verbatim in the
      error box, and the model menu degrades to the "offline list" note.
- [ ] Upgrade path: a `settings.json` still holding `"mode": "default"`
      (the removed Built In) relaunches into Custom API with a one-line
      stderr notice naming the removal — nothing else in the file is touched.

### 8. Game detection (the header suggestion chip)

Runtime-only surface — the matcher is unit-tested, but nothing can exercise a
real game in CI. The important item is #2: a false positive is the only
failure that costs anything.

- [ ] Summon over a game with a rule (`src-tauri/src/detect/rules.rs`) while
      the chip shows a *different* game → an accent chip appears left of the
      game chip naming the running game. Click it → the game chip switches and
      the suggestion disappears.
- [ ] Summon over a browser, over the desktop, over the Steam client, and from
      the tray (menu item and left-click) → **no suggestion appears and the
      game chip does not move**.
- [ ] Summon over a known game and *ignore* the chip; hide and re-summon → the
      suggestion is still offered and the selection is still untouched.
- [ ] Start an ask, Esc mid-stream, re-summon over a different known game →
      no suggestion while it streams (the answer and its sources stay
      coherent); it returns once the ask settles.
- [ ] Summon during a game's launcher/loading phase (Conan Exiles, Warframe)
      → no suggestion, no error. Expected: their Steam launch targets are
      launcher processes that own their own windows.
- [ ] With `WIKILENS_DEBUG=1`, each summon logs `detected <id>` or
      `detected nothing` with a running hit/miss tally. Record what a Game
      Pass / Microsoft Store title resolves to, and whether a BattlEye title
      (Conan Exiles) comes back denied.

### 9. Exclusive fullscreen (expected failure)

- [ ] An exclusive-fullscreen game covers the overlay. Verify this is still
      documented (README caveats, CLAUDE.md gotchas) — do **not** log it as a
      bug.

### 10. Debug window (`WIKILENS_DEBUG=1`)

- [ ] With the flag set, **nothing extra shows at launch** (the debug
      window starts hidden, like the overlay). The overlay footer's
      **Debug** chip opens a glass "WikiLens debug" panel at the monitor's
      top-left corner, above the borderless game — no black rectangle
      behind it, drop shadow visible on all sides while it floats
      mid-screen (transparency regressions apply here too). Without the
      flag: no window, no "Show debug panel" tray item, and no Debug chip
      in the overlay footer.
- [ ] Dragging by the panel's header moves the window **without stealing
      keyboard focus from the game** (keep typing in the game right after a
      drag). Resizing from the invisible window edges (just outside the
      glass) still works.
- [ ] The overlay footer's **Debug** chip hides the window; clicking it again
      re-shows it — both without activating it — and it works mid-ask.
- [ ] An ask streams in live: the provisional row's bar animates, completed
      phases settle into relative-width bars, the outcome badge lands — and
      the stderr table still prints, unchanged.
- [ ] Clicking, scrolling, and expanding groups in the debug window never
      steals keyboard focus from the game.
- [ ] Alt+F4 / tray → "Show debug panel" round-trip keeps the session
      history intact — including asks run while the window was hidden.
- [ ] Drag it to the >100% scale monitor: text renders sharp and the window
      geometry stays sane.

### 11. Packaged build

- [ ] The packaged installer (the CI draft's download for a release; a local
      `npm run tauri build` otherwise) runs and the installed app launches.
- [ ] First launch populates the game chip and the footer with **no error
      box** — the packaged-boot state race regression ("state not managed",
      vault/2026-08-02_packaged-boot-state-race.md): the bundled frontend
      boots fast enough to race setup, which dev never reproduces.
- [ ] With no `.env` present, a Custom-mode ask succeeds from the
      DPAPI-stored key alone (keys never ride env; the store lives in
      app-data).
- [ ] With Ollama running, a **Local AI** ask succeeds in the installed app
      (the mode's whole config lives in `settings.json` + the DPAPI store —
      nothing rides env).
- [ ] Source links still open in the browser (the opener URL scope is present
      at runtime — this fails as `ForbiddenUrl`, not at compile time).
- [ ] Ask a question, quit via the tray, relaunch: the History chip is there
      and the ask restores — `history.json` persists in app-data across
      restarts (vault/2026-08-03_answer-history.md).
- [ ] `THIRD-PARTY-LICENSES.txt` sits next to `WikiLens.exe` in the install
      folder, opens as plain text with the WikiLens header, and the
      uninstaller removes it (the `bundle.resources` entry —
      vault/2026-08-26_open-source-release-0-2-0.md).

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
