---
title: Human-readable PR descriptions — plain summary, behavior bullets, details folds
type: plan
status: active
created: 2026-07-14
updated: 2026-07-14
tags: [build]
related: ["[[2026-07-13_pr-delivery-workflow]]"]
commit:
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
  the file/function detail in a per-bullet `<details>` fold; a loud
  "⚠️ Behavior changes" section (or "None"); a Verification table
  (check | result); ~20 rendered lines above the fold; war stories folded.
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
- **Per-bullet `<details>` folds, not quote blocks.** GitHub renders
  `<details>` inside list items (2-space indent; blank line after
  `</summary>`), so the quote-block fallback is unnecessary.
- **CLAUDE.md enforces; the template is the skeleton.** `gh pr create --body`
  — how Claude opens every PR — bypasses GitHub's template auto-fill, so the
  template alone would never reach Claude-authored bodies. It still serves
  web-UI PRs and stays the single canonical copy of the skeleton.
- Convention over tooling: no body linter in CI — not worth a gate in a solo
  repo.

## Status log
- 2026-07-14 — created; anatomy agreed with the user.
