---
title: Tag-triggered release CI — build installers into a draft Release
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [build, testing]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-07-13_ci-pipeline-github-actions]]"
  - "[[2026-08-02_release-build-skill]]"
  - "[[2026-07-13_manual-smoke-checklist-live-cadence]]"
commit:
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
