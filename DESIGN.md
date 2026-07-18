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
  monogram:
    fontFamily: "ui-monospace, 'Cascadia Code', Consolas, monospace"
    fontSize: "10px"
rounded:
  cap: "3px"
  chip: "5px"
  control: "8px"
  input: "10px"
  panel: "12px"
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
  badge:
    backgroundColor: "{colors.veil-raised}"
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.chip}"
    padding: "1px 5px"
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
it refuses to do: no decoration, no entrance choreography, no color that isn't
information.

The system explicitly rejects Overwolf-style gamer overlay bloat (neon RGB,
widget clusters, aggressive branding) and generic AI chatbot styling (chat
bubbles, bot avatars, sparkle-emoji "AI" decoration, typing indicators). The
game is the main character; WikiLens borrows the screen and gives it back.

**Key Characteristics:**

- One smoked-glass sheet over the game; content-hugging, right-docked, gone
  when dismissed.
- One accent hue (Moonlight Blue) that only ever marks the interactive, the
  selected, and the focused; a rose trio that only ever marks failure.
- A 14px type ceiling: hierarchy by weight and ink strength, never by size.
- Controls are quiet until touched — bare ink at rest, a faint veil on hover.
- Two altitudes exactly: the panel above the game, menus above the panel.
- State changes are instant; the only animation in the product is a
  reduced-motion-gated progress shimmer in the debug window.

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

**The One Signal Rule.** Moonlight Blue is the interface's only voice of
color. If an element is not interactive, selected, or focused, it stays ink.
The rose trio speaks only on failure. There is no third hue, and there are no
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

The ceiling has a floor: below the ramp sit two glyph sizes — the 9px caret
and the 10px monogram lettering — typography used as iconography, never as
copy. The smallest reading text remains the 12px Label.

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

## 5. Components

Quiet until touched: every control rests as bare ink and only surfaces a
faint veil fill on hover, focus, or open. The panel at idle reads as text on
glass, not as a form.

### Buttons (Quiet Chips)

- **Shape:** softly rounded (5px), transparent at rest, no border ever.
- **Rest:** muted ink text (#9a9aa8), padding 2px 4px.
- **Hover / Open:** veil fill (rgba(255,255,255,0.06)) and ink promotion to
  #e8e8ee; the open state mirrors hover because an owned popup has no native
  focus ring.
- **Accent action rows** ("Add a game…", "Find its wiki"): same anatomy at
  menu-row size, ink swapped for Moonlight Blue.
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
- **Rows:** 13px ink text, 8px radius, veil on hover; selection is a
  Moonlight Blue check (and monogram tiles promote to ink on hover/selected).
- **Group headers:** label-style uppercase, collapsible with a rotating
  9px caret.
- **Placement:** menus are direct children of the panel, anchored by paired
  clearance constants; geometry is measured per open, not re-measured while
  open.

### Signature: the Monogram Tile

A 20×20 rounded tile bearing a 10px mono monogram ("SV", "CE") — the scan
anchor for a growing game list. Raised-veil fill, muted ink promoting to full
ink on hover/selection. The system's only "icon", and it's typography.

## 6. Do's and Don'ts

### Do:

- **Do** keep `html, body` transparent in every window's stylesheet — the
  webview paints a black rectangle without it.
- **Do** draw controls as bare ink at rest and reveal the veil fill only on
  hover/focus/open ("quiet until touched").
- **Do** reserve Moonlight Blue for the interactive, the selected, and the
  focused (the One Signal Rule) — answers and data stay ink.
- **Do** build every picker as an owned menu on Menu Glass, and keep menus
  direct children of `.panel` so `.content`'s overflow can't clip them.
- **Do** keep state changes instant, and gate any animation behind
  `prefers-reduced-motion` (the debug shimmer is the template).
- **Do** retune paired constants together — `styles.css` margins ↔
  `window.rs` floats, `--menu-clearance*` ↔ `menuPlacement.ts` — whenever
  geometry tokens change.

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
- **Don't** introduce a new hue or gradient; the palette is Moonlight Blue,
  the inks, and the rose error trio.
- **Don't** scale type past 14px for emphasis — promote weight or ink instead
  (the Fourteen-Pixel Ceiling).
- **Don't** add entrance choreography or animate the answer stream; the
  player is mid-game and the audit test is: *if you notice the panel while
  not asking it something, it's too loud.*
