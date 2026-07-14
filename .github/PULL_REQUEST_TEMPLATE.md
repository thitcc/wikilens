<!-- WikiLens PR anatomy. Target: ~20 rendered lines above the fold.
     Note: gh pr create with a body flag BYPASSES this template. When
     authoring via gh, write the body to this skeleton yourself
     (convention: CLAUDE.md §4, "PR descriptions"). -->

## Summary

<!-- 2-3 sentences, plain language, zero file/function jargon: what changed
     and why anyone should care. Then link the vault plan doc for depth. -->

Plan: `vault/YYYY-MM-DD_slug.md`

## Changes

<!-- One bullet per change, written at workflow/behavior level:
       GOOD: "Wiki page downloads now stop at a size cap instead of reading forever."
       BAD:  "`fetch.rs` now calls read_body_capped() instead of .text()."
     Every bullet nests its file/function detail in a details fold.
     Copy this exact shape. The blank lines are load-bearing: the one after
     </summary> is what makes GitHub render markdown inside the fold, and the
     2-space indent keeps the fold inside the bullet.

- Human-readable statement of the change.

  <details>
  <summary>Technical details</summary>

  File/function-level detail. Markdown works here: `code`, lists, links.

  </details>
-->

## ⚠️ Behavior changes

<!-- Anything a user or developer will notice working differently
     (defaults, timeouts, UI, env vars, error text). If none, write: None. -->

## Verification

<!-- One row per check actually run; add rows for manual or live checks. -->

| Check | Result |
| --- | --- |
| `/check` (type-check, tests, clippy) | ✅ |

<!-- Long war stories (flaky live suites, dead ends, debugging sagas) go at
     the bottom of the PR in their own details fold, never in the sections
     above. -->
