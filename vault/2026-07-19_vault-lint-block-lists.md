---
title: Teach vault-lint to read block-form YAML lists, and gate the tooling tests
type: fix
status: done
created: 2026-07-19
updated: 2026-07-19
tags: [vault, testing]
related: ["[[2026-07-19_remove-graphify]]", "[[2026-07-16_codex-config-mirror-skill]]", "[[2026-07-13_dependency-audit-lint-gates]]"]
commit: [5c6affa, c31dce4, 97967b9]
---

# Teach vault-lint to read block-form YAML lists, and gate the tooling tests

## Context / problem

Closing [[2026-07-19_remove-graphify]] drew a false warning — *"status is done
but has no tags and no related links"* — from a doc carrying three `related`
wikilinks. The doc was fine; the linter was not.

`parseFrontmatter` (`.claude/skills/vault-lint/lint.mjs`) is a line-oriented
regex parser, not a YAML parser, and its match is anchored at column 0. Given a
YAML **block sequence**, the key captures an empty value and every indented
`- item` line is silently dropped:

```
BLOCK   fm.related = ""                          → inlineList → []
INLINE  fm.related = "[\"[[a]]\", \"[[b]]\"]"    → inlineList → ["[[a]]","[[b]]"]
```

`inlineList` was innocent — handed an empty string, it correctly returned `[]`.
Three checks silently no-op'd as a result: the dangling-`related`-target ERROR,
the unquoted-wikilink ERROR, and the "done means…" warning, which fired
backwards. Only the third was visible, and only when `tags` was *also* empty —
which is why it survived: all seven block-form docs have tags. The doc that
closed the graphify work was the first to combine block-form `related` with
`tags: []`.

Audit of the live vault at the time: **7 docs, 16 wikilinks never validated, 0
dangling.** Nothing was broken; the linter just reported "✔ clean" while
checking about two-thirds of the `related` links it claimed to cover.

A second problem surfaced while planning the fix: **nothing ran `node --test`.**
Vitest's `include` is `src/**/*.test.{ts,tsx}`, so `sync-agents`'
`generate.test.mjs` — 23 tests — had never gated anything in CI or `/check` and
could have been rotting unnoticed.

## Goal / non-goals

- Goal: both YAML list forms lint identically, with a regression suite proving it.
- Goal: tooling tests actually gate, in `/check` and in CI.
- Non-goal: a real YAML parser. The frontmatter dialect here is small and closed;
  a dependency would cost more than it buys.
- Non-goal: rewriting the seven block-form docs into inline form. Both spellings
  are legal YAML and the vault already uses both; the linter should meet the docs
  where they are.

## Approach

1. **Parser fix.** `parseFrontmatter` walks by index; when a key has no inline
   value it absorbs following `^\s+-\s+` lines into the inline form, quotes
   intact. One change serves both consumers — `inlineList` splits on the comma,
   and the raw `matchAll` quoting check still sees its quotes.
2. **Testability**, ported from `sync-agents/generate.mjs` rather than invented:
   the vault root became a defaulted parameter (`runLint(root = DEFAULT_ROOT)`),
   the run returns a report instead of printing, `printReport` took an injectable
   log sink, and all printing plus every `process.exit` moved behind a
   Windows-aware `isMain` guard. The `process.exit(2)` preflight became a thrown
   `LintError`. Pure helpers are exported for direct unit tests.
3. **Tests** — `lint.test.mjs`, 15 cases on `node:test` with temp-dir vault
   fixtures, mirroring `generate.test.mjs`'s `makeRoot` shape.
4. **Gates** — a `test:node` script, a CI step in the `frontend` job, a fifth
   `/check` step, and the CLAUDE.md §3 entries.
5. **Retire the manual index list** (added mid-PR, see below): delete the
   hand-maintained doc census from `vault/index.md`, drop the linter's
   index-drift warning that enforced it, and remove the instructions telling
   agents to maintain it from `/plan-doc`, `/decision-doc`, `/vault-lint`, and
   the vault-lint skill.

## Decisions & trade-offs

- **The block-to-inline normalization happens in the parser, not at each call
  site.** Both the list consumer (`inlineList`) and the raw-text quoting check
  read `fm.related`, so normalizing once at the source keeps them agreed; fixing
  it per-consumer would have invited exactly the drift that caused the bug.
- **Exit codes are unchanged** (0 clean/warnings, 1 errors, 2 vault missing) —
  `SKILL.md` documents them as a contract, and the refactor routes every path
  back through the guard to preserve them.
- **`runLint`'s body was re-indented** when it became a function. The diff looks
  larger than the change; `git diff -w` shows the true delta.
- **The fix was verified to be a no-op on the live vault before landing**: only
  `related` uses block form, all 16 items were already quoted, so the newly-run
  checks pass. The lone bare `commit:` in `2026-07-13_testing-audit.md` finds no
  list items, so its value stays empty and its existing warning is preserved —
  pinned by a test.
- **`test:node` uses a Node-level glob** (`".claude/skills/**/*.test.mjs"`) so a
  new tooling test file is picked up without touching `package.json`. Passing the
  directory alone does not work on Node 24; the quoted glob does, on both the
  local runtime and CI's Node 22.
- **The manual index list is retired, not repaired.** `vault/index.md` carried a
  hand-maintained census of every doc by status. Three arguments retired it
  rather than fixing its entries: Dataview is installed
  (`vault/.obsidian/community-plugins.json`), so the three dashboards already in
  the file derive the same views from frontmatter; the list was **measurably
  wrong** — its `Todo (rewrite-review findings, in execution order)` line named
  seven docs, all seven of which are `status: done`; and its one unique
  contribution, the execution ordering, is already written into each of those
  docs' own status logs (`#1, priority 1`, `#2, priority 2`, …). Keeping it
  would mean a census that must be hand-edited on every open and close, which
  this very PR had to do three times before deciding against it.
- **The linter's index-drift warning went with it.** Leaving it would have had
  the tooling nag agents to maintain the thing being retired. Its `indexLinks`
  set died with it; `indexText` survives because `loadRegistry` still reads the
  tag registry from the same file, and `EXEMPT` survives because the filename
  rule still needs it.
- **`grep` is now the stated fallback**, not a second list. Commands can't drift
  the way an enumeration does.
- No real fork here — nothing warranting a separate decision doc.

## Status log

- 2026-07-19 — created; bug found while closing [[2026-07-19_remove-graphify]].
- 2026-07-19 — **executed and closed.** Parser fixed and `lint.mjs` restructured
  on the `generate.mjs` shape; `lint.test.mjs` adds 15 cases. The suite was
  verified to actually catch the bug: with the fix disabled, 6 of the 15 fail,
  covering all three silent consequences. Tooling tests now gate —
  `npm run test:node` runs 38 (23 sync-agents + 15 vault-lint) in `/check` step 3
  and in CI's frontend job. Live vault output is unchanged: 57 docs, 0 errors,
  the same lone pre-existing warning, exit 0. Gates green: `/check` all five,
  `/vault-lint` clean, `sync-agents` regenerated (2 adapters updated).
  Also corrected an index slip from the previous PR — [[2026-07-19_remove-graphify]]
  was closed `done` but left filed under **Active**; vault-lint checks only that
  a doc appears somewhere in `index.md`, not which section, so it passed silently.
- 2026-07-19 — **manual index list retired** (same PR). That slip, plus three
  hand-edits to the census in a single session, made the case: the list is gone,
  the linter's index-drift warning with it, and the four instruction files no
  longer ask agents to maintain it. Dataview's dashboards and `grep` cover it.
  Verified: registry still parses (an unknown tag still errors with a
  did-you-mean hint), 57 docs / 0 errors / exit 0, 38 node tests green,
  `sync-agents` updated exactly the three predicted adapters and reports
  "up to date" on a second run.
