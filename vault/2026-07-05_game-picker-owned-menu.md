---
title: Game picker — owned menu, pull-back of exploration 4c
type: plan
status: done
created: 2026-07-05
updated: 2026-07-05
tags: [frontend, overlay]
related: ["[[2026-07-05_design-tokens-claude-design-sync]]", "[[2026-07-05_user-added-game-wikis]]", "[[2026-07-05_model-picker-menu]]"]
commit: [3ffc78d, 589561f]
---

# Game picker — owned menu, pull-back of exploration 4c

## In simple terms
The game dropdown opens as a flat white system list over the dark glass —
Windows draws that popup, not us, so no styling can ever fix it. The browser
exploration settled the replacement: the game becomes a quiet chip in the
header (mirroring the footer's model chip, so two quiet chips bracket the
panel), and clicking it opens our own menu — filter on top, the last three
games pinned under "Recent", little two-letter tiles as scan anchors, and
"Add a game…" pinned at the bottom. The "+" button leaves the header; adding
a game now starts inside the game menu.

## Context / problem
In-game screenshot review (2026-07-05) showed the native `<select>` popup as
an unreadable white sheet over the glass: WebView2 renders it outside the
page, so `--surface-menu` and the ink tokens can never reach it — a
structural defect, not a styling bug. Exploration 4 ("Game Picker
Exploration.html" in the Claude Design project) produced three owned-menu
variants; the recorded decision (doc header, 2026-07-05) is **4c ships** —
"game as identity, not a form field". The design side already updated the
mirror: `--menu-clearance-top` retuned 52 → 42px (chip-height header), the
menu height cap documented, and `previews/game-picker.html` distilled as the
component card. This plan is the code pull-back.

## Goal / non-goals
- **Goal — trigger:** replace `GamePicker` (native select) + the "+" header
  button with a single `GameChip`: `.quiet-chip` styling (game name +
  caret), `.is-open` state = `--surface-control` background + `--fg` ink,
  name truncates with ellipsis, disabled while an ask streams. Header =
  brand + chip, nothing else.
- **Goal — `GameMenu`** on the shipped menu pattern (capture-phase Esc,
  outside-pointerdown excluding the chip, direct child of `.panel`,
  top-anchored):
  - Filter input on top (autofocus, "Filter games…", Enter picks the first
    visible row); typing filters across sections, **headings drop and rows
    dedupe** while a query is active.
  - **Recent** section: up to 3 game ids from
    `localStorage["wikilens.recentGames"]` (validated against the current
    list), most recent first; updated on every selection.
  - **All games**: built-ins + user-added merged, alphabetical; selected row
    gets the accent ✓; the list **scrolls the selected row into view on
    open**.
  - **Monogram tiles** on every row: `--space-20` square, `--surface-raised`,
    `--radius-chip`, `--font-mono` 10px; muted ink, `--fg` on hover/selected.
    Derivation: initials of the first two words, else the first two letters,
    uppercased (the exploration's "PW"/"VH" camel-splits are mock aesthetics,
    not derivable — accepted).
  - **Pinned footer action** ("Add a game…", dashed `tile-add` "+" tile,
    accent text) above a hairline: switches straight to the add-game menu.
    Removal stays in the add-game flow's "Your games" — no ✕ in this menu.
- **Goal — CSS/tokens:** `--menu-clearance-top` 52 → 42px (comment: quiet
  chip header, ~22px line, no border); `.menu--top` cap becomes
  `calc(100% - var(--menu-clearance-top) - var(--menu-clearance))` so an
  open top menu never covers the footer chip (applies to the add-game menu
  too — conservative, shared); `.quiet-chip.is-open` + `.chip-name`
  ellipsis; `.tile` / `.tile-add`; `.menu-footer` + shared `.menu-action`;
  owned-scrollbar styling on `.menu-list` (thumb `--surface-raised`, 10px,
  transparent track — all menus get it); delete `.game-picker`,
  `.add-game-btn`, `.game-controls`.
- **Goal — wiring:** `openMenu` union gains `"game"`; the game menu's add
  action sets `openMenu = "addGame"` directly (menu swap, no refocus
  flicker; `AddGameMenu`'s trigger-exclusion ref becomes the game chip);
  `handleGameChange` also pushes onto the recents list. `closeMenu()`'s
  focus-return contract unchanged.
- **Goal — docs:** CLAUDE.md gains the native-select gotcha (OS-drawn popup,
  unstylable → every picker is an owned menu), updated component map
  (GameChip/GameMenu, "+" moved into the menu), updated clearance pairing
  note (42px, cap between the two clearances).
- **Non-goal:** variant 4a's in-menu remove rows and variant 4b's compact
  typeahead list + `--menu-width-compact` token — not chosen, not adopted.
- **Non-goal:** Rust changes — `list_games` already provides id/name/custom;
  recents and sorting are display concerns.
- **Non-goal:** real game art in tiles (monograms only, by design).

## Approach
1. **styles.css** — token retune + comment, `.menu--top` cap, new rules
   (`.quiet-chip.is-open`, `.chip-name`, `.tile`, `.tile-add`,
   `.menu-footer`, scrollbars), delete the three dead selectors.
2. **`GameChip.tsx`** — quiet chip mirroring `ModelChip` (name, caret, open/
   disabled, shared `buttonRef` for menu exclusion).
3. **`GameMenu.tsx`** — the menu per the spec above; props: games, selected
   id, recents, onSelect, onAddGame, onClose, chipRef.
4. **`App.tsx`** — `openMenu: "model" | "addGame" | "game" | null`; recents
   read/persist (`wikilens.recentGames`, store ~5, render 3); header swap;
   `AddGameMenu` triggerRef → game chip ref; delete `GamePicker.tsx`.
5. **Verify** — tsc + build; in-app over a real game: menu opens readable
   over a **bright** scene (the native popup's failure case), filter/Enter,
   recents update, add-a-game flow via the pinned action end-to-end, Esc
   layering (menu → overlay), focus returns to the prompt.
6. **Review** — ultracode adversarial pass (frontend focus/Esc/state lens +
   CSS-geometry lens), fix confirmed findings.
7. **Mirror follow-up** — `previews/panel.html` + `previews/controls.html`
   headers still show the select + "+": update both to the chip header after
   shipping; mark `previews/game-picker.html` SHIPPED with the commit. Vault
   close-out: this doc → done + commit; design-sync doc status log entry.

## Decisions & trade-offs
- **4c over 4a/4b** (browser decision, recorded in the exploration doc):
  calmer header over bright frames and recents-for-switchback, at the cost
  of a less discoverable borderless trigger (caret + hover are the cues) and
  add-a-game moving one hop deeper. Removal deliberately stays in the
  add-game flow.
- **Shared cap for both top menus** — the 4c height cap also constrains the
  add-game menu; consistent geometry beats a per-menu special case.
- **Recents are selection history, frontend-only** — localStorage like every
  other UI preference; invalid/removed ids filtered on read.
- **Deterministic monograms** — a tiny pure function, accepting that a few
  names monogram differently than the mock.

## Status log
- 2026-07-05 — created from the design-system pull: exploration 4 decision
  "4c ships" (see [[2026-07-05_design-tokens-claude-design-sync]]); mirror
  tokens already retuned browser-side (--menu-clearance-top 42px verified in
  the project sheet); queued as todo.
- 2026-07-05 — executed and landed in `3ffc78d`. GameChip + GameMenu on the
  shipped menu pattern; GamePicker deleted; openMenu is now a three-way
  union; recents persist in `wikilens.recentGames` (store 5, show 3).
  Adversarial review (17 agents, 2 lenses, 3-refuter panels) confirmed two
  real defects, both fixed before landing: (1) closing a menu by re-clicking
  its chip bypassed `closeMenu()` — focus stranded on the chip, typing went
  dead and Enter reopened the menu (fixed on the game chip AND the model
  chip, where the hole pre-existed); (2) the center-on-open scroll used
  `offsetTop` relative to `.menu` (the rows' offsetParent — `.menu-list` is
  unpositioned), overshooting by the ~40px search-bar height and hiding the
  selection above the fold on short panels (now converts into list
  coordinates). Two other findings refuted (chip is-open across the menu
  swap is correct semantics; the in-flight-add race pre-exists this change —
  candidate for a later hardening pass). tsc + vite build green. Mirror
  follow-up pushed: panel + controls cards now show the chip header, the
  game-picker card is marked SHIPPED. In-app pass (menu over a bright scene,
  Esc layering, recents, add-game hop) stays with the user.
- 2026-07-05 — in-app pass caught a real sizing flaw, fixed in `589561f`:
  the top-menu height cap resolved `100%` against the **content-hugging
  panel**, so on an idle panel the game menu strangled to one visible row
  (the design mock's fixed-height halves had hidden this). The cap is now
  viewport-based — `calc(100vh - --panel-gap - --menu-clearance-top -
  --shadow-room-bottom)` — legitimate because the window always holds the
  70% cap and `.panel` doesn't clip absolute children, so the menu extends
  into the free window space below a short panel. Applies to the add-game
  menu too (shared `.menu--top`). Known asymmetry left open: the
  bottom-anchored model menu stays panel-bound (no window room above the
  panel) — a candidate for the open design loop if it bothers in practice.
  Mirror comments (token sheet + game-picker card) updated to the shipped
  rule.
