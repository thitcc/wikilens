---
title: Auto-updater via tauri-plugin-updater
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [build, security, tauri]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-08-04_tag-triggered-release-ci]]"
commit:
---

# Auto-updater via tauri-plugin-updater

## Context / problem

Publishing 0.3.0 doesn't reach existing installs — every update means a
manual re-download. `tauri-plugin-updater` closes that gap: the installed
app checks a `latest.json` manifest (attachable to each GitHub Release) and
offers to download and apply the new installer in place.

## Goal / non-goals

- Goal: installed apps notice a new release and self-update after user
  consent.
- Non-goal: silent/forced updates; building this before installs exist that
  the owner doesn't control.

## Approach

1. Land [[2026-08-04_tag-triggered-release-ci]] first — `tauri-action`
   generates the artifact signatures and `latest.json` as part of the same
   build.
2. Generate and custody the minisign keypair: the private key signs every
   release artifact, the app ships the public key. Losing the key strands
   every existing install (no future update can ever verify); leaking it
   lets someone feed users malicious updates.
3. Wire the plugin (Rust + capability + frontend consent flow through
   `src/api.ts`).

## Decisions & trade-offs

Heaviest of the versioning follow-ups and deliberately last: it changes the
release ritual (signatures + manifest on every release) and its payoff only
exists once real external users do — the same milestone the ADR sets for
1.0.0.

## Status log

- 2026-08-04 — created as an idea from the versioning explainer session.
