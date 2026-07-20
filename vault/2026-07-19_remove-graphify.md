---
title: Remove graphify from the repo and the machine
type: plan
status: done
created: 2026-07-19
updated: 2026-07-19
tags: []
related: ["[[2026-07-17_graphify-findings-review]]", "[[2026-07-17_claude-md-capture-catchup]]", "[[2026-07-16_codex-config-mirror-skill]]"]
commit: 9689f39
---

# Remove graphify from the repo and the machine

## Context / problem

graphify — the knowledge-graph tool wired in on 2026-07-17
([[2026-07-17_graphify-findings-review]]) — is being retired. It was wired into
the repo at four points: a `## graphify` section in `CLAUDE.md` (hand-customized
after the findings review, so it carried caveats the vendor's own uninstaller
knows nothing about), a `merge=graphify` driver line in `.gitattributes`, a
`graphify-out/` ignore block plus a hook-guard comment in `.gitignore`, and
per-machine state outside git (post-commit/post-checkout git hooks, a PreToolUse
hook-guard in `.claude/settings.local.json`, a user-profile skill, and the CLI).

The owner ran the vendor uninstallers before this session. Those cover the
machine-side state and, as it turns out, both of the tracked-file edits they
claimed — but `.gitignore` is untouched, and nothing has been committed. This
doc covers verifying what actually landed, finishing the tracked-file cleanup,
and delivering it as a PR.

## Goal / non-goals

- Goal: no live graphify instruction or config remains in the tracked tree.
- Goal: verify the owner-run machine-side removal actually landed, rather than
  trusting the uninstaller's console output.
- Non-goal: rewriting history. Vault docs that *record* graphify work stay
  exactly as they are — they are the durable record of a real run, and
  [[2026-07-17_graphify-findings-review]] in particular stays untouched.
- Non-goal: the user-profile `~/.claude/CLAUDE.md` graphify section. Outside
  this repo; flagged to the owner separately.

## Approach

**Owner-run (PowerShell, already executed before this session):**

1. `graphify hook uninstall` — removed the post-commit and post-checkout git
   hooks and the `merge=graphify` driver line from `.gitattributes`.
2. `graphify claude uninstall` — removed the user-profile skill
   (`~/.claude/skills/graphify/SKILL.md`), the `## graphify` section from
   `CLAUDE.md`, and the PreToolUse hook from `.claude/settings.local.json`.
3. `graphify uninstall --purge` — deleted `graphify-out/` (gitignored, never
   committed); everything else reported as already-removed.
4. `uv tool uninstall graphifyy` + `winget uninstall astral-sh.uv` — removed the
   CLI and the `uv` runtime that hosted it.
5. **Outstanding:** `Remove-Item -Recurse -Force src-tauri\graphify-out` — a
   second output tree the purge missed (see below).

**Agent-run (this doc's work):**

5. Verify the machine-side cleanup: no graphify blocks in `.git/hooks/`, no
   `graphify-out/`, no hook-guard in `.claude/settings.local.json`, no adapter
   in `.agents/skills/`, no merge driver in the local git config.
6. `.gitignore`: drop the `graphify-out/` block; reword the comment above
   `.claude/settings.local.json` so the ignore rule survives without citing the
   hook-guard as its reason.
7. Confirm the uninstaller's `CLAUDE.md` and `.gitattributes` edits are complete
   (they arrive as uncommitted working-tree changes) and commit them.
8. Grep the tracked tree for surviving references and triage each one.
9. `node .claude/skills/sync-agents/generate.mjs` — `CLAUDE.md` changed, so the
   Codex adapters get regenerated per convention.
10. Gates: `/vault-lint`, `/check`; PR via `gh pr create --assignee @me` on
    branch `chore/remove-graphify`; human merges.

## Decisions & trade-offs

- **`.claude/settings.local.json` keeps its ignore rule.** The graphify
  hook-guard was only the *reason* the file was first ignored; the file is still
  per-machine (absolute paths, machine-local permission grants), so only the
  comment's justification changes, not the rule.
- **Historical vault docs stay verbatim, including their graphify commands.**
  [[2026-07-19_menu-keyboard-nav]]'s gates step lists `graphify update .`
  because that is what was actually run at the time. A vault doc is a record of
  what happened, not a live runbook — editing closed docs to match present-day
  tooling would falsify the record. Same reasoning keeps
  [[2026-07-17_graphify-findings-review]] untouched.
- **The vendor uninstaller's tracked-file edits are verified, not trusted.**
  It claimed the `CLAUDE.md` section removal twice with contradictory output
  (`graphify claude uninstall` said "section removed", then `uninstall --purge`
  said "not found"), so the working-tree diff is the authority.
- **A second output tree survived the purge: `src-tauri/graphify-out/`** (2.5 MB
  — graph.json, graph.html, GRAPH_REPORT.md, manifest.json, a 38-file AST
  cache), left by a run rooted at `src-tauri/` instead of the repo root;
  `--purge` deleted only the tree its `.graphify_root` marker pointed at. It hid
  in plain sight because the old ignore pattern `graphify-out/` carried no
  leading slash and so matched at any depth — removing that rule is what
  revealed it. Untracked and never committed, so it cannot reach the PR, but it
  now shows in `git status` until deleted. Left for the owner to remove in
  PowerShell, consistent with machine-side steps 1–4; deliberately *not*
  re-ignored, since a fresh `graphify-out/` rule would defeat the point of this
  change.
- No real fork here — nothing warranting a separate decision doc.

## Status log

- 2026-07-19 — created; machine-side removal already run by the owner.
- 2026-07-19 — **executed and closed.** Machine-side removal verified: no
  graphify blocks in `.git/hooks/` (only `.sample` files), no root
  `graphify-out/`, no PreToolUse hook-guard in `.claude/settings.local.json`, no
  adapter in `.agents/skills/`, no merge driver in the local git config. The
  uninstaller's two tracked-file edits turned out complete — the `## graphify`
  section was fully gone from `CLAUDE.md`, with no hand-customized remnant left
  behind — so the only new edit was `.gitignore`. Codex adapters regenerated
  (`sync-agents`: up to date, 5 adapters). Grep of the tracked tree found the
  only surviving references in vault docs, all historical, all left verbatim.
  Gates green: `/vault-lint` 56 docs / 0 errors, `/check` all four (tsc, 115
  frontend tests, clippy `-D warnings`, 247 Rust tests). One machine-side item
  stays open for the owner: deleting `src-tauri/graphify-out/`.
