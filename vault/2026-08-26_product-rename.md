---
title: Rename the product away from the wikilens.app collision
type: plan
status: todo
created: 2026-08-26
updated: 2026-09-08
tags: [build, overlay]
related:
  - "[[2026-08-26_open-source-release-0-2-0]]"
  - "[[2026-08-04_release-scoped-semver]]"
commit:
---

# Rename the product away from the wikilens.app collision

## Context / problem
`wikilens.app` is a live, unrelated product (a German-language SPA, online
since at least 2026-03) and the domain is not the owner's. The scaffold
picked `com.wikilens.app` as the Tauri identifier and `https://wikilens.app`
as the OpenRouter `HTTP-Referer` on the assumption the name was free. The
open-source prep ([[2026-08-26_open-source-release-0-2-0]]) moves both to
neutral values — identifier `io.github.thitcc.wikilens`, Referer
`https://github.com/thitcc/wikilens` — but the product name "WikiLens"
itself still collides, and the owner intends to rename after 0.2.0 ships.

## Goal / non-goals
- Goal: pick a name that is free as a domain, a GitHub repo, and a Windows
  product name; apply it everywhere the current name is load-bearing.
- Non-goal: doing it inside the 0.2.0 release arc.

## Approach
Rename **before a public install base exists** — the release right after
0.2.0 — because three things key on the name and are painful to move once
installed: the Tauri identifier (the app-data dir), the NSIS/MSI upgrade
identity (`productName` + `bundle.publisher`), and the GitHub repo URL.
The rename ships in **0.3.0** together with pt-BR support (a separate plan
doc when that work starts; only named here). Under
[[2026-08-04_release-scoped-semver]] an identifier change is MAJOR-class
unless a migration copies the app-data folder, so 0.3.0 commits this arc to
the migration. Per the release-scoped rule the rename PR does **not** bump
the version; the 0.3.0 release PR does, after both arcs have merged.

"Replace every `wikilens`" is not the rule — the 2026-09-08 inventory (111
files outside the vault, 52 inside) falls into four groups:

### 1. Rename outright (product-facing, no compatibility concern)
- `productName`, the three window titles (`tauri.conf.json`), the three
  html `<title>`s, the header brand (`App.tsx`), the debug window's brand,
  the tray tooltip (`tray.rs`), the OpenRouter `X-Title` header
  (`providers.rs`, `llm.rs`), `releaseName` in `release.yml`.
- User-facing copy in Rust (~25 strings: "restart WikiLens and try again",
  "This wiki refused WikiLens's request", "WikiLens already includes
  {name}"…) and in the frontend (slow-wiki hint, first-summon placeholder,
  the two thinking-badge titles in `ModelMenu.tsx`).
- stderr prefixes `[wikilens]` / `wikilens:` and the table/trace titles
  `wikilens.debug`, `wikilens.rewrite`, `wikilens.retrieval` — cosmetic,
  renamed for consistency (README documents the debug table by env var, not
  by title).
- Docs: README, CLAUDE.md (§1, §2 tree, §4/§5 prose), DESIGN.md, PRODUCT.md,
  CONTRIBUTING, `docs/` (smoke checklist, release runbook, the html
  explainers), the issue templates, the PR template, `.github/SECURITY.md`,
  the skills under `.claude/` (bump, licenses, release-build, vault-lint,
  proof-sheet skeleton, the commands) — then `node
  .claude/skills/sync-agents/generate.mjs` regenerates `.agents/`.
- `THIRD-PARTY-LICENSES.txt` header text lives in `licenses.mjs`
  (`APP_CRATE`, the header lines, the tmp-file name): edit the script, run
  `npm run licenses`, never hand-edit the txt. `about.toml` comments too.

### 2. Rename with a migration or a compatibility layer
- **Identifier** `io.github.thitcc.wikilens` → `io.github.thitcc.loreno`:
  the app-data dir moves. First launch of loreno copies `settings.json`,
  `keys.json`, `history.json`, `wikis.json` from the old dir when the new
  one is empty (DPAPI is per-user, so the ciphertexts stay valid); never
  deletes the old dir; a copy failure logs and starts empty (the stores'
  read-only-this-session pattern). Pin in `config_guardrails.rs`: the new
  identifier, and that the migration reads only those four filenames.
- **Installer identity**: a new `productName` means the WikiLens NSIS/MSI
  entry is not upgraded in place — both would coexist. The 0.3.0 release
  notes and `docs/release.md` say "uninstall WikiLens first"; the smoke
  checklist gains an upgrade-from-0.2.0 item (old app-data found and copied,
  old entry removed by hand, hotkeys/keys/history intact).
- **localStorage** `wikilens.selectedGame`, `.recentGames`,
  `.selectedProvider`, `.selectedModel.<id>`, `.theme` → `loreno.*`, with
  a one-time read-old-write-new at boot (`modelPick.ts`, `theme.ts`,
  `App.tsx`; unit-tested like the existing storage tests). Cheap, so no
  reset.
- **Env vars the app reads** (`WIKILENS_DEBUG`, `_TRACE_RETRIEVAL`,
  `_QUERY_REWRITE`, `_TITLE_INDEX`, `WIKILENS_<PROVIDER>_MODEL`) →
  `LORENO_*`, reading the old spelling as an alias for one release and
  naming it in `target::warn_legacy_env` ("WIKILENS_DEBUG still works this
  release; use LORENO_DEBUG"). The eval-owned `WIKILENS_EVAL_*` family and
  the `.env.example` comments rename freely (dev-only, no install base).
- **Crate** `wikilens` / `wikilens_lib` (`Cargo.toml`, `main.rs`,
  `APP_CRATE` in `licenses.mjs`, the `cargo test -p wikilens` comment in
  `wiki/mod.rs`), the npm `name`, the ask-cancelled sentinel
  `wikilens::ask-cancelled` (mirrored in `api.ts` and `commands.rs` — both
  or neither), the debug-window label strings.
- **User-Agent** `wikilens/0.1 (game overlay; contact: none)` →
  `loreno/<version> (game overlay; contact: none)` in `wiki/mod.rs` and its
  eval mirror `eval/lib.mjs` (parity-tested); fixes the stale "0.1" while at
  it. MediaWiki etiquette wants the UA to identify the bot, so this is not
  optional.
- **URLs**: `HTTP-Referer`, `homepage` (`tauri.conf.json`), `repository`
  (`Cargo.toml`, `package.json`), the CI badge and every README link →
  `https://github.com/thitcc/loreno`, after the repo rename (GitHub
  redirects the old URL; `git remote set-url` locally).

### 3. Leave alone
- The vault: filenames are permanent IDs and the docs are history (52 files);
  this doc describes the rename instead. Tags `v0.1.0`–`v0.2.0`, the
  published Releases and git history stay as they are.
- `target.rs`'s legacy-env list (`WIKILENS_DEFAULT_*`,
  `WIKILENS_REWRITE_*`): these are the *old* variable names the startup
  notice warns about — they must keep the old spelling.
- Mentions of `wikilens.app` that explain the collision.
- Test fixtures and sentinels (`"wikilens - Visual Studio Code"` in
  `detect/mod.rs`, `WIKILENS-SENTINEL-NOT-A-REAL-KEY`) — optional.
- Local, gitignored: `.env`, `eval/out/*.log`, `.claude/settings.local.json`
  (the owner updates by hand if they care).
- `src-tauri/gen/schemas/*` is generated on build.

### 4. Outside the repo (owner)
- Rename the GitHub repo `thitcc/wikilens` → `thitcc/loreno` (redirect
  follows; the local remote and the URLs in group 2 change after).
- Register `loreno.app` (free 2026-09-08); `loreno.gg` optional.
- Rename the Claude Design project "WikiLens" and its cards on the next
  mirror true-up ([[2026-09-05_design-mirror-true-up-2]] describes the
  ritual); the brand string in every card changes.
- Optionally rename the local checkout folder (it also changes the agent's
  auto-memory directory key — copy the memory dir if so).

### Order of work
1. Repo rename on GitHub + `loreno.app` registration (owner, first — the
   URLs in the code depend on it).
2. One code PR, branch `feat/rename-loreno`: groups 1 and 2, the app-data
   migration with its pin, the localStorage migration with tests, the env
   aliases, `npm run licenses`, sync-agents, docs. `/check` +
   `/vault-lint`. No version bump.
3. The Claude Design mirror true-up (outside the repo, after merge).
4. pt-BR support lands as its own arc and plan doc.
5. The 0.3.0 release PR (`npm run bump 0.3.0`), release notes with the
   uninstall-WikiLens step, smoke incl. the upgrade item, live suites,
   publish.

## Decisions & trade-offs
- **Version 0.3.0** (owner, 2026-09-08), shared with the pt-BR support arc.
  Choosing MINOR over MAJOR makes the app-data migration mandatory (the
  semver ADR's rule), so "accept a reset" is off the table; the migration
  copies the four store files at first launch and never deletes the old dir.
- Env aliases for exactly one release (0.3.x); the 0.4.0 notes drop the
  `WIKILENS_*` spellings and the startup notice starts naming them as
  removed, the way `WIKILENS_DEFAULT_*` is handled today.
- localStorage migrates (one boot-time read) rather than resets: the game
  pick, recents and model picks are the whole "it remembers me" of the app.
- **Name: loreno** (owner's pick, 2026-09-08, proof sheet round 5). Lowercase
  in the Default theme; the Micrographics instrument rule keeps uppercasing
  the brand (`LORENO`), decided on round 4. Slug `loreno` everywhere:
  identifier `io.github.thitcc.loreno`, env prefix `LORENO_`, crate
  `loreno_lib`, localStorage `loreno.*`, product name / exe `loreno`.
- Availability at pick time: loreno.app and loreno.gg free (RDAP / DNS);
  loreno.com registered but parked for sale at GoDaddy (the word is otherwise
  a given name / surname — a music producer and a clothing shop use it, no
  software); GitHub user @loreno taken, 7 personal repos with that name (0–1
  stars, no descriptions); crates.io, npm and winget free. The .com requirement
  was relaxed to .app on round 5. No trademark search was done — owner's
  step before the rename ships.

## Status log
- 2026-08-26 — created as an idea during the open-source prep; the interim
  identifier/Referer change ships in that PR.
- 2026-09-06 — name search rounds 1–3 on one proof sheet
  (`.impeccable/critique/product-name-sheet.html`, artifact): game-metaphor
  coinages (lorewisp, lorepeek, runelet…), movement compounds (overskim,
  flitback…) and lowercase invented words (vwibo, fwoop…) — all declined.
  The mechanical sweep (RDAP .app/.com, DNS .gg, GitHub user + exact-name
  repo search, crates.io, npm, winget, one web search per survivor) showed
  every real English or pt-BR word, slang included, taken on .com and nearly
  always on .app.
- 2026-09-08 — round 4, two-word companion names (search fella, wiki
  fella, wiki homie…): declined. Micrographics brand case decided: keep the
  shipped caps rule; lowercase-kept variants (0.14em, 0.1em) declined.
  Round 5, built on "lore": **pick = loreno**. Declined: lore fella, lore
  buff, lore peek/hint/ping/monk, lore buddy/pal/tap/scout (list A entirely),
  loreiro, loreano, loresi, loredino, lorinho, lorimo, lorezo, lorela,
  lorico. Ruled out by the sweep: lore seeker (shipping game + studio), lore
  guide (LoreGuide.com), lorista (a losartan brand), wiki pal (shipping
  apps). Status → todo; the rename arc starts when the owner says so.
- 2026-09-08 — execution plan written from a full inventory of the name
  (111 files outside the vault, 52 inside); owner set the version to 0.3.0,
  shared with the later pt-BR support arc. Open before the code PR: the
  GitHub repo rename and the loreno.app registration (owner).
