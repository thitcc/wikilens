---
title: Show the running version in Settings
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [frontend, tauri]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-08-04_versioning-adoption]]"
  - "[[2026-07-19_hotkey-config]]"
commit:
---

# Show the running version in Settings

## Context / problem

Once installed, nothing on screen says which build is running — the version
lives only in the manifests and the installer filename. "Did I update this?"
and any future bug report both want the number visible in-app.

## Goal / non-goals

- Goal: a quiet muted `WikiLens <version>` row at the bottom of the Settings
  panel, read from `tauri.conf.json`'s version via Tauri's `getVersion()`.
- Non-goal: an About dialog, update checks, or anything beyond the one row.

## Approach

1. `src/api.ts` grows a tiny `getAppVersion()` wrapper over
   `@tauri-apps/api/app`'s `getVersion()` — components never import
   `@tauri-apps/*` directly.
2. `SettingsMenu.tsx` renders the row as muted metadata (14px ceiling, no
   new tokens expected).

## Decisions & trade-offs

Cheapest of the versioning follow-ups and the first worth doing — but
pointless before the first tag exists, since every install reads 0.1.0
anyway.

## Status log

- 2026-08-04 — created as an idea from the versioning explainer session.
