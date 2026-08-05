---
title: npm run bump — one-command release version bump
type: plan
status: done
created: 2026-08-04
updated: 2026-08-04
tags: [build]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-08-04_versioning-adoption]]"
commit: [2d4f8a5, db3d40c, b1107fc, 7230081]
---

# npm run bump — one-command release version bump

## Context / problem

The release PR (ritual step ②) hand-edits the version in three manifests
(`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`) and
refreshes two lockfiles. Five chances to typo or forget one, and a mismatch
is the annoying kind of bug: the installer filename says one version, the
app another.

## Goal / non-goals

- Goal: `npm run bump 0.2.0` — a small Node script that rewrites the version
  in the three manifests, refreshes the locks (`npm install
  --package-lock-only`; `cargo check` from `src-tauri/`), and prints what
  changed.
- Non-goal: choosing the number, tagging, release notes — those stay in the
  ritual's other steps.

## Approach

1. Script beside the other Node tooling; wired as the `bump` npm script.
2. Unit coverage rides `npm run test:node` if the parsing warrants it.

## Decisions & trade-offs

Deliberately built **inside the first real release PR**, not ahead of time —
the release itself verifies it against a bump that actually ships. If the
five-file dance turns out not to annoy, skip it forever.

## Status log

- 2026-08-04 — created as an idea from the versioning explainer session.
- 2026-08-04 — owner's call: built in its own PR, not inside the first
  release PR — deviating from the trade-off above. No version changes ride
  along (the manifests stay 0.1.0); the script was still verified against a
  real bump-and-revert on this machine, and the first release PR
  live-verifies it again.
- 2026-08-04 — done: engine + tests at `.claude/skills/bump/` (byte-spliced
  EOL-preserving rewrites, [package]-scoped Cargo.toml locator, manifest
  restore on a failed lock refresh), wired as `npm run bump`. CLAUDE.md §3,
  the release-build skill, the versioning explainer, and README now point
  at the command.
