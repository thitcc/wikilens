---
title: Parser snapshot and property tests (insta + proptest)
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, rust, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-04_rendered-html-extraction]]", "[[2026-07-02_wikitext-extraction]]"]
commit:
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
