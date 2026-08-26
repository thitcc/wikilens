---
title: Tag-triggered release CI — build installers into a draft Release
type: plan
status: done
created: 2026-08-04
updated: 2026-08-26
tags: [build, testing]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-07-13_ci-pipeline-github-actions]]"
  - "[[2026-08-02_release-build-skill]]"
  - "[[2026-07-13_manual-smoke-checklist-live-cadence]]"
commit: [7e6e506, 7c1011e, c1a798f]
---

# Tag-triggered release CI — build installers into a draft Release

## Context / problem

Ritual steps ⑤–⑥ are fully manual: local `npm run tauri build`, smoke
checklist, `gh release create` with hand-uploaded installers. The local
build that gets smoked and the artifact that gets shipped are only
convention-identical, and the human does all the ferrying.

## Goal / non-goals

- Goal: a second workflow triggered on `v*` tag pushes — `windows-latest` +
  `tauri-apps/tauri-action` builds the NSIS/MSI installers and creates a
  **draft** GitHub Release with them attached. The human downloads the CI
  artifact, walks the smoke checklist against it, pastes the results, and
  promotes the draft.
- Non-goal: auto-publish (the human stays the last gate — CI never makes a
  release visible); code signing (artifacts stay unsigned, same as local
  builds, so SmartScreen behavior is unchanged).

## Approach

1. `.github/workflows/release.yml` on `push: tags: ['v*']`, separate from
   `ci.yml`.
2. `tauri-apps/tauri-action` with `releaseDraft: true`; cache Cargo via
   `rust-cache` — a cold Tauri release build runs 15–25 minutes.
3. Smoke-checklist results get pasted into the draft before promotion, per
   the checklist's own instruction.

## Decisions & trade-offs

Smoked bytes become shipped bytes — a correctness upgrade over the manual
ritual, not just a convenience. Threshold to build it: >2 releases/month;
below that, the manual path is cheaper than debugging a CI build
environment.

## Status log

- 2026-08-04 — created as an idea from the versioning explainer session.
- 2026-08-04 — promoted from idea on the owner's call: the doc's own
  >2-releases/month threshold isn't met, but the versioning plans are being
  terminated rather than left open, and the correctness upgrade (smoked
  bytes = shipped bytes) stands on its own.
- 2026-08-04 — done: `release.yml` in 7e6e506 (`tauri-apps/tauri-action@v1`
  into a draft Release, tag↔package.json guard before the 20-minute build,
  `uploadUpdaterJson: false`, upload-only `retryAttempts: 3`); docs retold
  around the draft in 7c1011e + c1a798f (release.md/.html, versioning.html,
  smoke checklist, CLAUDE.md, release-build skill, README). True end-to-end
  verification lands on the first real tag push — same deal as the bump
  script.
- 2026-08-05 — correction, caught by the pre-tag audit on the 0.1.1 release
  PR: `retryAttempts` retries the **build** as well as the uploads (action.yml
  — "re-try building the app if the initial build fails"; `runner.ts` wraps
  `execTauriCommand` in `retry()`), so the 2026-08-04 entry's "upload-only" is
  wrong. At 3 a broken build would compile four times, past the 60-minute
  timeout, with the error buried; `release.yml` now sets 0 — fail once, fail
  legibly, re-run from the Actions tab for the upload-flake case.
- 2026-08-26 — the first real tag push, on record: run 31037775479 on
  `v0.1.1` (2026-08-05) built both installers and uploaded them into a draft
  Release in 18 minutes; the draft was later deleted unpublished (the
  runbook's unsmoked-draft path — no smoke walk was recorded) and the tag
  stays. The CI build-and-upload half is therefore verified end to end; the
  publish half lands with 0.2.0, the first published GitHub Release
  ([[2026-08-26_open-source-release-0-2-0]]).
