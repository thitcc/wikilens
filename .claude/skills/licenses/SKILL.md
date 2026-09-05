---
name: licenses
description: Regenerate THIRD-PARTY-LICENSES.txt — the license texts of every Rust crate linked into wikilens.exe and every production npm package bundled into the frontend, one plain-text file the installers ship next to the exe; npm run licenses rewrites it, npm run licenses -- --check is CI's freshness gate. Invoke after a dependency change (Cargo.lock / package-lock.json), when that CI job fails, or when asked to regenerate the third-party licenses.
allowed-tools: Bash(npm:*), Bash(cargo:*), Read
---

# licenses

One-command third-party license inventory. The engine is
`.claude/skills/licenses/licenses.mjs` (zero-dependency; tests in
`licenses.test.mjs`, run by `npm run test:node`); the output is
`THIRD-PARTY-LICENSES.txt` at the repo root, committed, and shipped next to
`WikiLens.exe` through `tauri.conf.json`'s `bundle.resources` (plan:
`vault/2026-08-26_open-source-release-0-2-0.md`).

## How to run
From anywhere in the repo (the script resolves the repo relative to itself):

```
npm run licenses              # regenerate; writes only when the content changed
npm run licenses -- --check   # regenerate in memory; exit 1 if the committed file is stale
```

Needs **cargo-about 0.9.x** once per machine (CI installs it per run):

```
cargo install cargo-about --locked --version 0.9.2 --features cli
```

The script prints `licenses: THIRD-PARTY-LICENSES.txt unchanged|created|updated
(N crates, M packages, K KB)`; `--check` fails with the first differing line
and the fix (`npm run licenses`, commit the result).

## What it does
- **Rust half** — `cargo about generate --format json --locked --fail` in
  `src-tauri/`, configured by `src-tauri/about.toml`: the
  `x86_64-pc-windows-msvc` graph, normal dependencies only, the app crate
  excluded (`publish = false` in `Cargo.toml` + `private.ignore`), and a
  **priority** `accepted` list — an `A OR B` crate contributes the first
  accepted alternative, so MIT ranks first. A crate under a license not in
  the list fails the run; add it deliberately.
- **npm half** — a walk of `package-lock.json`'s `packages` map (production
  entries only) reading each `node_modules/<pkg>/LICENSE*`; a package that
  ships none fails the run — map it in `NPM_LICENSE_OVERRIDES` with a
  reviewed file (currently only `@tauri-apps/plugin-opener`, which ships an
  SPDX document instead of its MIT text).
- Both halves are sorted with a code-point comparator, normalized to LF, and
  rendered without dates, versions or machine paths, so the file is a pure
  function of the two lockfiles and CI can byte-compare it.

## Expected warnings (not failures)
cargo-about prints `unable to find text for license … falling back to
canonical text` for the ~10 crates that ship no license file (the
`webview2-com*` trio, the `unic-*` family, `alloc-stdlib`, `selectors`) —
the canonical SPDX text is used. Only a non-zero exit is a failure.

## Invariants
- The release PR stays bump-only: the app crate is excluded, so `npm run bump`
  never stales the file. If `--check` fails on a bump PR, the exclusion broke.
- `THIRD-PARTY-LICENSES.txt` is pinned `text eol=lf` in `.gitattributes`; the
  script writes LF regardless of `core.autocrlf`.
- Upgrading cargo-about (CI pins `0.9.2`) can change canonical texts: bump the
  pin and regenerate in the same PR.
