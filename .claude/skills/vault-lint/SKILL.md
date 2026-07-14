---
name: vault-lint
description: Validate the WikiLens planning-vault docs (vault/*.md) against their frontmatter conventions — closed tag registry, type/status vocab, YYYY-MM-DD_slug filenames, ISO dates, quoted resolvable wikilinks. Invoke after creating or closing a vault doc, before committing changes under vault/, or when asked to check the vault.
allowed-tools: Bash(node:*), Read, Edit
---

# vault-lint

Deterministic checker for the planning vault. The engine is `lint.mjs` in this skill
folder; the tag registry and vocab live in `vault/index.md` and `CLAUDE.md` §7 — the
script reads the registry live so it never drifts.

## When to run
- Right after `/plan-doc` or `/decision-doc` creates a doc.
- When closing a doc to `status: done`.
- Before committing — or pushing a PR with — any change under `vault/`.
- Whenever asked to check or tidy the vault.

## How to run
From anywhere in the repo (the script resolves the vault relative to itself):

```
node .claude/skills/vault-lint/lint.mjs
```

It prints `vault-lint: N docs checked — X errors, Y warnings`, then per-file `ERROR` /
`WARN` lines, and exits non-zero if there are any errors.

## Acting on results
- Report the grouped results.
- Offer to fix the **mechanical** issues on confirmation — never rewrite frontmatter
  silently: a `status`/`type` typo, a missing/should-bump `updated`, an unquoted
  all-digit `commit` hash, or a doc missing from the `index.md` list.
- Leave **judgment** issues to the human: which tag really applies, a wrong `related`
  target, whether a `done` doc should carry a `commit`.
- Re-run after fixing to confirm a clean pass.

## Rules
**Errors** (gate, non-zero exit): missing required field (`title/type/status/created/
updated`); `type`/`status` off-vocab; invalid ISO date or `updated` < `created`;
filename ≠ `YYYY-MM-DD_<kebab>.md` or its date prefix ≠ `created`; a tag outside the
registry; a `related` `[[slug]]` with no matching file, or an unquoted wikilink.

**Warnings** (don't gate): `done` with no `commit`; `done` with no tags and no related;
an unquoted all-digit `commit` hash; a doc unreferenced in `index.md`.
