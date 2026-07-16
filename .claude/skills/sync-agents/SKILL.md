---
name: sync-agents
description: Regenerate the local Codex skill adapters under .agents/skills/ from the committed Claude workflow sources (.claude/commands/*.md and .claude/skills/*/SKILL.md). Run after changing CLAUDE.md or anything under .claude/, on a fresh clone before using Codex skills, or when Codex workflow skills look missing or stale.
allowed-tools: Bash(node:*), Read
---

# sync-agents

`.claude/` is the only hand-edited workflow source. Codex discovers the same
workflows as skills under `.agents/skills/` — generated, local-only, and
gitignored. The deterministic generator is
`.claude/skills/sync-agents/generate.mjs`; repository guidance needs no
generation at all (Codex reads `CLAUDE.md` directly via `.codex/config.toml`).

## How to run
From the repo root:

```
node .claude/skills/sync-agents/generate.mjs
```

It prints declared collisions, stripped Claude-only metadata, any warnings, and
the generated / updated / removed adapter names, ending in a change summary —
or `up to date` when nothing needed writing (a no-op run rewrites nothing, so
content and mtimes are untouched). Exit code 0 on success, 1 on any failure.

## When to run
- After changing `CLAUDE.md` or anything under `.claude/` (commands, skills).
- On a fresh clone, before using Codex skills — adapters are never committed.
- Whenever Codex workflow skills look missing or stale.

## What it does
- Reads `.claude/commands/*.md` and `.claude/skills/*/SKILL.md` in sorted order
  and emits one adapter per workflow at `.agents/skills/<name>/SKILL.md`.
- Converts Claude-isms: `/name` workflow references become Codex `$name`
  mentions, the argument placeholder becomes prose, and Claude-only frontmatter
  (`allowed-tools`, `argument-hint`) is stripped and reported — the Codex
  sandbox and approval policy remain the real permission boundary.
- Copies `.claude/...` paths and `CLAUDE.md` references verbatim: adapters
  point at the committed engines (e.g. vault-lint's `lint.mjs`) instead of
  duplicating them.
- Tracks its outputs in `.agents/skills/.sync-manifest.json`: adapters whose
  source disappeared are removed, and files it does not own are never
  overwritten.

## Failure modes (fail-loud by design)
The run aborts with a clear error instead of guessing: a root `AGENTS.md` or
`AGENTS.override.md` (either would shadow `CLAUDE.md` in Codex's instruction
discovery), unknown source frontmatter, an undeclared name collision, an
unowned file at an output path, a corrupt ownership manifest, a broken
`.claude/...` engine reference, or an untranslated Claude-ism surviving in an
emitted body. Fix the cause and re-run.
Never hand-edit anything under `.agents/skills/`.

## Tests
`node --test .claude/skills/sync-agents/generate.test.mjs` runs the generator's
unit tests (Node built-in test runner, no dependencies).
