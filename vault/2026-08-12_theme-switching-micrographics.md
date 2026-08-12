---
title: Theme switching — settings picker and a micrographics variant
type: plan
status: done
created: 2026-08-12
updated: 2026-08-12
tags: [frontend, overlay]
related:
  - "[[2026-08-04_overlay-design-followups]]"
  - "[[2026-07-05_design-tokens-claude-design-sync]]"
  - "[[2026-07-18_impeccable-design-context]]"
  - "[[2026-07-30_design-mirror-true-up]]"
  - "[[2026-08-12_chrome-only-themes]]"
  - "[[2026-08-12_theme-persistence-localstorage]]"
commit: ["33d9d72", "078bd20"]
---

# Theme switching — settings picker and a micrographics variant

## Context / problem

User request: an interface for switching themes — the current appearance
stays the default, plus a second theme "inspired by micrographics graphic
design" (the archival microfilm/microfiche register: dense mono type,
hairline rules, index and registration marks, high information density).
Today the product has exactly one appearance: `src/styles.css`'s `:root`
tokens carry it, DESIGN.md's frontmatter is its normative source, and
nothing in `src/` knows what a theme is.
[[2026-08-04_overlay-design-followups]] explicitly declared "a general
theming system or user-facing theme picker" a non-goal; this plan
supersedes that non-goal — and the bright-scene variant parked there
gains a natural home once a theme dimension exists.

## Goal / non-goals

- Goal: a theme picker in the settings menu; the current appearance is the
  default theme, visually unchanged.
- Goal: one new theme in the micrographics register — the direction is
  named here; the visual spec itself is design-pass work under DESIGN.md
  governance.
- Goal: the pick persists across restarts.
- Non-goal: automatic switching (scene-sampling stays parked in
  [[2026-08-04_overlay-design-followups]]).
- Non-goal: user-defined or per-element theming — a closed set of two.
- Non-goal: theming `capture.html` (inline styles, no tokens; a reticle,
  not chrome).
- Non-goal: the micrographics palette/spec in this doc — direction and
  constraints only.

## Approach

1. **Token audit** — classify the `:root` block (`src/styles.css` L26–115)
   into themeable vs theme-invariant. Invariant for certain: Float
   geometry (`--panel-gap`, `--shadow-room-*`, paired with `window.rs`
   constants) and Menu geometry (`--menu-clearance*`, JS twin in
   `menuPlacement.ts`). Clearly themeable: Ink, Glass surfaces, Status
   colors, shadow *colors*. The contested middle — Type and Shape — is the
   twins fork below. Output: an explicit invariant list that step 7 pins.
2. **DESIGN.md first** — extend the normative frontmatter with a theme
   dimension, and amend the named rules to be per-theme ("the One Signal
   Rule: each theme has exactly one accent voice") — that amendment is its
   own step, not a schema footnote. Regenerate `.impeccable/design.json`
   via the impeccable `document` workflow (Step 4b — the sidecar is
   agent-authored; DesignSync is the separate claude.ai/design mirror
   ritual and never writes repo files). The detector parses **flat**
   frontmatter maps only, so the theme dimension must be expressed as
   theme-prefixed flat keys, never nested `themes:` tables.
3. **Application mechanism** — apply the non-default token set to the
   overlay page (mechanism chosen in the code phase, not here). Hard
   constraint either way: `html, body { background: transparent }` is
   required by the transparent window — themes paint surfaces on `.panel`
   and menus, never the body.
4. **Settings UI** — a third `menu-heading` section in `SettingsMenu.tsx`
   with modeRow-shaped, mutually exclusive rows and the accent check on
   the active theme, following the "Answers" section's shape.
5. **Persistence** — a fork (below). Lean: localStorage under a
   `wikilens.*` key with a corruption-tolerant reader and lazy useState
   initializer, matching the game/provider/model precedent — a theme is a
   UI pick, not behavior config.
6. **Micrographics design pass** — under the standing constraints:
   PRODUCT.md's register (quiet, precise, the game is the main character;
   the anti-reference is neon overlay bloat), the 14px ceiling, one accent
   voice. Micrographics here means restraint borrowed from archival film —
   hairlines, mono, density — not ornament; no decorative frame marks
   adding noise over gameplay.
7. **Guardrail tests** — pin the theme-invariant token set (geometry
   values byte-identical across themes) the way `config_guardrails.rs`
   pins CSP/capabilities; a corruption-tolerant persistence test per
   `App.storage.test.tsx` if localStorage wins the fork.

Open questions — the real forks; verdicts split into decision docs when
taken:

- **Persistence home.** localStorage (UI-pick precedent) vs
  `settings.json` (SettingsStore; the `get_settings` envelope is
  doc-commented as the future config-panel home, but it costs a
  settings.rs field + command + api.ts + types.ts). Lean localStorage —
  it is also same-origin-shared across the three pages for free if scope
  ever grows.
- **Page scope.** Overlay-first vs unifying tokens across the three pages
  first. `debug/styles.css` is a hand-copied subset whose own header
  defers a shared tokens.css until "a fourth page ever appears" — that
  trigger has not fired. Lean overlay-first; debug keeps the default
  appearance until a theme must reach it.
- **The twins problem (sharpest fork).** `--menu-clearance` /
  `--menu-clearance-top` are derived from *rendered* footer/header
  heights; `--shadow-room-*` must contain the shadow geometry — all
  paired with `window.rs` / `menuPlacement.ts` constants. A theme that
  changes type metrics (mono body, denser leading) changes rendered
  heights and desyncs the twins. Fork: v1 themes are chrome-only (ink,
  surfaces, borders, shadow color — never type metrics, never
  shadow/menu geometry) vs making the twins theme-aware (heavier,
  per-theme constants on both sides). Lean chrome-only — but mono type is
  central to the micrographics register, so the design pass may push
  back; if it does, that is the decision doc.
- **DESIGN.md schema shape.** Per-theme color tables vs theme-prefixed
  flat names, and whether component specs keep `{colors.x}` references
  theme-neutral via role names. Partly depends on what design.json /
  DesignSync can express.
- **New type tokens?** Sub-question of the twins fork: does micrographics
  re-weight existing roles or add smaller sizes — legal under the
  ceiling, but new sizes land in DESIGN.md and ripple into the clearance
  math.

## Decisions & trade-offs

Both open forks closed as decision docs:

- [[2026-08-12_chrome-only-themes]] — themes are chrome-only; the geometry
  twins stay theme-invariant, pinned by `src/styles.guardrails.test.ts`
  (absence-based: no invariant token name in any `[data-theme]` block).
- [[2026-08-12_theme-persistence-localstorage]] — `wikilens.theme` in
  localStorage; the reader owns the default; zero IPC rides a pick.

Smaller calls, recorded here: the DESIGN.md theme dimension is **flat
theme-prefixed keys** (the detector never recurses — nested `themes:` tables
would be silently invisible and every micrographics literal a finding);
the default theme is the **absence** of `data-theme`, so `:root` stays the
single default source; label transforms split as "JS owns characters, CSS
owns case" so aria-labels stay sentence case by construction;
`--tracking-brand` shipped at **0.14em** (the approved preview) over the
mirror sheet's 0.12em — the mirror gets corrected at true-up.

## Status log

- 2026-08-12 — created.
- 2026-08-12 — active. The design pass (step 6) happened in the claude.ai/design
  project first: `tokens/wikilens-tokens-micrographics.css` (token swap, chrome-only,
  geometry twins untouched) + the "Instrument" component-voice layer picked from
  exploration C. User locked two calls: labels faithful to the preview (copy
  transforms — brackets/underscores — visible text only, aria stays sentence case)
  and `--tracking-brand: 0.14em` (the approved preview over the sheet's 0.12em).
  Step 2's sidecar attribution corrected: the impeccable `document` workflow, not
  DesignSync.
- 2026-08-12 — done. Shipped as `33d9d72` (DESIGN.md theme dimension + sidecar)
  and `078bd20` (picker, persistence, token block, instrument layer, guardrail
  tests). Both forks closed as ADRs (see Decisions). Follow-up that is NOT in
  the repo: the claude.ai/design mirror true-up (token sheet PROPOSAL→shipped,
  0.14em fix, preview cards) — post-merge, per the 07-30 ritual.
