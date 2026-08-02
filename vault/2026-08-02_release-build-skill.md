---
title: Capture the packaged-build knowledge as a release-build skill
type: plan
status: done
created: 2026-08-02
updated: 2026-08-02
tags: [build]
related: ["[[2026-07-13_pr-delivery-workflow]]", "[[2026-07-13_manual-smoke-checklist-live-cadence]]"]
commit: c3ba1ad
---

# Capture the packaged-build knowledge as a release-build skill

## Context / problem
The app has only ever been run via `npm run tauri dev`, so "make it an
executable" required rediscovering the packaging story from scratch: where
`npm run tauri build` puts the standalone exe vs the installers, that release
binaries drop the console window and start hidden in the tray (a fresh launch
looks like nothing happened), and that `.env` stops working once the app is no
longer launched from the repo. None of that was written down anywhere an agent
would find it at build time — CLAUDE.md §3 has one line ("Release:
`npm run tauri build`, then walk the manual smoke checklist").

## Goal / non-goals
- Goal: a `release-build` skill under `.claude/skills/` that captures the
  command, the artifact paths, the packaged-runtime caveats, and the post-build
  verification step, so future build/package requests don't re-derive them.
- Non-goal: changing the build itself (config, targets, versioning, CI
  packaging) — the skill documents what exists.

## Approach
1. Run `npm run tauri build` and verify the real artifact paths and names
   rather than asserting them from Tauri docs.
2. Write `.claude/skills/release-build/SKILL.md` in the same shape as
   `vault-lint`/`sync-agents` (frontmatter with `name`/`description`/
   `allowed-tools`, "How to run" / "When to run" sections).
3. Regenerate the Codex adapters (`node .claude/skills/sync-agents/generate.mjs`)
   since `.claude/` changed; run `/check`'s gates and `/vault-lint`; land via PR.

## Decisions & trade-offs
- Doc-only skill (no engine script): the build is one npm command; the value is
  the paths and caveats around it, not automation. Keeps `test:node` untouched.
- The skill points at README's "Default mode" and "Retrieval tuning" tables for
  the env-var contracts instead of re-listing them — same single-source rule
  CLAUDE.md follows.

## Status log
- 2026-08-02 — created; build running, skill drafted alongside.
- 2026-08-02 — done. Build verified (exe 12.7 MB, NSIS 2.9 MB, MSI 4.4 MB;
  2m33s Rust compile; WiX/NSIS toolchains downloaded on first bundle), skill
  landed in c3ba1ad, adapters regenerated, all gates green.
