---
title: Codex reads CLAUDE.md directly via config fallback instead of a generated AGENTS.md
type: decision
status: done
created: 2026-07-16
updated: 2026-07-16
tags: []
related: ["[[2026-07-16_codex-config-mirror-skill]]"]
commit: e1be9d2
---

# Codex reads CLAUDE.md directly via config fallback instead of a generated AGENTS.md

## Context
The repo is Claude-first with occasional Codex use. A user-triggered Codex
import once cloned `CLAUDE.md` / `.claude/` into `AGENTS.md` / `.agents/` with
a naive Claude → Codex rewrite that corrupted paths, duplicated the vault-lint
engine, and skipped two commands — and any second hand-maintained copy drifts.
Codex discovers instructions by checking `AGENTS.override.md`, then
`AGENTS.md`, then the filenames a trusted checkout's `.codex/config.toml`
lists in `project_doc_fallback_filenames`; workflows, however, are only
discoverable as skills under `.agents/skills/`. So guidance and workflows
need different treatments.

## Decision
Track `.codex/config.toml` with
`project_doc_fallback_filenames = ["CLAUDE.md"]` so Codex reads the canonical
`CLAUDE.md` directly, keep the root `AGENTS.md` absent **and unignored**, and
generate only workflow adapters — locally, gitignored — into
`.agents/skills/` with the committed deterministic `sync-agents` skill.

## Consequences
- Good: `CLAUDE.md` and `.claude/` stay the only hand-edited sources; the
  guidance path has zero transform steps left to corrupt; a stray `AGENTS.md`
  shows loudly in `git status` and aborts the generator; adapters reference
  committed engines instead of duplicating code.
- Cost / bad: each fresh clone needs one bootstrap run
  (`node .claude/skills/sync-agents/generate.mjs`); the checkout must be
  trusted in Codex, and config / instruction-discovery changes only take
  effect in a fresh Codex session; CLAUDE.md role wording must stay
  agent-neutral.
- Follow-ups: re-run the generator after `CLAUDE.md` / `.claude/**` changes;
  watch CLAUDE.md's growth toward Codex's 32 KiB default
  `project_doc_max_bytes` (the generator warns near the cap, though the cap
  covers the whole instruction chain, which it cannot fully measure).

## Alternatives considered
- **Generated root `AGENTS.md` mirror** — outranks the fallback in discovery,
  so any stale copy silently shadows `CLAUDE.md`, and it keeps alive the
  instruction transform the import already got wrong.
- **Global `~/.codex/config.toml` fallback** — leaks into unrelated repos and
  can't be reviewed or delivered through this repo's PR flow.
- **Codex custom prompts for workflows** — deprecated and local-only; skills
  are Codex's current reusable workflow surface.
- **LLM-driven rewrite instead of a deterministic transform** — silent
  "helpful" rewriting is exactly the observed failure mode; the allowlisted
  transform fails loudly on anything new instead.
- **Committing the generated adapters** — derived files land in every review
  and become a second drift surface; adapters stay local behind one bootstrap
  command.
