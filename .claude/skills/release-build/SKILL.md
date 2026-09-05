---
name: release-build
description: Build the packaged WikiLens executable and Windows installers locally with npm run tauri build — where the artifacts land, what changes when the app no longer runs from a terminal (.env, tray-only launch), and the post-build smoke step. Invoke when asked to build or package the app, produce an exe or installer, or prep a release (versioned releases build in CI from the tag push; this is the local/fallback build).
allowed-tools: Bash(npm:*), Read
---

# release-build

One command packages the app; everything non-obvious is what surrounds it.
This skill records the verified artifact paths and the packaged-runtime
caveats so a build/package request never re-derives them.

## How to run

From the repo root:

```
npm run tauri build
```

It runs `npm run build` (`tsc && vite build`) first — a type error aborts
before any Rust compiles — then a `release`-profile cargo build, then the
bundlers. Rebuilds are incremental; a cold build is the slow one (~2.5 min
Rust compile on the dev machine), and the **first-ever** bundle also
downloads the WiX + NSIS toolchains, so it needs network once.

## What you get (all gitignored under `src-tauri/target/`)

- `src-tauri/target/release/wikilens.exe` — standalone, double-clickable
  (~13 MB).
- `src-tauri/target/release/bundle/nsis/WikiLens_<version>_x64-setup.exe` —
  NSIS installer; bootstraps WebView2 if the machine lacks it.
- `src-tauri/target/release/bundle/msi/WikiLens_<version>_x64_en-US.msi` —
  WiX/MSI installer.

`<version>` is `tauri.conf.json`'s `version`. Hand people an **installer**
(Start-menu entry, uninstall, WebView2 bootstrap); the raw exe is fine for
local use but assumes WebView2 is present (it ships with Windows 10/11).

The installers name their publisher and copyright from `tauri.conf.json`'s
`bundle` block (`publisher`, `copyright`) — that string is what Add/Remove
Programs shows and what the NSIS upgrade check matches on, so it changes
only with a deliberate rename.

## Running the packaged app — what changes vs `npm run tauri dev`

- **Launching looks like nothing happened. That's correct.** Release builds
  have no console (`windows_subsystem = "windows"` in `main.rs`) and every
  window is created hidden — the app is the tray icon plus the summon
  hotkey (default Ctrl+`).
- **`.env` stops working.** A packaged app's cwd is unpredictable, so
  dotenv is dev-only: the `WIKILENS_*_MODEL` overrides and the
  retrieval-tuning/debug vars (README "Retrieval tuning (advanced)") must be
  **OS env vars** for a packaged install. Those README tables are the var
  contracts — don't re-list them.
- **Both answer modes are unaffected**: API keys live in the DPAPI store
  (`keys.json`, app-data) and the Local AI config in `settings.json` — all
  of it set from Settings → Answers, never env.

## After building

Walk `docs/smoke-checklist.md` — the packaged/runtime surface (overlay,
hotkey, tray, DPI, keys) has no automated coverage. Item 10 specifically
runs on the packaged **installer**; a quick exe sanity check (tray icon
appears, Ctrl+` summons over a borderless window) covers a non-release
build.

## Cutting a versioned release

The build alone isn't a release — and for a release, CI runs this build.
Versions are release-scoped SemVer (ADR:
`vault/2026-08-04_release-scoped-semver.md`): the version comes from
`tauri.conf.json` and bumps **only in a release PR** (`chore/release-X-Y-Z`
— `npm run bump X.Y.Z`, which rewrites the three manifests and refreshes
both locks via `npm install --package-lock-only` + `cargo check`; then run
`/check`), never per-merge. Then: human merges → human tags the merge commit
(`git tag -a vX.Y.Z` + push — tagging mutates `main`, so it stays
owner-side) → the tag push triggers `.github/workflows/release.yml`, which
runs this same build on CI and attaches the NSIS/MSI installers to a
**draft** GitHub Release → smoke checklist + `--ignored` live suites against
the **downloaded** installers → human publishes the draft. This local build
stays the dev/fallback path; the artifact paths and caveats above still
apply to it. Release notes live in GitHub Releases only — no CHANGELOG.md.
