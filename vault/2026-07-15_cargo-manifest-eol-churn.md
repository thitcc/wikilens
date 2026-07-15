---
title: Cargo.toml shows phantom-modified after every tauri dev (EOL churn)
type: fix
status: done
created: 2026-07-15
updated: 2026-07-15
tags: [build, tauri]
related: ["[[2026-07-13_parser-snapshot-property-tests]]"]
commit: 8e0a097
---

# Cargo.toml shows phantom-modified after every tauri dev (EOL churn)

## Context / problem

Every `npm run tauri dev` run left `src-tauri/Cargo.toml` flagged as modified
in source control, but opening it showed no changes, and discarding the change
only lasted until the next dev run.

## Diagnosis

- Git for Windows ships `core.autocrlf=true` (system gitconfig), so tracked
  files check out with CRLF even though the repo stores LF.
- The Tauri v2 CLI parses and rewrites `src-tauri/Cargo.toml` on every
  `tauri dev`/`build` to sync the `tauri` dependency's Cargo features with
  `tauri.conf.json` — and writes it back with LF endings.
- The on-disk file then no longer matches what checkout would produce, so git
  flags it modified even though the content is byte-identical after conversion
  (`git diff` is empty; `git ls-files --eol` showed `i/lf w/lf` under
  autocrlf). Editors show no diff because only the line terminators differ.
- Same failure class as the `*.snap` churn fixed in
  [[2026-07-13_parser-snapshot-property-tests]] — and `tauri.conf.json` was
  already sitting at LF in the working tree, one stat-refresh away from the
  same symptom. `Cargo.lock` is latent too (cargo writes LF when it updates
  the lockfile).

## Fix

Pin the toolchain-rewritten files to LF in `.gitattributes`
(`src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`)
and run `git add --renormalize .` once to refresh the index's conversion
state — the stored blobs were already LF, so nothing but `.gitattributes`
changed.

Verified in a throwaway clone before applying: fresh checkout → CRLF; LF
rewrite reproduced the phantom flag; with the pins the status stayed clean
through repeated LF rewrites and `git checkout -f` restores, and the file now
checks out as LF (matching what the Tauri CLI writes).

## Status log

- 2026-07-15 — diagnosed, fix verified in scratch clone, landed.
