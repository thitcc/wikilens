---
title: Themes are chrome-only — the geometry twins stay theme-invariant
type: decision
status: done
created: 2026-08-12
updated: 2026-08-12
tags: [frontend, overlay]
related:
  - "[[2026-08-12_theme-switching-micrographics]]"
  - "[[2026-08-12_theme-persistence-localstorage]]"
commit: "078bd20"
---

# Themes are chrome-only — the geometry twins stay theme-invariant

## Context

The theme plan's sharpest fork ([[2026-08-12_theme-switching-micrographics]],
"the twins problem"): `--menu-clearance` / `--menu-clearance-top` are derived
from *rendered* footer/header heights and have a JS twin in
`menuPlacement.ts`; `--panel-gap` / `--shadow-room-*` pair with `window.rs`
constants. A theme that changes type metrics changes rendered heights and
desyncs the twins — but mono type is central to the micrographics register,
so the design pass could have pushed the other way. It didn't: the
Micrographics token sheet holds every size, leading, pad, and clearance
byte-identical (numeric line-heights make rendered boxes font-face-independent),
and its instrument layer is flat marks — pseudo-elements in existing gaps and
line boxes (the header ruler rides the panel's 12px flex gap; the barcode is
12px inside a ~20px chip line; chip labels drop 13px→12px, shrinking chrome,
never growing it).

## Decision

A theme may change chrome only — ink, surfaces, borders, shape, shadow
values, and the type *voice* (face, weight, tracking) — never rhythm, pads,
text sizes, leadings, blur, or the JS-twinned geometry tokens; and
instrument-layer decoration must be height-neutral by construction.

## Consequences

- Good: the twins never desync, no per-theme constants on either side of the
  pair, and `src/styles.guardrails.test.ts` turns the rule into a CI gate
  (absence-based: no invariant token *name* may be declared in any
  `[data-theme]` block — auto-covers theme #3). DESIGN.md §8 documents the
  invariant list.
- Cost / bad: a future theme that genuinely wants bigger type or denser
  leading is blocked until someone makes the twins theme-aware — that work
  is the revisit trigger, and it starts by amending this decision.
- Follow-ups: none; the guardrail test and DESIGN.md §7's Do carry it.

## Alternatives considered

- Theme-aware twins (per-theme clearance constants in `menuPlacement.ts` /
  `window.rs` mirrored by per-theme CSS values) — strictly more expressive,
  rejected now: the micrographics design pass proved the register achievable
  inside invariant metrics, so the heavier machinery would ship unused.
