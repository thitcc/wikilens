---
title: Parser snapshot and property tests (insta + proptest)
type: plan
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [testing, rust, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-04_rendered-html-extraction]]", "[[2026-07-02_wikitext-extraction]]"]
commit: [d596925, 61ae86d, b5903a7]
---

# Parser snapshot and property tests (insta + proptest)

## In simple terms
The HTML and wikitext cleaners embody hard-won behaviors — infobox rows
surviving as `label | value` lines, navboxes dropped, tables stripped from the
fallback path. Snapshot tests freeze the exact output over real captured wiki
pages, so any change to a cleaner shows up as a reviewable diff instead of a
silent answer-quality regression. Property tests then throw random garbage at
the parsers to prove they never crash. Priority 4 of the testing audit.

## Context / problem
The cleaners' happy paths are solidly unit-tested with inline snippets, but the
hand-rolled scanner's edge cases are not ([[2026-07-13_testing-audit]]): `>`
inside quoted attributes, unterminated tags/comments, attr-name substring false
positives. One confirmed bug-class: **an unbalanced `{{` or `{|` silently
discards the rest of the page** in `wikitext::remove_balanced` — unclosed `[[`
recovers gracefully, unclosed braces don't, and downstream the page cleans to
empty and gets dropped ("couldn't read their contents").

## Goal / non-goals
- **Goal:** a checked-in fixture corpus of real wiki pages with `insta` snapshots
  of `to_plaintext` output, plus proptest no-panic/no-residue properties over
  `html::to_plaintext`, `wikitext::to_plaintext`, and both SSE line parsers.
- **Non-goal:** cargo-fuzz (skipped by the audit — proptest captures ~90% of the
  value here) and snapshotting live pages (fixtures are static captures; the
  live golden suite owns drift detection).

## Approach
- **insta** (+ `cargo-insta` for review): fixture files per engine variant —
  Stardew 2-column infobox page, a Fandom portable-infobox page, a UESP
  namespaced page, and one raw-wikitext fallback sample. `assert_snapshot!` the
  cleaned output; intentional parser changes become a one-keystroke
  `cargo insta review` accept.
- `.gitattributes`: force LF on `*.snap` and fixture files so Windows autocrlf
  doesn't churn snapshots (shared with [[2026-07-13_ci-pipeline-github-actions]]).
- **proptest** (second step): no-panic on arbitrary input for the three parsers;
  no `{{`/`[[` residue in cleaned wikitext output; `to_plaintext` output length
  bounded relative to input.
- Seed the unbalanced-brace case explicitly (stray `{{`, `}}`, `{|`, `|}`) and
  fix `remove_balanced` to degrade gracefully — keep prose after an unclosed
  marker instead of dropping the tail. The fix lands with this plan since the
  test defines the intended behavior.

## Decisions & trade-offs
- Snapshots of *reduced output*, not raw HTML structure — the plaintext handed to
  the LLM is the contract worth freezing.
- proptest over quickcheck (actively maintained, shrinking gives minimal failing
  inputs).

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 4.
  No work started.
- 2026-07-13 — **done.** Landed as fix → snapshots → properties:
  - **`remove_balanced` fix** shipped as a single-pass mark-stack (emit
    everything, record `out.len()` at each open, truncate back on its close)
    instead of the depth-gated suppressor: balanced removal byte-identical,
    an unmatched open drops just its marker and keeps the trailing content
    (inner balanced regions still removed), stray closes stay literal —
    matching the module's leave-ambiguous-text-literal philosophy.
  - **Snapshots** (insta, 4): stardewvalleywiki Parsnip (default-engine 2-col
    infobox), Core Keeper Copper Ore (Fandom portable infobox), UESP
    Skyrim:Iron (namespaced), and Stardew Wood raw wikitext through the
    fallback cleaner. Fixtures carry provenance headers (stripped as HTML
    comments, never reach output); `.gitattributes` pins them to LF next to
    the existing `*.snap` rule. Intentional cleaner changes are re-accepted
    with `INSTA_UPDATE=always` and reviewed as a `.snap` diff.
  - **Properties** (proptest, 10): no-panic + byte-length bound for both
    cleaners on arbitrary chars and marker soup; no-panic + non-`data:` →
    `Ignore` for both SSE line parsers; no marker residue over generated
    balanced wikitext; unmatched-open tail preservation for both marker
    pairs. **Deviation from the plan as written:** the no-residue property is
    scoped to a balanced-construct strategy, not arbitrary input — the
    universal claim is provably false (replace-with-empty passes can join
    separated `{` bytes: `{''{` cleans to `{{`), and unclosed `[[` keeps its
    literal marker by design. Stray closes were kept literal (the plan's
    seeded stray-`}}`/`|}` cases pin passthrough, not dropping).
