---
title: Auto-updater via tauri-plugin-updater
type: plan
status: dropped
created: 2026-08-04
updated: 2026-08-05
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
- 2026-08-05 — **dropped.** Two facts settled after
  [[2026-08-04_tag-triggered-release-ci]] shipped, both from the owner:
  the repo stays private permanently, and distribution will be a storefront
  (Steam or similar), never a direct download.
  - The Approach's mechanism can't work. Release assets on a private repo
    need an authenticated request, so an installed app can't fetch
    `latest.json` from GitHub Releases. The only workaround is embedding a
    token in a shipped binary — extractable, and it would grant read access
    to the source.
  - A storefront *is* the updater. Steam patches every install from its own
    depots and tracks a file manifest, so binaries the app rewrote itself
    get flagged and reverted by "Verify integrity of game files". Two update
    systems disagreeing about what's on disk is worse than one.
  - Revive only if WikiLens is ever distributed as a direct download outside
    a storefront. If that happens, note the ordering trap: the updater's
    public key is compiled into the binary, so it must be wired into the
    **first** build anyone installs — an install shipped without the plugin
    can never self-update, whatever is published afterwards.
  - What actually gates a stranger installing this is code signing, not
    updating: the installers are unsigned, SmartScreen warns on every
    install, and a storefront doesn't sign your binaries for you. Not
    planned anywhere yet.
