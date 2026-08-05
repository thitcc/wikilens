---
name: bump
description: Bump the release version in one command — npm run bump X.Y.Z rewrites package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json and refreshes both lockfiles (npm install --package-lock-only + cargo check). Invoke only when prepping a release PR or when asked to bump the app version.
allowed-tools: Bash(npm:*), Read
---

# bump

One-command release version bump. The engine is
`.claude/skills/bump/bump.mjs`; versions bump **only in release PRs**
(CLAUDE.md §3 — release-scoped SemVer, ADR
`vault/2026-08-04_release-scoped-semver.md`), so this runs at most once per
release, on the `chore/release-X-Y-Z` branch.

## How to run
From anywhere in the repo (the script resolves the manifests relative to itself):

```
npm run bump 0.2.0
```

It rewrites the version in the three manifests (`package.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`), refreshes both locks —
`npm install --package-lock-only` (may need the network), then `cargo check`
(also proves the edited Cargo.toml still compiles) — prints an old → new line
per file, and exits non-zero on any failure.

## What it refuses
- A version that isn't plain `X.Y.Z` (no `v` prefix, no prerelease suffix).
- The version the manifests already have.
- Manifests that disagree on the current version — that mismatch is the bug
  this tool exists to prevent; fix them by hand first.

## On failure
A failed lock refresh restores all three manifests, so a retry after fixing
the environment starts clean; the error says to check `git status` for a
half-refreshed lock. After a successful run: `/check`, then open the release
PR per the ritual.

## Tests
`npm run test:node` runs the tooling unit tests, including this script's
(`.claude/skills/bump/bump.test.mjs`, Node's built-in runner, no dependencies).
