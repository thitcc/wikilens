---
title: Open-source prep and the 0.2.0 release
type: plan
status: active
created: 2026-08-26
updated: 2026-09-04
tags: [build, security, vault]
related:
  - "[[2026-08-26_mit-license]]"
  - "[[2026-08-26_product-rename]]"
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-08-04_versioning-adoption]]"
  - "[[2026-08-04_tag-triggered-release-ci]]"
  - "[[2026-08-15_security-review]]"
  - "[[2026-08-24_local-ai-mode]]"
  - "[[2026-08-05_windows-code-signing]]"
commit:
---

# Open-source prep and the 0.2.0 release

## Context / problem
The repo goes public at 0.2.0 — decided when Local AI replaced the
env-configured Default mode ([[2026-08-24_local-ai-mode]] non-goals,
[[2026-08-24_replace-default-mode-with-local-ai]] follow-ups). Nothing
about the repo was built for strangers yet: no license, no community files,
a README that describes 3 of the 15 built-in games and the retrieval
pipeline backwards, installers that name their publisher "wikilens" with an
empty copyright, and a bundle identifier / OpenRouter attribution pointing
at `wikilens.app` — a live third-party product of the same name that the
owner does not control.

Before planning, a read-only audit ran over the whole history and tree
(5 finders → adversarial verifiers per finding → completeness critic, 31
agents, HEAD `51a0d4e`). Its verdict, skeptic-verified against primary
sources:

- **No secrets, ever.** All 415 commits on every ref, merge diffs, deleted
  blobs, tag and commit messages, PR bodies, PR review comments and issue
  comments: zero token-shaped strings (Anthropic / OpenRouter / GitHub / AWS
  / Google / JWT / PEM / webhooks), no real e-mail in file content, no DPAPI
  ciphertext, no key-store or settings dumps, no screenshots. `.env` was
  never committed. Largest blob in history: 214 KB (a wiki fixture).
  `npm audit` and `cargo audit` on the current CI run: 0 vulnerabilities
  (cargo's 18 warnings are all the unmaintained/unsound class the gate
  already allows).
- **Residuals, accepted without a history rewrite:** the author e-mail on
  every commit (normal for open source) and one real profile path in a test
  fixture (`src-tauri/src/detect/mod.rs:186`, in history since `c00bd9e`).
  A rewrite would orphan every vault `commit:` pin and both tags — the same
  reason squash merges are banned — so the fixture is fixed in-tree and the
  history stays.
- **Remote is clean:** `origin` holds exactly one branch; the ~70
  `origin/*` refs seen locally are stale tracking refs.
- **Known Tier-2 hardening items** from [[2026-08-15_security-review]] are
  all still open at HEAD (unguarded release `.env` load, un-wiped DPAPI
  plaintext buffer, `history.json.bak` surviving Clear history, an unused
  `global-shortcut` grant, plus the accepted S2/S4/S7/S8), and the review's
  threat model says "private repo" — it goes stale at the flip.
- **Blockers for going public:** no `LICENSE`; wiki fixtures under three CC
  licenses, two of them NonCommercial (stardewvalleywiki.com), with no
  root-level third-party notice; `authors = ["you"]`; the `wikilens.app`
  collision; the stale README; no SECURITY/CONTRIBUTING; `ci.yml` running
  with the repo-default *write* token; squash and rebase merges enabled
  against the merge-commit mandate; no repo description or topics.

## Goal / non-goals
- Goal: make the repo publishable — license and notices, honest manifest
  metadata, community files, CI token hygiene, the small in-tree fixes the
  audit found — in PRs that land **before** the release PR, which stays
  bump-only ([[2026-08-04_release-scoped-semver]]).
- Goal: a README written for a stranger landing from GitHub (install from
  the Releases page, unsigned-installer SmartScreen note, the real feature
  set and game list), with the contributor material moved to
  `CONTRIBUTING.md`.
- Goal: land the four cheap Tier-2 hardening fixes the security review
  itself prescribes, so the published review reads as current.
- Goal: cut 0.2.0 as the **first published GitHub Release** (v0.1.0 and
  v0.1.1 are tag-only baselines; the 0.1.1 draft was built by CI and
  deleted unpublished) and flip the repo public after it is published.
- Non-goal: any git history rewrite, re-tagging, or a `CHANGELOG.md`.
- Non-goal: dependency major bumps or `cargo update` in this arc; emptying
  `src-tauri/.cargo/audit.toml` (its recheck trigger has arrived — a scoped
  xcap/xcb/wayland-scanner bump removes both ignored quick-xml versions) is
  its own chore PR after the release.
- Non-goal: the product rename ([[2026-08-26_product-rename]]), a real icon,
  code signing ([[2026-08-05_windows-code-signing]]), an installer EULA page
  (`bundle.licenseFile`), `dependabot.yml`, a code of conduct.
- Non-goal: regenerating the two Stardew fixtures from a BY-SA wiki — the
  root notice carves them out (fork recorded in [[2026-08-26_mit-license]]).

## Approach
Four PRs off `main`, in this order; every one merges by merge commit after
`/check` and CI.

1. **`chore/open-source-prep`** — these vault docs; `LICENSE` (MIT, Thiago
   Tenório); `THIRD-PARTY-NOTICES.md` listing the four wiki fixtures and
   their four derived `.snap` files with source, capture date and license;
   manifest metadata (`Cargo.toml` authors/license/repository,
   `package.json` license/repository, `tauri.conf.json` publisher,
   copyright, homepage, short description, license, category); the interim
   identifier `io.github.thitcc.wikilens` and the OpenRouter `HTTP-Referer`
   pointed at the GitHub repo; `permissions: contents: read` on `ci.yml`;
   the machine-local excludes promoted from `.git/info/exclude` into
   `.gitignore`; the fixture path, `PRODUCT.md` platform, the review's
   time-scoped threat model + status banner, the dangling explainer
   pointer, the 0.1.1 draft's fate in the release-CI log, and the release
   runbook's guard wording and smoke item number; `.github/SECURITY.md` and
   two issue forms (bug report, game request).
2. **`fix/security-review-tier2`** — `#[cfg(debug_assertions)]`-gate the
   `.env` load; zero the DPAPI output buffer before `LocalFree`; delete
   `history.json.bak` in `clear()`; drop the unused `global-shortcut`
   grant and pin its absence in `config_guardrails.rs`; one test per fix;
   flip S3/F6/S6/G1 to resolved in the review's ledger.
3. **`docs/readme-rework`** — the public README (~140 lines: pitch,
   screenshot slot, how it works, requirements, Install + SmartScreen,
   first run, answer sources, using it, the 15 games + add-your-own,
   privacy, build from source, advanced fold, caveats, pointers) and
   `CONTRIBUTING.md` (prerequisites, verification bundle, PR/merge-commit
   rules, conventions, adding a game, the eval suite, and a "how this
   project is built" paragraph framing `CLAUDE.md`, `.claude/`, `.codex/`,
   `vault/`, `.impeccable/`).
4. **`chore/release-0-2-0`** — `npm run bump 0.2.0`, nothing else; the PR
   body doubles as the release notes covering `v0.1.0..v0.2.0`.

Owner-side, after PR 4: tag the release PR's merge commit, smoke the
downloaded installers (item 11; Add/Remove Programs must read the new
publisher), `cargo test -- --ignored`, publish the draft; then, in one
sitting: description + topics, disable squash and rebase merges, Actions
default token → read, flip public, enable secret scanning + push
protection, Dependabot alerts (alerts only — the documented no-bot stance
stays), private vulnerability reporting, a ruleset on `main`. Copy the
dev machine's `%APPDATA%\com.wikilens.app` to the new identifier's folder
once (DPAPI is per-user; the keys survive the copy) — and
`%LOCALAPPDATA%\com.wikilens.app` too: Tauri keeps the WebView2 profile
under `LocalData/<identifier>`, and that is where the panel's remembered
picks (game, recents, model, theme — `localStorage`) live; skip it and they
reset. The `game` label the game-request issue form names already exists
(created 2026-09-04; templates don't create labels). Before smoking the
0.2.0 installer, **uninstall the 0.1.x dev install by hand**: the NSIS
template stores the install dir under `Software\<publisher>\WikiLens`, the
publisher string changed with this PR, so an NSIS-over-NSIS upgrade reads
an empty path and may abort instead of replacing the old copy.

## Decisions & trade-offs
- License: MIT — [[2026-08-26_mit-license]] (dependency tree verified
  compatible: 602 crates, 218 npm packages, zero copyleft).
- No history rewrite for the e-mail and the fixture path — the vault's
  `commit:` pins and the tags outrank a cosmetic scrub.
- Identifier and Referer change now, product name later: there is no public
  install base yet, so the identifier is free to move today; the rename that
  resolves the collision properly is its own arc.
- The four hardening fixes ride as their own PR so the user can drop it
  without touching the legal/metadata PR.
- Flip after the Release is published, not before: the README's Install
  section must link to installers that exist on day one.

## Status log
- 2026-08-26 — created after the read-only audit; decisions taken with the
  owner (MIT; interim identifier `io.github.thitcc.wikilens` + GitHub
  Referer; publisher/copyright "Thiago Tenório"; flip after v0.2.0 is
  published). PR 1 opened from this doc; its code commit is `9145706`.
- 2026-09-04 — an external review of the four PRs was verified claim by
  claim: #81's clear-history ordering fixed (`.bak` first), MSVC Build
  Tools added to the build prerequisites, the UAC wording dropped from
  PR 1's body, the `game` label created; the owner step gained the
  `%LOCALAPPDATA%` profile move and the 0.1.x manual uninstall. The
  missing third-party license texts became their own follow-up branch.
