# Release runbook

How a WikiLens release actually ships, command by command. The strategy
(what the version segments mean, when a release is worth cutting) lives in
`docs/versioning.html`; this doc is the walk itself, with the same six
step numbers as the guide's ritual. The agent preps step 2; every other
step is the owner gating. (A visual version of this page lives at
`docs/release.html`.)

## 1. Pick the number — owner

A feature batch since the last release → minor (`0.2.0`); fixes only →
patch (`0.1.1`). The full segment semantics are in the versioning guide.
Before going further: `main` is green in CI and your working tree is
clean; the release PR must contain nothing but the bump.

## 2. Release PR — agent

Ask the agent to "prep the release PR for 0.2.0". Concretely it runs:

```
git checkout -b chore/release-0-2-0
npm run bump 0.2.0
```

`npm run bump` rewrites the version in the three manifests
(`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`),
refreshes both lockfiles (`npm install --package-lock-only` +
`cargo check`), and prints an old → new line per file. It refuses
manifests that disagree on the current version, a malformed version, and
bumping to the version already there; if a refresh command errors out, it
restores the manifests, so a retry after fixing the environment starts
clean.

The agent then runs the verification bundle (`/check`: type-check,
frontend tests, tooling tests, clippy, Rust tests) and opens the PR: one
commit, branch `chore/release-0-2-0`. The PR summary doubles as draft
release notes.

## 3. Merge — owner

Merge commit, as always — never squash (vault docs pin commit hashes).

## 4. Tag the merge commit — owner

From an up-to-date `main` checkout, so the annotated tag lands on the
release PR's merge commit:

```
git checkout main
git pull
git tag -a v0.2.0 -m "WikiLens 0.2.0"
git push origin v0.2.0
```

Tagging mutates `main`, so it stays owner-side, same line as merging.

## 5. Build and validate — owner

```
npm run tauri build
```

Artifacts land gitignored under `src-tauri/target/`:

| Artifact | Path |
|---|---|
| Standalone exe (assumes WebView2) | `src-tauri/target/release/wikilens.exe` |
| NSIS installer (bootstraps WebView2) | `src-tauri/target/release/bundle/nsis/WikiLens_0.2.0_x64-setup.exe` |
| MSI installer | `src-tauri/target/release/bundle/msi/WikiLens_0.2.0_x64_en-US.msi` |

The first-ever bundle downloads the WiX + NSIS toolchains (needs network
once); a cold Rust build is the slow part. Hand people an **installer**;
the raw exe is for local use. Packaged-runtime caveats (no console, `.env`
stops working, keys unaffected) are in
`.claude/skills/release-build/SKILL.md`.

Then the real gate; the version is a promise that it ran:

- Walk `docs/smoke-checklist.md`: item 10 runs on the packaged
  **installer**, not the dev build. Tick the boxes as you go; they get
  pasted into the release notes.
- Run the live wiki/API suites: `cargo test -- --ignored` from
  `src-tauri/`.

## 6. GitHub Release — owner

```
gh release create v0.2.0 src-tauri/target/release/bundle/nsis/WikiLens_0.2.0_x64-setup.exe src-tauri/target/release/bundle/msi/WikiLens_0.2.0_x64_en-US.msi
```

Notes = behavior bullets from `git log --first-parent v0.1.0..v0.2.0`
(the PRs merged since the last release; their titles are already
behavior-level) + the ticked smoke-checklist boxes. GitHub Releases are
the **only** changelog; there is no `CHANGELOG.md`.

## When it goes sideways

- **`npm run bump` refuses or fails** — it names the cause (mismatched
  manifests, malformed version, failed lock refresh). A refresh that
  errors out restores the manifests; fix the environment and re-run. Check
  `git status` for a half-refreshed lock.
- **Smoke fails after tagging** — the tag stays a tag: fix on `main`
  through normal PR flow, then cut the *next* number. Never move, reuse,
  or delete a pushed tag, and never create a Release object for a version
  that didn't pass the walk.
- **Skip the "Create release from tag" shortcut** — a Release object
  claims installers were built and smoked at that commit; `v0.1.0` is
  tag-only for exactly this reason.

## Pointers

- `docs/versioning.html` — the strategy: segment semantics, when to cut,
  house rules.
- `vault/2026-08-04_release-scoped-semver.md` — the decision record with
  the rejected alternatives.
- `.claude/skills/release-build/SKILL.md` — build mechanics and
  packaged-runtime caveats.
- `docs/smoke-checklist.md` — the manual validation walk.
- CLAUDE.md §3 · Versioning — the one-bullet summary agents read every
  session.
