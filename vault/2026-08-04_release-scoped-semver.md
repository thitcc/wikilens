---
title: Versions are release-scoped SemVer, bumped in release PRs
type: decision
status: active
created: 2026-08-04
updated: 2026-08-04
tags: [build]
related:
  - "[[2026-08-04_versioning-adoption]]"
  - "[[2026-07-13_pr-delivery-workflow]]"
  - "[[2026-08-02_release-build-skill]]"
  - "[[2026-07-13_ci-pipeline-github-actions]]"
commit:
---

# Versions are release-scoped SemVer, bumped in release PRs

## Context

Every version-bearing file already reads `0.1.0` — `package.json`,
`package-lock.json` (two spots), `src-tauri/Cargo.toml`, `Cargo.lock`, and
`src-tauri/tauri.conf.json`, whose explicit `version` names the installers
(`WikiLens_<version>_x64-setup.exe`). Nothing pins that number: 56 PRs / 288
commits in, the repo has zero tags and zero GitHub Releases. A release is
manual and expensive (`npm run tauri build`, the smoke checklist, the
`--ignored` live suites), delivery is PR-only with a human merging (merge
commits, never squash), and authored commits are ~100% conventional-prefix.
The triggering question: should every PR bump the version?

## Decision

WikiLens versions are SemVer-shaped with application semantics, bumped only
in release PRs — a version names a validated installer, never a merge.

- Segments read app-flavored, not library-flavored: **MAJOR** breaks the
  installed world (app-data `settings.json` / `keys.json` / `history.json` /
  `wikis.json` incompatible without migration; stays 0 pre-stable),
  **MINOR** is a feature release (the 0.x workhorse), **PATCH** is a
  fix-only release. 1.0.0 when app-data compatibility across upgrades is a
  kept promise and the installer is stranger-ready.
- Release notes live in GitHub Releases only — no `CHANGELOG.md`. PR titles
  are already behavior-level, and `git log --first-parent vPREV..vNEXT`
  reconstructs any release from them.
- v0.1.0 is adopted as an annotated tag only — no Release object, no
  artifacts (attaching installers would imply the full ritual ran).
- The release ritual: ① pick the number (feature → minor, fixes → patch)
  ② agent opens the release PR (`chore/release-0-2-0`: bump the three
  manifests, refresh locks via `npm install --package-lock-only` +
  `cargo check`, `/check`) ③ human merges ④ human tags the merge commit
  (`git tag -a v0.2.0` + push — tagging mutates `main`, so it stays
  human-side) ⑤ build + smoke checklist + live suites ⑥ `gh release create`
  with the NSIS/MSI installers and notes.

## Consequences

- Good: the number is a promise about a validated artifact; concurrent
  branches never fight over the same five version lines; nothing is
  hand-maintained between releases.
- Cost / bad: the version doesn't advance with merges (56 merged PRs still
  read 0.1.0); a release costs one extra PR; the ritual stays manual.
- Follow-ups: [[2026-08-04_versioning-adoption]] (tag v0.1.0 + docs), then
  the idea docs — [[2026-08-04_version-in-settings]],
  [[2026-08-04_release-bump-script]],
  [[2026-08-04_tag-triggered-release-ci]], [[2026-08-04_auto-updater]].

## Alternatives considered

- **Per-PR bump** — 56 PRs and zero releases in, the counter would already
  read v0.56.0: a merge count, not a promise. Every bump edits the same five
  lines in five files, so any two open branches conflict on merge. Rejected:
  the version's job is naming a validated installer.
- **Commit-driven automation (release-please / changesets)** — the commit
  discipline qualifies, but the expensive parts of a WikiLens release (smoke
  checklist, live suites) are irreducibly manual, and it adds a bot to a
  repo whose last gate is deliberately human. Rejected for now; revisit at
  >2 releases/month or a second contributor.
- **CalVer** — a date can't express 0.x pre-stable maturity; `2026.08` reads
  as a done-ness the app hasn't earned. Rejected.
- **`CHANGELOG.md`** — a third hand-maintained copy of what PR titles and
  `git log --first-parent` already record, and a merge-conflict magnet.
  Rejected: GitHub Releases hold the notes.
