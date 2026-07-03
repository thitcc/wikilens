---
description: Lint the planning vault — validate every vault/*.md against its conventions
allowed-tools: Bash(node:*), Read, Edit
---
Run the vault linter from the repo root:

`node .claude/skills/vault-lint/lint.mjs`

Summarize the report grouped by file. If there are errors, offer to fix the mechanical
ones (a `status`/`type` typo, a missing/stale `updated`, an unquoted all-digit hash, a
doc missing from the `index.md` list) — with my confirmation, never silently — then
re-run to confirm clean. Leave judgment calls (which tag applies, a wrong `related`
target) for me to resolve.
