---
title: Mirror the Claude config into a gitignored Codex config via a sync skill
type: plan
status: todo
created: 2026-07-16
updated: 2026-07-16
tags: []
related: ["[[2026-07-13_pr-delivery-workflow]]"]
commit:
---

# Mirror the Claude config into a gitignored Codex config via a sync skill

## Context / problem

The repo is driven Claude-first, but Codex gets used occasionally. Codex's
auto-migration cloned `CLAUDE.md`/.`claude/` into `AGENTS.md`/`.agents/` with a
naive `Claude → Codex` find-and-replace: it corrupted paths (`.Codex/commands/`,
`node .Codex/skills/vault-lint/lint.mjs` — neither exists), duplicated the
vault-lint engine wholesale, and silently skipped 2 of the 4 commands
(`/plan-doc`, `/decision-doc`). Hand-maintaining two configs means every doc,
gotcha, and skill edit lands twice and drifts. Solo project, Claude preferred:
nothing forces the Codex config into git history — it can be a generated,
gitignored artifact instead of a second source.

## Goal / non-goals

- Goal: `CLAUDE.md` + `.claude/` stay the only hand-edited config; a committed
  skill (`/sync-agents`) regenerates `AGENTS.md` + `.agents/skills/` on demand;
  generated output is gitignored.
- Goal: generated skills point at committed engines
  (`node .claude/skills/vault-lint/lint.mjs`) — code assets are never copied.
- Goal: re-running with no upstream changes is a reported no-op.
- Non-goal: Codex cloud/CI or other collaborators (a gitignored mirror is
  local-only by design; revisit only if the project stops being solo).
- Non-goal: auto-sync on every file save — invocation is manual or
  agent-triggered after config edits; the run is cheap.

## Approach

1. Delete the broken artifacts the Codex migration left (`AGENTS.md`,
   `.agents/` — both still untracked), then add both to `.gitignore`.
2. New committed skill `.claude/skills/sync-agents/`: `SKILL.md` +
   `generate.mjs` (Node, zero deps, same shape as vault-lint's `lint.mjs`).
   - `AGENTS.md` from `CLAUDE.md`: prepend a `GENERATED — edit CLAUDE.md and
     run /sync-agents` banner; retitle; apply an **explicit substitution
     table** for agent-role phrases only (e.g. "Claude never merges" → "the
     agent never merges"; slash-command mentions get a Codex-side note). Never
     rewrite `.claude/…` paths or `CLAUDE.md` self-references — that blind
     replace is exactly what broke the auto-clone.
   - `.agents/skills/<name>/SKILL.md` from `.claude/skills/*/SKILL.md` and
     `.claude/commands/*.md`: skills copied minus Claude-only frontmatter
     (keep `name`/`description`); commands converted to skill format; on a
     name collision (vault-lint is both) the skill wins, mirroring Claude
     Code's own precedence; engines are not copied — bodies already invoke the
     committed `.claude/…` path, which Codex runs fine from the repo root.
   - Guardrails, loud by default: fail if output contains `.Codex` or a
     "Claude" occurrence outside known-safe patterns (paths, `CLAUDE.md`);
     fail if a referenced repo path doesn't exist; warn near Codex's 32 KiB
     `project_doc_max_bytes` default. Print "up to date" when output matches.
3. `SKILL.md` wiring: description worded so the agent invokes it after edits
   to `CLAUDE.md` or `.claude/**`; also user-invocable as `/sync-agents`. Body:
   run `generate.mjs`; if it flags an unhandled phrase, extend the
   substitution table (in the script, reviewed like any code) and re-run.
4. Docs: one bullet in `CLAUDE.md` §4 — Codex mirror is generated, gitignored,
   edit only the Claude side.
5. Verify: run the skill; `rg '\.Codex' AGENTS.md .agents` comes back empty;
   in Codex, ask a doc-only question (why Shift+C is problematic) and run the
   mirrored vault-lint skill; `git status` shows no generated files.
6. Deliver via PR per the delivery workflow ADR.

## Decisions & trade-offs

- **Generated mirror vs committed canonical `AGENTS.md`** (with `CLAUDE.md` as
  an `@AGENTS.md` import shim), vs **Codex's `project_doc_fallback_filenames`**
  pointing at `CLAUDE.md`: the mirror keeps the repo single-source and
  Claude-first at the cost of being local-only and stale between runs
  (mitigated by the agent-invocation wording and the banner). This is a real
  fork — capture it as a `/decision-doc` ADR when implementation starts and
  cross-link it here.
- **Deterministic script vs LLM rewrite** for the doc transform: script with
  an explicit substitution table that fails loudly on anything it doesn't
  recognize. The auto-clone's failure mode was silent corruption; ours should
  be a visible error listing the offending lines.

## Status log

- 2026-07-16 — created; approach agreed in chat (Cowork session), execution
  not started.
