# Release runbook

How a WikiLens release actually ships, command by command. The strategy
(what the version segments mean, when a release is worth cutting) lives in
`docs/versioning.html`; this doc is the walk itself, with the same six
step numbers as the guide's ritual. The agent preps step 2 and CI builds
step 5's installers; every other step is the owner gating. (A visual
version of this page lives at `docs/release.html`.)

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

Tagging mutates `main`, so it stays owner-side, same line as merging. The
push is also the trigger: step 5's build starts the moment the tag lands.

## 5. CI builds, you validate — CI + owner

The tag push triggers `.github/workflows/release.yml`
(vault/2026-08-04_tag-triggered-release-ci.md): a `windows-latest` runner
builds the installers with `tauri-apps/tauri-action` and attaches them to
a **draft** GitHub Release named after the tag. A draft is invisible to
everyone but collaborators — nothing is published yet. The workflow first
checks the tag name against `package.json`'s version and fails in seconds
on a mismatch — it catches a tag whose number was never bumped, not a
correctly-numbered tag placed on a later commit, so tag the release PR's
merge commit (`git log --first-parent -1` before `git tag`); a cold build
then takes 15–25 minutes.

When the run goes green, download both installers from the draft
(GitHub → Releases → WikiLens v0.2.0):

| Artifact | Asset name |
|---|---|
| NSIS installer (bootstraps WebView2) | `WikiLens_0.2.0_x64-setup.exe` |
| MSI installer | `WikiLens_0.2.0_x64_en-US.msi` |

The smoked bytes are now the shipped bytes — validate the downloads, not
a local build. A local `npm run tauri build` still works and is the
fallback when CI is down; artifact paths and packaged-runtime caveats
(no console, `.env` stops working, keys unaffected) are in
`.claude/skills/release-build/SKILL.md`.

Then the real gate; the version is a promise that it ran:

- Walk `docs/smoke-checklist.md`: item 11 runs on the **downloaded**
  installer, not the dev build. Tick the boxes as you go; they get
  pasted into the release notes.
- Run the live wiki/API suites: `cargo test -- --ignored` from
  `src-tauri/`.

## 6. Publish the draft — owner

Paste the notes over CI's placeholder body (the web UI's edit view is
easiest), then publish:

```
gh release edit v0.2.0 --draft=false
```

Notes = behavior bullets from `git log --first-parent v0.1.0..v0.2.0`
(the PRs merged since the last release; their titles are already
behavior-level) + the ticked smoke-checklist boxes. The installers are
already attached — nothing to upload. GitHub Releases are the **only**
changelog; there is no `CHANGELOG.md`.

## When it goes sideways

- **`npm run bump` refuses or fails** — it names the cause (mismatched
  manifests, malformed version, failed lock refresh). A refresh that
  errors out restores the manifests; fix the environment and re-run. Check
  `git status` for a half-refreshed lock.
- **The release workflow fails** — re-run it from the Actions tab (the
  tag never re-pushes; re-running the failed run is the retry). If a
  re-run leaves a stray duplicate draft, delete the stale one. If CI
  itself is the problem, the old manual path is the fallback: local
  `npm run tauri build`, smoke the local installers, then
  `gh release upload v0.2.0 <nsis> <msi>` onto the draft (or
  `gh release create` if no draft exists).
- **Smoke fails after tagging** — the tag stays a tag: delete the
  unsmoked draft (`gh release delete v0.2.0` — a draft was never
  visible, so nothing observable disappears; the tag itself stays), fix
  on `main` through normal PR flow, then cut the *next* number. Never
  move, reuse, or delete a pushed tag, and never *publish* a Release for
  a version that didn't pass the walk.
- **Skip the "Create release from tag" shortcut** — a *published*
  Release object claims installers were built and smoked at that commit;
  `v0.1.0` is tag-only for exactly this reason. CI's **draft** keeps the
  rule's spirit: it stays invisible until you publish it, so the claim
  is still made by a human, at publish time, after the walk.

## Pointers

- `docs/versioning.html` — the strategy: segment semantics, when to cut,
  house rules.
- `.github/workflows/release.yml` — the tag-triggered build: installers
  onto a draft Release on every `v*` push
  (vault/2026-08-04_tag-triggered-release-ci.md).
- `vault/2026-08-04_release-scoped-semver.md` — the decision record with
  the rejected alternatives.
- `.claude/skills/release-build/SKILL.md` — build mechanics and
  packaged-runtime caveats.
- `docs/smoke-checklist.md` — the manual validation walk.
- CLAUDE.md §3 · Versioning — the one-bullet summary agents read every
  session.
