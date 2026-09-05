# Contributing

Thanks for looking. WikiLens is a solo, spare-time project with strong
conventions; the short version is below, the long version is `CLAUDE.md`.

## Set up

Windows 10/11, Node.js 22, a stable Rust toolchain, the Visual Studio C++
Build Tools (workload *Desktop development with C++*, which the MSVC target
links with — see [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/#windows)),
and the WebView2 runtime.

```
npm install
npm run tauri dev
```

The panel starts hidden in the tray — press **Ctrl+`** over a borderless or
windowed game. `WIKILENS_DEBUG=1` prints a per-ask table and creates a debug
panel that also starts hidden — show it with the footer **Debug** chip or the
tray's "Show debug panel"; `.env.example` lists every variable a dev build
reads.

## Verify before you push

The same gates CI runs on every PR (`.github/workflows/ci.yml`):

```
npx tsc --noEmit                                   # frontend types
npm test                                           # Vitest (src/**)
npm run test:node                                  # tooling + eval-mirror parity tests
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

`cargo test -- --ignored` (from `src-tauri/`) runs the live wiki/API suites —
needed when you touch a game or provider. Retrieval changes are measured with
the eval suite in `eval/` (see `eval/README.md`) before and after.

## How changes land

- Branch off `main` as `<type>/<slug>` (`feat/…`, `fix/…`, `docs/…`,
  `chore/…`); commits use conventional prefixes (`feat:`, `fix:`, `docs:`,
  `chore:`, `ci:`, `docs(vault):`).
- Every change is a pull request; CI must be green. The maintainer merges,
  always with a **merge commit** — never squash or rebase — because the
  planning docs under `vault/` pin commit hashes from PR branches.
- PR title: an imperative, sentence-case headline of the outcome, no `type:`
  prefix. PR body: the template in `.github/PULL_REQUEST_TEMPLATE.md` — a
  plain-language summary, behavior-level change bullets, a behavior-changes
  section, and a verification table.
- Versions bump only in release PRs (`npm run bump X.Y.Z`); GitHub Releases are
  the changelog. Release walk: `docs/release.md`.

## Adding a built-in game

One `GameWiki` entry in `src-tauri/src/wiki/games.rs` plus a golden query in
`GOLDEN_CASES` (`src-tauri/src/wiki/mod.rs` — an offline test fails without
one; verify it live with `cargo test golden -- --ignored`). Verify the API
endpoint first, derive `page_url` from the wiki's real `articlepath`, set
`search_namespace` for shared wikis, and prefer official or independent wikis
over stale Fandom copies. A foreground-detection rule (`detect/rules.rs`) is
optional and separate — add one only for an executable name observed on a real
install. Full rules: `CLAUDE.md` §4.

Players can also add any MediaWiki in-app (game menu → *Add a game…*), so a
built-in entry is for games worth a verified search setup and detection.

## How this project is built

The codebase is developed with AI coding agents under human review, and the
repo carries that workflow in the open:

- `CLAUDE.md` — the canonical project guide the agents (and you) read:
  architecture map, conventions, gotchas. It is written for an agent audience,
  so it is dense; this file is the human on-ramp.
- `vault/` — the planning vault: plans, decisions (ADRs), research and retros
  as dated Markdown with YAML frontmatter, linted by `.claude/skills/vault-lint`.
  Written in-session as work happens; "the owner" in those notes is the sole
  maintainer. Open the folder in Obsidian for the dashboards, or just `grep`.
- `.claude/` — slash commands and skills (the version bump, the vault linter,
  the release-build notes); `.codex/config.toml` points a second agent at the
  same `CLAUDE.md`; `.impeccable/` and `DESIGN.md` hold the design tokens.

None of that is required to build or run the app.
