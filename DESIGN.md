---
name: WikiLens
description: A glass field guide floating over the game — wiki answers in seconds, never instead of the view.
colors:
  moonlight-blue: "#8ab4ff"
  moon-ink: "#e8e8ee"
  ink-muted: "#9a9aa8"
  ink-strong: "#ffffff"
  smoked-glass: "#121218eb"
  menu-glass: "#181820f7"
  veil: "#ffffff0f"
  veil-raised: "#ffffff14"
  ink-well: "#0000004d"
  hairline: "#ffffff14"
  rose-signal: "#ffb3bd"
  rose-wash: "#781e2859"
  rose-line: "#ff788c40"
  micro-cream-ink: "#eae4d4"
  micro-ink-muted: "#a49d8b"
  micro-ink-strong: "#f7f2e3"
  micro-parchment: "#d9c69a"
  micro-carbon-glass: "#13120ff0"
  micro-menu-glass: "#171613fa"
  micro-veil: "#eae4d412"
  micro-veil-raised: "#eae4d41a"
  micro-ink-well: "#00000052"
  micro-hairline: "#eae4d429"
  micro-line: "#eae4d459"
  micro-salmon-signal: "#ffb4a4"
  micro-salmon-wash: "#7a2c1c59"
  micro-salmon-line: "#ff8a7040"
typography:
  title:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    letterSpacing: "0.02em"
  body:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  status:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.4
  label:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    letterSpacing: "0.06em"
  code:
    fontFamily: "ui-monospace, 'Cascadia Code', Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
  caret:
    fontSize: "9px"
  stepper-arrow:
    fontSize: "9px"
  monogram:
    fontFamily: "ui-monospace, 'Cascadia Code', Consolas, monospace"
    fontSize: "10px"
  micro-ui:
    fontFamily: "'Cascadia Code', ui-monospace, Consolas, monospace"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  micro-title:
    fontFamily: "'Cascadia Code', ui-monospace, Consolas, monospace"
    fontSize: "14px"
    fontWeight: 400
    letterSpacing: "0.14em"
  micro-label:
    fontSize: "12px"
    fontWeight: 400
    letterSpacing: "0.1em"
  micro-section-head:
    fontSize: "10px"
    letterSpacing: "0.14em"
  micro-arrow:
    fontSize: "11px"
rounded:
  cap: "3px"
  chip: "5px"
  control: "8px"
  input: "10px"
  panel: "12px"
  micro-sharp: "0px"
spacing:
  space-2: "2px"
  space-4: "4px"
  space-6: "6px"
  space-8: "8px"
  space-10: "10px"
  space-12: "12px"
  space-14: "14px"
  space-20: "20px"
components:
  panel:
    backgroundColor: "{colors.smoked-glass}"
    textColor: "{colors.moon-ink}"
    rounded: "{rounded.panel}"
    padding: "14px"
  quiet-chip:
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
    padding: "2px 4px"
  icon-chip:
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
    padding: "4px"
  static-chip:
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
    padding: "2px 4px"
  quiet-chip-hover:
    backgroundColor: "{colors.veil}"
    textColor: "{colors.moon-ink}"
  prompt-input:
    backgroundColor: "{colors.veil}"
    textColor: "{colors.moon-ink}"
    rounded: "{rounded.input}"
    padding: "10px 12px"
  menu-card:
    backgroundColor: "{colors.menu-glass}"
    rounded: "{rounded.input}"
  menu-row:
    textColor: "{colors.moon-ink}"
    rounded: "{rounded.control}"
    padding: "5px 8px"
  menu-row-hover:
    backgroundColor: "{colors.veil}"
  history-row-meta:
    textColor: "{colors.ink-muted}"
  badge:
    backgroundColor: "{colors.veil-raised}"
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
    padding: "1px 5px"
  key-cap:
    backgroundColor: "{colors.veil-raised}"
    rounded: "{rounded.chip}"
    padding: "1px 5px"
  hotkey-row:
    textColor: "{colors.moon-ink}"
    rounded: "{rounded.control}"
    padding: "5px 8px"
  hotkey-row-armed:
    borderColor: "{colors.moonlight-blue}"
  stepper-value:
    textColor: "{colors.moon-ink}"
    rounded: "{rounded.control}"
    padding: "8px"
  stepper-arrow-seat:
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
  stepper-popover:
    backgroundColor: "{colors.menu-glass}"
    rounded: "{rounded.input}"
  game-suggestion:
    textColor: "{colors.moonlight-blue}"
    rounded: "{rounded.chip}"
    padding: "2px 4px"
---

# Design System: WikiLens

## 1. Overview

**Creative North Star: "The Field Guide"**

WikiLens is a field guide flipped open mid-hike: you summon it over the running
game, read one terse, sourced entry, and pocket it again in seconds. The whole
visual system serves that gesture. The surface is a single sheet of smoked
glass (near-opaque so text survives any game backdrop) docked at the screen's
right edge, hugging its content; everything on it is ink first and chrome
never. The interface's personality — quiet, precise, instant — comes from what
it refuses to do: no decoration, no motion without a question to answer, no
color that isn't information.

The system explicitly rejects Overwolf-style gamer overlay bloat (neon RGB,
widget clusters, aggressive branding) and generic AI chatbot styling (chat
bubbles, bot avatars, sparkle-emoji "AI" decoration, typing indicators). The
game is the main character; WikiLens borrows the screen and gives it back.

**Key Characteristics:**

- One smoked-glass sheet over the game; content-hugging, right-docked, gone
  when dismissed.
- One accent hue per theme (Moonlight Blue in the default, Parchment in
  Micrographics — see Themes) that only ever marks the interactive, the
  selected, and the focused; one failure trio that only ever marks failure.
- A 14px type ceiling: hierarchy by weight and ink strength, never by size.
- Controls are quiet until touched — bare ink at rest, a faint veil on hover.
- Two altitudes exactly: the panel above the game, menus above the panel.
- State changes are instant; motion is governed by the Named-Question Rule
  and speaks the Quiet Directional voice (see Motion) — six admitted
  motions: five on the overlay and the debug window's shimmer.

## 2. Colors

A monochrome glass world where a single cool blue does all the talking.

### Primary

- **Moonlight Blue** (#8ab4ff): cool light in a dark room. It marks
  interactivity and selection only — focused input borders, links and source
  titles, the selection check in menus, the accent action row ("Add a
  game…"), the live-ask border in the debug window, the capture marquee. It
  never fills a surface and never decorates.

### Neutral

- **Moon Ink** (#e8e8ee): the default text color — answers, rows, filled
  chips on hover.
- **Muted Ink** (#9a9aa8): resting chips, statuses, hints, labels, metadata.
  The panel at idle is mostly this color.
- **Strong Ink** (#ffffff): reserved for `strong` emphasis inside answers and
  the asked question in debug cards.
- **Smoked Glass** (rgba(18, 18, 24, 0.92) = #121218eb): the panel surface —
  dark enough to guarantee readable text over any game frame, translucent
  enough (with its 12px blur) to admit the game's light.
- **Menu Glass** (rgba(24, 24, 32, 0.97) = #181820f7): near-opaque menu
  surface; menus float over panel text and cannot share its translucency.
- **Veil** (rgba(255, 255, 255, 0.06) = #ffffff0f): the hover/control fill —
  the faint white breath a control takes when touched.
- **Raised Veil** (rgba(255, 255, 255, 0.08) = #ffffff14): monogram tiles,
  badges, `kbd`, inline code, scrollbar thumbs.
- **Ink Well** (rgba(0, 0, 0, 0.3) = #0000004d): inset recesses — code
  blocks, attachment thumbnails, progress-bar troughs.
- **Hairline** (rgba(255, 255, 255, 0.08) = #ffffff14): every border and
  divider in the system, one pixel, always.

### Error

- **Rose Signal** (#ffb3bd) on **Rose Wash** (rgba(120, 30, 40, 0.35)) with
  **Rose Line** (rgba(255, 120, 140, 0.25)): the failure voice — error boxes,
  the blocked-attachment hint, destructive hover on remove buttons, aborted
  asks. Never used for warnings, emphasis, or decoration.

### Named Rules

**The One Signal Rule.** Each theme has exactly one accent voice. In the
default theme it is Moonlight Blue; in Micrographics it is Parchment
(#d9c69a). If an element is not interactive, selected, or focused, it stays
ink. The failure trio (rose in the default, salmon in Micrographics) speaks
only on failure. There is no third hue in any theme, and there are no
gradients.

## 3. Typography

**Body Font:** system-ui stack (`system-ui, -apple-system, "Segoe UI",
Roboto, sans-serif`)
**Data/Mono Font:** `ui-monospace, "Cascadia Code", Consolas, monospace`

**Character:** Native and invisible. The system stack renders instantly,
matches the player's OS, and carries zero brand flourish — the field guide is
typeset like the machine it runs on. Mono is the instrument voice, reserved
for identifiers and measurements (model ids, timings, char counts, monogram
tiles).

### Hierarchy

- **Title** (600, 14px, 0.02em tracking): the brand wordmark and the strongest
  text on the glass. Same size as body — weight is the promotion.
- **Body** (400, 14px, 1.5 — answers relax to 1.55): questions and streamed
  answers. Compact enough for an in-game glance.
- **Status** (400, 13px): chip labels, phase statuses, menu rows, hints — the
  working size of most controls.
- **Label** (400, 12px, 0.06em tracking, UPPERCASE): section headings inside
  menus ("Recent", provider groups, "Sources"), badges, metadata.
- **Code** (400, 12.5px mono): inline code, model ids, timings.

### Named Rules

**The Fourteen-Pixel Ceiling.** 14px is the largest type in the product.
Nothing displays, headlines, or shouts; hierarchy is carried by weight, ink
strength (muted → ink → strong), tracking, and case. If a design wants a
bigger font, it wants attention the game should keep.

The ceiling has a floor: below the ramp sit two glyph sizes — 9px for the
caret and the settings steppers' ◁ ▷ arrows (direction hints, one size in
both themes), and the 10px monogram lettering — typography used as
iconography, never as copy. The smallest reading text remains the 12px
Label. Under Micrographics two more sizes join the below-ramp band: the 10px
indexed section heads (decorative micro-copy, never message text) and the
11px source arrow (typography as iconography, like the caret).

## 4. Elevation

Elevation is structural, never atmospheric. Shadows exist to separate glass
from game — they mark what floats above the world, and nothing else. Depth
within the panel is conveyed by surface tone (veil fills, ink-well recesses),
not by shadow.

### Shadow Vocabulary

- **Panel float** (`box-shadow: 0 12px 32px rgba(0, 0, 0, 0.32)`): lifts the
  glass sheet off the game frame. Paired with the 12px backdrop blur and the
  1px hairline border — the three together are the glass material.
- **Menu float** (`box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45)`): the second
  altitude; menus cast deeper because they stand on the panel.

### Named Rules

**The Two Altitudes Rule.** Exactly two things float: the panel above the
game, and menus above the panel. There is no third level — no raised cards,
no hovering toasts, no shadowed buttons. If a new element wants a shadow, it
is claiming to be a window, and it almost certainly isn't one.

One scoped exception, admitted by decision doc
`vault/2026-08-13_stepper-popover-third-altitude.md`: the settings stepper's
option popover floats above the settings card, reusing Menu Glass and the
menu float shadow — no third shadow exists. Nothing else may cite it as
precedent.

## 5. Components

Quiet until touched: every control rests as bare ink and only surfaces a
faint veil fill on hover, open, or the keyboard highlight. The panel at idle
reads as text on glass, not as a form.

**Keyboard focus** (`:focus-visible`) speaks with the accent instead: a 1px
Moonlight Blue outline, offset 2px on standalone controls (chips, links,
inline actions) and inset 1px on in-list rows so touching rows don't overlap.
Pointer presses never ring. Inputs keep their border-shift (see Inputs) — a
field with a border needs no second voice.

### Buttons (Quiet Chips)

- **Shape:** softly rounded (5px), transparent at rest, no border ever.
- **Rest:** muted ink text (#9a9aa8), padding 2px 4px.
- **Hover / Open:** veil fill (rgba(255,255,255,0.06)) and ink promotion to
  #e8e8ee; the open state mirrors hover because an owned popup has no native
  focus ring.
- **Keyboard focus:** the authored Moonlight Blue outline (see "Keyboard
  focus" above), replacing the UA default ring.
- **Accent action rows** ("Add a game…", "Find its wiki"): same anatomy at
  menu-row size, ink swapped for Moonlight Blue.
- **Inline text action** ("Stop", beside the phase label in the status row):
  the quiet-chip anatomy at 12px label size, resting as muted ink inside the
  status line — an escape hatch, not a call to action.
- **Disabled:** 0.6 opacity, cursor reverts.

### Chips (Badges)

- **Style:** raised-veil fill, hairline border, 5px radius, 12px text, padding
  1px 5px; muted ink by default.
- **Accent variant:** Moonlight Blue text for capability marks (e.g. "Image"
  on vision-capable model rows).

### Cards / Containers (the Glass Panel)

- **Corner Style:** 12px radius (the largest in the system).
- **Background:** Smoked Glass + 12px backdrop blur; Menu Glass for floating
  menus; veil fill + hairline + 12px radius for debug ask-cards.
- **Shadow Strategy:** panel float / menu float only (see Elevation).
- **Border:** 1px hairline, always.
- **Internal Padding:** 14px (panel), 10–12px (boxes).
- **Micro radius:** the debug window's 6px-tall progress bar rounds with a
  3px cap — the pill end-cap, the smallest radius in the system (below the
  5px chip).

### Inputs / Fields

- **Style:** veil fill, hairline border, 10px radius (prompt) / 8px (menu
  search); ink text, muted-ink placeholder.
- **Focus:** border shifts to Moonlight Blue; no outline, no glow.
- **Disabled:** 0.6 opacity. Errors live in adjacent rose boxes, not on the
  field.

### Navigation (Owned Menus)

- **Style:** every picker is an owned menu card on Menu Glass — filter input
  at top, scrolling list, optional pinned footer action; never a native
  `<select>` (OS-drawn popups render as unstylable white sheets over the
  glass).
- **Rows:** 13px ink text, 8px radius, veil on hover, on the arrow-key
  highlight (the keyboard's hover — same voice, independent states), *and*
  while the row's key form is open (the third voice of the same state: a row
  that opened something stays lit until it closes); selection is a Moonlight
  Blue check (and monogram tiles promote to ink on hover/selected).
- **Rows, the one exception:** a Settings provider key line rests at *muted*
  ink until a key is stored; then it promotes to full ink and wears a neutral
  **Set** badge on its right rail, before the trash (the badge position every
  other row uses). The badge is the mark — ink strength alone
  proved illegible without a neighbor to compare against — and it stays
  neutral because storage is not selection: the accent check belongs to the
  Answers options (the One Signal Rule). A keyed line is not a control at
  all; its
  only action is the trash, and the unkeyed line's `aria-label` plus the
  badge text carry the state for assistive tech.
- **Keyboard:** focus stays on the filter input; Up/Down move the highlight
  through the pickable rows only (no wrap — Home/End jump), Enter picks the
  highlighted row, and typing keeps filtering. Group headers and pinned
  footer actions stay outside the arrow order (Tab reaches them, ringed by
  the authored focus outline).
- **Group headers:** label-style uppercase, collapsible with a rotating
  9px caret.
- **Placement:** menus are direct children of the panel, anchored by paired
  clearance constants; geometry is measured per open, not re-measured while
  open.
- **Key recorder (Shortcuts popover):** combos render as key-cap chips
  (raised veil, chip radius, label size); the armed row speaks with a
  Moonlight Blue border only — recording is an interactive state, so the One
  Signal Rule covers it — and refusals use the rose failure voice.

### Steppers (Settings enum rows)

- **Anatomy:** `◁  Value  ▷` — the game-settings voice for a small enum
  (Answers, Theme). The arrows cycle the value and **wrap** at the ends (a
  stepper has no scroll viewport, so the menus' no-wrap rationale doesn't
  apply — `cycle` in `stepper.ts`); the centered value opens a floating
  option popover. With one option (an install with no built-in) the arrows
  disable and the value still opens its one-row popover.
- **Rail:** the settings control rows — both steppers, the shortcut recorder
  rows, and the Custom API key lines — sit on one **90% centered rail**
  inside the card, so the controls read as a column set into the plate;
  headings, notes, and error boxes keep the card's full measure.
- **Frame:** the stepper row wears a **1px hairline frame** at control radius
  (`--border` — the framed-control voice, the input frame's family, not a
  button border). Deliberately steppers only: the shortcut rows stay bare, so
  the accent border remains the armed recorder's one voice, and the key
  lines stay storage-bare. The frame takes an **extra 8px below its section
  heading** — a bare row's padding is invisible, a frame is not, so the gap
  matches the optical heading-to-row rhythm the Shortcuts section sets.
- **Arrows:** the fixed 20×20 square seat (the trash/caret recipe) around a
  9px glyph; bare muted ink at rest, veil + ink promotion on hover, 0.6
  opacity disabled. No transition — a cycled value is an instant state
  change.
- **Value:** 13px ink centered on the row (the note — "Needs a key",
  "Not set up here" — keeps the right rail), settings-row padding, 8px
  radius; hover and open wear the veil (open mirrors hover).
- **Popover:** Menu Glass, input radius, menu float shadow — the scoped Two
  Altitudes exception (§4). Options are standard menu rows: accent check on
  the current one, per-option notes on the right rail. Placement is measured
  per open from the row (below it, flipping above near the card's bottom
  edge); a list scroll or window resize closes it rather than re-measuring.
  Esc layers popover → menu → overlay.
- **Keyboard:** ArrowLeft/Right on the focused value cycle without opening;
  in the popover, focus lands on the current option and Up/Down walk the
  rows, clamped — only the horizontal cycle wraps.

### Signature: the Monogram Tile

A 20×20 rounded tile bearing a 10px mono monogram ("SV", "CE") — the scan
anchor for a growing game list. Raised-veil fill, muted ink promoting to full
ink on hover/selection. The system's only *pictorial* mark that is typography —
the three vector glyphs beside it, the header gear, the header's Start over
whirl, and the key line's trash, are 12px Bootstrap Icons, admitted where no
typographic mark reads as the action.
The gear sits in a square 20×20 seat (`icon-chip`, equal 4px padding): a
circular glyph in the quiet-chip's landscape padding read off-center whenever
the veil lit, and the square echoes the monogram tile's footprint.

## 6. Motion

The default is stillness: state changes — hover, focus, open — apply
instantly, and the glass never moves to be noticed. New motion enters one
animation at a time through the Named-Question Rule below, proven over real
game backdrops before it ships; the admitted inventory lives here and is
updated in the same PR as any admission.

### Admitted inventory

- **A-01 · panel-summon** — the panel's 12px slide-in from the right with a
  fade on summon (120ms ease-out; a pre-summon hold keeps the panel in the
  keyframe's "from" state while hidden, and `overlay://shown` arms the
  entrance frame-synced — double rAF, timer backstop — so the resuming
  webview can't burn the 120ms clock before it presents a frame; dropped on
  `animationend` and on hide — hiding stays instant). Rust force-disables
  DWM's own window transitions (`DWMWA_TRANSITIONS_FORCEDISABLED`,
  `window.rs`) so the OS's one-time first-show fade + rise can't layer over
  the entrance — without it the first summon after launch moves bottom-up.
  Its question: "the panel came from the right edge."
- **A-02 · hover-release** — quiet chips light instantly on hover and open
  (the on-path is always a hard cut), and release with a 120ms fade of veil
  and ink — on pointer-leave and on menu-close alike. Its question: "the
  control heard you." The inventory's one named deviation: the release
  fades background and ink (a chip-sized repaint, not transform/opacity),
  accepted as-is in the live test.
- **A-03 · attach-confirm** — the capture attachment block mounts with a
  4px rise and fade (150ms ease-out); removal unmounts — instant. Its
  question: "your grab landed."
- **A-04 · menu-arrive** — menus mount with a 4px arrival from their anchor
  chip, direction-aware (120ms ease-out); closing unmounts — instant. Its
  question: "this came from that chip."
- **A-05 · suggestion-arrive** — the game-detection offer mounts with a 4px
  arrival from the right, the direction of the game chip it would fill
  (120ms ease-out); accepting or ignoring unmounts it — instant. Its
  question: "the panel noticed the game." Deliberately the same amplitude as
  A-04: this is an offer the player did not ask for, so it must be findable
  and ignorable in one glance, never insistent.
- **A-06 · stepper-popover-arrive** — the settings stepper's option popover
  mounts with a 4px arrival from its anchor row, direction-aware — dropping
  when placed below the row, rising when flipped above (120ms ease-out,
  reusing A-04's keyframes: same voice, same travel); closing unmounts —
  instant. Its question: "this came from that row."
- **debug-shimmer** — the debug window's indeterminate progress shimmer (a
  1.2s translateX loop), gated behind `prefers-reduced-motion` with a static
  accent-strip fallback. Its question: "is this ask still running?"
  Predates the rule (carried over from the instant-states doctrine); the
  rule's bounds and paperwork apply to motions admitted through it.

### The Motion Voice: Quiet Directional

The rule decides whether a motion exists; the voice governs how it moves.
Every admitted motion carries one bit of spatial information — where the
thing came from — at whisper amplitude: a fade plus a small directional
travel (4px for elements inside the panel, ~12px for the panel itself),
plain ease-out, 120–150ms. Both extremes fail the same way: a travel-less
fade answers "something appeared" but not "from where", and a pronounced
slide, a scale, or a lingering tail answers louder than the player asked.
The live test that set this voice (2026-07-25, over real game backdrops)
picked the quietest variant that still answered its question — every time.

### Named Rules

**The Named-Question Rule.** Motion may exist only to answer a question the
player already has — "did my keypress register?", "where did this come
from?", "did that attach?" — and only when it answers faster than a hard cut
would. Every animation names its question, in a stylesheet comment and in its
PR; a motion whose question can't be named is decoration, and this register
rejects decoration. Motions that pass are still bound: transform/opacity only
(the glass floats over a GPU-saturated game — compositor work, never
repaint); ≤200ms; ease-out on entry; exits are instant — leaving is the
product; input readiness is never delayed (animation is paint, not gate); and
everything sits behind prefers-reduced-motion with a static fallback. The
answer stream and focus indicators never animate. Candidates enter through a
live test over real game backdrops; admission updates this inventory in the
same PR.

## 7. Do's and Don'ts

### Do:

- **Do** keep `html, body` transparent in every window's stylesheet — the
  webview paints a black rectangle without it.
- **Do** draw controls as bare ink at rest and reveal the veil fill only on
  hover/open and the keyboard highlight ("quiet until touched"); keyboard
  focus speaks with the Moonlight Blue outline, never a fill.
- **Do** reserve Moonlight Blue for the interactive, the selected, and the
  focused (the One Signal Rule) — answers and data stay ink.
- **Do** build every picker as an owned menu on Menu Glass, and keep menus
  direct children of `.panel` so `.content`'s overflow can't clip them.
- **Do** keep state changes instant unless a motion has passed the
  Named-Question Rule, and gate every admitted animation behind
  `prefers-reduced-motion` with a static fallback (the debug shimmer is the
  template).
- **Do** retune paired constants together — `styles.css` margins ↔
  `window.rs` floats, `--menu-clearance*` ↔ `menuPlacement.ts`, and the
  stepper popover's 4px gap / 6px inset ↔ `POPOVER_GAP` / `POPOVER_INSET` in
  `Stepper.tsx` (jsdom can't read custom properties, so placement math
  duplicates them in JS) — whenever geometry tokens change.
- **Do** express a new theme as token overrides only: ink, surfaces, borders,
  shape, shadows, and type voice. Rhythm, pads, text sizes, leadings, blur,
  and every JS-twinned geometry token (`--panel-gap`, `--shadow-room-*`,
  `--menu-clearance*`) are theme-invariant — a theme block never redeclares
  them (pinned by `src/styles.guardrails.test.ts`).

### Don't:

- **Don't** ship Overwolf-style gamer overlay bloat: neon RGB, widget
  clusters, ads, or aggressive branding competing with the game for
  attention.
- **Don't** ship generic AI chatbot styling: chat bubbles, bot avatars,
  sparkle-emoji "AI" decoration, or typing indicators.
- **Don't** reintroduce a native `<select>` (or any OS-drawn popup) for
  anything shown over the game.
- **Don't** add a third elevation — no shadowed buttons, raised cards, or
  toasts (the Two Altitudes Rule).
- **Don't** introduce a new hue or gradient; each theme's palette is its one
  accent, its inks, and its failure trio — nothing else, in any theme.
- **Don't** scale type past 14px for emphasis — promote weight or ink instead
  (the Fourteen-Pixel Ceiling).
- **Don't** ship a motion that can't name its question (the Named-Question
  Rule) — and never animate the answer stream or focus indicators; the
  player is mid-game and the audit test is: *if you notice the panel while
  not asking it something, it's too loud.*

## 8. Themes

The appearance is a closed set of two themes, picked in **Settings → Theme**
(the stepper anatomy, §5: arrows cycle the theme live, the value's popover
lists name-only options with the accent check on the active one, no swatch —
the panel itself is the preview). The pick applies live and
persists locally (`wikilens.theme` in localStorage). Mechanically a theme is
a wholesale custom-property swap: the default theme **is** the bare `:root`
block (no attribute), and a non-default theme is an overrides-only
`:root[data-theme="<id>"]` block set on the root element by the app. The
debug window and the capture reticle stay on the default appearance by
design.

**Theme-invariant, in every theme:** rhythm and pads, text sizes and
leadings, `--blur-panel`, the float/menu geometry twins (`--panel-gap`,
`--shadow-room-*`, `--menu-clearance*` — paired with `window.rs` /
`menuPlacement.ts`), `--font-mono`, `--opacity-disabled`, and the admitted
motion inventory. Themes are chrome: ink, surfaces, borders, shape, shadow
values, and the type *voice* (face, weight, tracking) — never type metrics.

### Micrographics

The second theme, in the micrographic-modern register: a spec plate over the
game rather than a field guide page. Two layers:

- **Token swap** — warm carbon glass (`micro-carbon-glass`) under cream ink
  (`micro-cream-ink` / `micro-ink-muted` / `micro-ink-strong`), the
  **Parchment** accent (`micro-parchment` — same monochrome family as the
  ink; chroma alone separates it), cream-tinted veils and a one-step-stronger
  hairline (`micro-hairline` — thin lines are this style's structure), the
  failure trio warmed to salmon (`micro-salmon-*`), **every radius 0**
  (`micro-sharp` — drafting-sharp corners, the theme's loudest single move),
  tighter flatter shadows, and Cascadia Code promoted from instrument voice
  to UI face (`micro-ui`; the brand drops to regular weight and spreads to
  0.14em — `micro-title`).
- **Instrument layer** — component-voice marks active only under the theme:
  the caps brand, uppercase underscore chip labels (`[ Stardew_Valley ]`,
  `ANTHROPIC · CLAUDE_SONNET_5` — JS swaps characters, CSS owns case;
  accessible names stay sentence case), the underscore prompt placeholder
  (`ASK_ABOUT_THE_GAME…`, same character/case split), indexed section heads
  (`/01 PROMPT`, `/02 ANSWER`, `/03 SOURCES` — `micro-section-head` type,
  hairline rules in `micro-line`, aria-hidden), the uppercase status row —
  phase label (case is paint; the text stays the accessible status),
  dot-matrix progress (aria-hidden), and the uppercase `STOP` pushed to the
  row's right edge — uppercase source links with the `micro-arrow` ↗, and
  the crosshair capture glyph. Decoration budget ends there — the layer is
  flat marks inside existing line boxes and measured content flow, so
  rendered header/footer heights never move and the clearance twins hold.
  (A header ruler tick strip and a footer barcode shipped in the first pass
  and were removed on visual review — the spec plate reads better without
  free-floating decoration.)
