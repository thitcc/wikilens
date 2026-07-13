---
title: Dependency audits, lint gates, and config guardrail tests
type: plan
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [testing, security, build, tauri]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_ci-pipeline-github-actions]]", "[[2026-07-13_pr-delivery-workflow]]"]
commit: [a323c22, e811ce9, 1438a9b]
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
- 2026-07-13 — **done.** Notes against the plan as written:
  - The "explicit `tsc` gate" remainder was already stale — `npx tsc --noEmit`
    has been a standalone CI step since [[2026-07-13_ci-pipeline-github-actions]]
    landed. Nothing to do there.
  - **Audits** landed as a third CI job (ubuntu — audits only read lockfiles):
    `npm audit --audit-level=high` straight off `package-lock.json` (no
    `npm ci`), `cargo audit` from `src-tauri/` via a prebuilt
    taiki-e/install-action binary. Severity mapping decision: the npm flag
    exits non-zero only at high/critical; cargo-audit fails on any RUSTSEC
    *vulnerability* and warns on informational advisories — it has no
    CVSS-threshold flag, so that's the closest fit to "fail on high/critical,
    warn below". No scheduled cron (deliberate: PR/push-triggered only).
  - **Triage at gate-landing time:** npm reported 0 advisories at any level.
    cargo-audit flagged quick-xml < 0.41 (RUSTSEC-2026-0194/0195, both 7.5
    high) at three lockfile sites: the one compiled on Windows
    (plist → tauri-utils) was cleared by bumping plist to 1.10.0; the two
    confined to xcap's Linux-only subtrees (xcb build-dep, wayland-scanner
    proc-macro — never compiled for the Windows-only target, build-time
    parsing of vendored protocol XML) have no semver-compatible fix and are
    ignored in `src-tauri/.cargo/audit.toml` with dated justifications and a
    recheck-on-xcap-bump trigger. 17 informational warnings (unmaintained
    gtk3/unic crates, glib unsoundness — all Linux-side) warn without failing.
  - **Guardrail tests** landed as `src-tauri/src/config_guardrails.rs`
    (test-only module, gated like `test_support`): capabilities dir-iterated
    for no-http/fs + opener URL scope; prod CSP directive sets pinned exactly
    (connect-src = IPC pair, img-src = 'self' data:, effective script policy =
    'self', no unsafe-eval), devCsp deliberately unasserted; ProviderInfo's
    serialized field set pinned; configured key values asserted absent from
    provider JSON; MissingApiKey names the var, never a value; a sentinel-keyed
    request's transport error never echoes headers. Bite-checked: a poisoned
    connect-src and a fake `http:default` permission each fail their test.
    (Found in passing: tauri-build itself rejects unknown http:/fs:
    permissions while the matching plugin is absent — the guardrail covers the
    day someone adds the plugin.)
  - `/check` and CLAUDE.md now say "mirrors CI's *code* gates": the audits stay
    CI-side (network/advisory-dependent — a fresh advisory can redden CI with
    no code change, which is the gate working as designed).
