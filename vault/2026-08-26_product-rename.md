---
title: Rename the product away from the wikilens.app collision
type: plan
status: idea
created: 2026-08-26
updated: 2026-08-26
tags: [build, overlay]
related:
  - "[[2026-08-26_open-source-release-0-2-0]]"
  - "[[2026-08-04_release-scoped-semver]]"
commit:
---

# Rename the product away from the wikilens.app collision

## Context / problem
`wikilens.app` is a live, unrelated product (a German-language SPA, online
since at least 2026-03) and the domain is not the owner's. The scaffold
picked `com.wikilens.app` as the Tauri identifier and `https://wikilens.app`
as the OpenRouter `HTTP-Referer` on the assumption the name was free. The
open-source prep ([[2026-08-26_open-source-release-0-2-0]]) moves both to
neutral values — identifier `io.github.thitcc.wikilens`, Referer
`https://github.com/thitcc/wikilens` — but the product name "WikiLens"
itself still collides, and the owner intends to rename after 0.2.0 ships.

## Goal / non-goals
- Goal: pick a name that is free as a domain, a GitHub repo, and a Windows
  product name; apply it everywhere the current name is load-bearing.
- Non-goal: doing it inside the 0.2.0 release arc.

## Approach
Rename **before a public install base exists** — ideally the release right
after 0.2.0 — because three things key on the name and are painful to move
once installed: the Tauri identifier (the app-data dir name: `settings.json`,
`keys.json`, `history.json`, `wikis.json` — a MAJOR-class change under
[[2026-08-04_release-scoped-semver]] unless a migration copies the folder),
the NSIS/MSI upgrade identity (`productName` + `bundle.publisher`), and the
GitHub repo URL (redirects follow a rename, but the CI badge, `repository`
fields and Referer should be updated). Also: `productName`, the window
titles, the tray tooltip, the `User-Agent` (`wiki::USER_AGENT`), the
`wikilens.*` localStorage keys (a one-time migration or accept the reset),
`WIKILENS_*` env vars (keep as aliases for a release), the `wikilens_lib`
crate name, the vault's own name (docs are permanent IDs — leave them),
and DESIGN/PRODUCT prose.

## Decisions & trade-offs
- Whether to migrate app-data across the identifier change or accept a
  reset is the fork to settle when the name is chosen; a folder copy at
  first launch is cheap and keeps DPAPI keys valid (per-user scope).

## Status log
- 2026-08-26 — created as an idea during the open-source prep; the interim
  identifier/Referer change ships in that PR.
