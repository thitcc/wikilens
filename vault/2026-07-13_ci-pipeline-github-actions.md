---
title: CI pipeline on GitHub Actions
type: plan
status: done
created: 2026-07-13
updated: 2026-08-04
tags: [testing, build]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_frontend-test-harness]]", "[[2026-07-13_dependency-audit-lint-gates]]", "[[2026-07-13_pr-delivery-workflow]]"]
commit: [c68f99a, 4c2a05a]
---

# CI pipeline on GitHub Actions

## In simple terms
The project has 138 good tests and nothing runs them automatically — they only
run when someone remembers to type the command. CI is the robot that runs every
check on every push and fails loudly, so a regression can't merge unnoticed.
Priority 1 of the testing audit: everything else builds on this.

## Context / problem
There is no `.github/workflows/` at all. The only frontend gate is `tsc` as a
side effect of `npm run build`; the Rust suite gates nothing unless run by hand
(`/check`). See [[2026-07-13_testing-audit]].

## Goal / non-goals
- **Goal:** every push/PR runs type-check + clippy + the full offline Rust suite
  (plus `vitest run` once [[2026-07-13_frontend-test-harness]] lands), with no
  secrets required.
- **Non-goal:** coverage gates (coverage stays a local diagnostic).
- **Non-goal:** release/bundle pipeline — no `npm run tauri build` per push; full
  bundles (tauri-action) only on tags, later.
  *(2026-08-04: that "later" shipped — see the status log.)*
- **Non-goal:** running the live `#[ignore]` suites in CI (cadence handled by
  [[2026-07-13_manual-smoke-checklist-live-cadence]]).

## Approach
- One workflow, two jobs:
  - **ubuntu-latest** (cheap, fast): `npm ci` → `npx tsc --noEmit` → later
    `vitest run`. The frontend needs no Windows.
  - **windows-latest**: `cargo clippy --all-targets -- -D warnings` +
    `cargo test` in `src-tauri/`. WebView2 is preinstalled on the runner; none of
    the Linux system-deps dance applies.
- `Swatinem/rust-cache` + `dtolnay/rust-toolchain`; set `CARGO_PROFILE_DEV_DEBUG=0`
  (or `debug = 0` in the dev profile) to shrink the target-dir cache — Windows
  runners are ~2x slower and cache misses hurt most there.
- Live tests stay `#[ignore]`, so CI never needs API keys.
- Pre-empt CRLF churn: `.gitattributes` forcing LF on test fixtures/snapshots
  (shared concern with [[2026-07-13_parser-snapshot-property-tests]]).

## Decisions & trade-offs
- Rust job on Windows because that's the only shipping platform — a Linux-only
  `cargo test` could green-light Windows-specific breakage.
- `cargo test` over cargo-nextest for now (one less tool; nextest is a deferred
  nice-to-have per the audit).

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 1.
- 2026-07-13 — done; workflow in `4c2a05a` on the `ci/github-actions-pipeline`
  PR. Preflight surfaced 4 clippy errors (never gated before): two justified
  `#[allow(too_many_arguments)]` (ask's IPC contract, answer_streaming) and two
  mechanical fixes (`is_none_or`, char-array pattern) — `c68f99a`. One deviation:
  `.gitattributes` ships now with only the `*.snap` LF rule; fixture-dir entries
  wait for [[2026-07-13_parser-snapshot-property-tests]]. tsc + clippy + 128
  offline tests green locally; PR checks verified green before handoff.
- 2026-08-04 — the "full bundles (tauri-action) only on tags, later" non-goal
  shipped as its own workflow: [[2026-08-04_tag-triggered-release-ci]]
  (`release.yml`).
