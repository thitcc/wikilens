---
title: Adopt the impeccable design context as tracked project docs
type: decision
status: done
created: 2026-07-18
updated: 2026-07-18
tags: [frontend, overlay]
related: ["[[2026-07-05_design-tokens-claude-design-sync]]"]
commit: 506c0f4
---

# Adopt the impeccable design context as tracked project docs

## Context
The overlay's visual system was disciplined but undocumented above the token
layer: `styles.css` names every value (the 2026-07-05 token extraction), yet
nothing recorded the strategy — who the design serves, what it refuses to do —
or the system's rules in a form other tools could read. Running the impeccable
design skill produced that layer: `PRODUCT.md` (register, users, positioning,
design principles), `DESIGN.md` (the visual system, with a normative YAML
token frontmatter), and a machine-readable sidecar + live-mode config under
`.impeccable/`. Its first critique of the overlay panel (2026-07-18, 31/40)
showed the doc/tool gap concretely: the deterministic detector flagged five
intentional micro-values (9px carets, 10px monogram lettering, the 3px
progress pill cap) simply because DESIGN.md didn't list them.

## Decision
Track impeccable's design context in the repo — `PRODUCT.md` and `DESIGN.md`
at the root, `.impeccable/design.json` and `.impeccable/live/config.json`
alongside — with DESIGN.md's frontmatter as the normative token source (the
micro-glyph sizes now documented), a CLAUDE.md "Design Context" pointer for
both agents, and critique snapshots kept local via `.gitignore`
(`.impeccable/critique/`).

## Consequences
- Good: both agents (Claude, and Codex via the CLAUDE.md fallback) get the
  design strategy and visual rules without re-deriving them from
  `styles.css`; the detector and DESIGN.md now agree (`detect.mjs` exits
  clean), so any future finding is signal, not noise.
- Cost: two more root docs to keep truthful when `styles.css` changes —
  DESIGN.md's frontmatter joins the repo's paired-constants discipline
  (tokens ↔ stylesheet values).
- Follow-ups: two critique-derived roadmap ideas recorded in CLAUDE.md §6 —
  panel corner-pick, footer vocabulary collapse.

## Alternatives considered
- **Design knowledge in CLAUDE.md §5 + styles.css comments only** — rejected:
  no strategic layer, and nothing machine-readable for the detector or live
  mode; every design conversation restarts from a code crawl.
- **Suppress the five findings with inline `impeccable-disable` waivers** —
  rejected: waivers silence, tokens document; the values are intentional
  design and belong in the system's single source.
- **Land the docs but leave `.impeccable/` untracked** — rejected: the
  sidecar and live config are what make the design system renderable and
  iterable in the browser; untracked, every clone regenerates them and
  drifts.
