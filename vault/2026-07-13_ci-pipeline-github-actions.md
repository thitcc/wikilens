---
title: CI pipeline on GitHub Actions
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, build]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_frontend-test-harness]]", "[[2026-07-13_dependency-audit-lint-gates]]"]
commit:
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
  No work started.
