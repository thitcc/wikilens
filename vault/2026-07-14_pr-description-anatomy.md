---
title: Human-readable PR descriptions — plain summary, behavior bullets, details folds
type: plan
status: done
created: 2026-07-14
updated: 2026-07-14
tags: [build]
related: ["[[2026-07-13_pr-delivery-workflow]]"]
commit: [00df75b, 47542dc]
---

# Human-readable PR descriptions — plain summary, behavior bullets, details folds

## Context / problem
PR bodies to date (PR #10 is the reference offender) are dense 90-word bullets
of file/function jargon, nested parentheticals, and bold+inline-code overload —
accurate but unreadable. The delivery ADR ([[2026-07-13_pr-delivery-workflow]])
fixed how PRs land, not what their descriptions say. An anatomy was agreed with
the user on 2026-07-14.

## Goal / non-goals
- Goal: every future PR body follows the anatomy — 2–3-sentence plain-language
  summary linking the plan doc; change bullets at workflow/behavior level with
  the file/function detail in a `>` quote under the bullet (only when it adds
  something); a loud "⚠️ Behavior changes" section (or "None"); a Verification
  table (check | result); one source line per paragraph/bullet (no hard wraps);
  ~20 rendered lines; war stories folded.
- Goal: encode it where it bites — `.github/PULL_REQUEST_TEMPLATE.md` as the
  canonical skeleton (auto-fills the web UI), CLAUDE.md §4 as the enforcement
  point for Claude-authored bodies.
- Non-goal: rewriting historical PR bodies.
- Non-goal: CI-side body linting — convention only, revisit if bodies drift.

## Approach
1. Add `.github/PULL_REQUEST_TEMPLATE.md`: four-section skeleton, writing
   rules as non-rendering `<!-- -->` comments, including a worked
   bullet + nested-`<details>` example with the blank-line placement GitHub
   needs to render markdown inside the fold.
2. Add a compact "PR descriptions" bullet to CLAUDE.md §4, adjacent to the
   Delivery bullet, pointing at the template for the skeleton.
3. Point README "Development workflow" step 4 at the template.
4. Dogfood: this change's own PR is written in the new anatomy.

## Decisions & trade-offs
- **Per-bullet `>` quotes, not `<details>` folds** (reversed after the first
  dogfood, PR #13): folds rendered as three identical "Technical details"
  stubs that hid the substance behind clicks; a quote keeps the detail
  scannable and visually attached to its bullet. Folds stay for
  bottom-of-PR war stories only.
- **No hard-wrapped source in PR bodies.** GitHub renders single newlines in
  PR/issue bodies as line breaks (unlike README rendering), so 80-column
  wrapping comes out as ragged short lines — each paragraph/bullet is one
  source line.
- **CLAUDE.md enforces; the template is the skeleton.** `gh pr create --body`
  — how Claude opens every PR — bypasses GitHub's template auto-fill, so the
  template alone would never reach Claude-authored bodies. It still serves
  web-UI PRs and stays the single canonical copy of the skeleton.
- Convention over tooling: no body linter in CI — not worth a gate in a solo
  repo.

## Status log
- 2026-07-14 — created; anatomy agreed with the user.
- 2026-07-14 — **done.** Template + CLAUDE.md §4 bullet + README pointer landed
  (`00df75b`); the delivery PR dogfoods the anatomy.
- 2026-07-14 — revised after the first dogfood render (PR #13): per-bullet
  detail folds → `>` quotes, and PR bodies are written without hard line
  wraps (`47542dc`).
