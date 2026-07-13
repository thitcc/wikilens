---
title: Dependency audits, lint gates, and config guardrail tests
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, security, build, tauri]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_ci-pipeline-github-actions]]"]
commit:
---

# Dependency audits, lint gates, and config guardrail tests

## In simple terms
Some of the project's most important rules exist only as prose in CLAUDE.md:
never grant the webview network access, never let API keys reach the frontend,
keep the opener's URL scope. Prose can't fail a build — this plan turns those
rules into executable checks, and adds scanners that warn when a dependency has
a known vulnerability. Priority 5 of the testing audit.

## Context / problem
No cargo-audit or npm audit anywhere; clippy is not enforced; `tsc` runs only as
a side effect of `npm run build`. The security conventions in CLAUDE.md §4-5 are
unenforced — the opener-scope gotcha ("compiles fine, `ForbiddenUrl` at
runtime") is exactly the class of drift only a config test catches. See
[[2026-07-13_testing-audit]].

## Goal / non-goals
- **Goal:** dependency-vulnerability scanning in CI, clippy as a hard gate, and
  config-as-data guardrail tests pinning the security invariants.
- **Non-goal:** a license policy, lockfile-freshness automation, or mutation
  testing (all descoped by the audit).

## Approach
- **CI additions** (into [[2026-07-13_ci-pipeline-github-actions]]): `cargo audit`
  (rustsec) and `npm audit --audit-level=high` — fail on high/critical, warn
  below, so noise doesn't train us to ignore red.
- **Lint gates:** `cargo clippy --all-targets -- -D warnings`; `npx tsc --noEmit`
  as an explicit step (not just inside build); both also added to the `/check`
  skill.
- **Config-as-data guardrail tests** (plain Rust `#[test]`s that parse the JSON
  files as fixtures):
  - `capabilities/default.json` grants no `http:*`/`fs:*` permissions, and the
    opener permission carries an inline `http(s)://*` URL scope.
  - `tauri.conf.json` CSP shape: `connect-src` limited to IPC, `img-src` limited
    to `'self' data:`, no remote script origins.
  - `ProviderInfo` serialization contains no key material (serialize a fixture,
    assert no env-var names/values leak).
  - User-facing error strings never embed key values (scrub check on
    `AppError` display output built from a fake key).

## Decisions & trade-offs
- Guardrail tests read the real config files from the repo, so they drift-check
  the actual shipped configuration — slightly unusual for unit tests,
  deliberately so.
- Audits pinned at high/critical: a solo project can't triage every advisory;
  the gate must stay credible.

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 5.
  No work started.
- 2026-07-13 — one slice landed early: the clippy `-D warnings` gate is live in
  CI (`4c2a05a`) and `/check` now runs it (`b80df5d`, with
  [[2026-07-13_pr-delivery-workflow]]). Remaining scope here: cargo-audit /
  npm audit in CI, explicit `tsc` gate, and the config-as-data guardrail tests.
