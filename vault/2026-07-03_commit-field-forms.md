---
title: Document the vault commit: field forms (hash or list)
type: plan
status: done
created: 2026-07-03
updated: 2026-07-03
tags: [vault]
related: []
commit: d273bbd
---

# Document the vault `commit:` field forms (hash or list)

## Context / problem
The vault's `commit:` frontmatter field (introduced with the vault in `29a5b71`)
was named in a flat list but undocumented at the field itself, so its accepted forms
weren't discoverable in place — and it was unclear what to record when work spans
multiple commits.

## Goal / non-goals
- **Goal:** make the accepted forms self-evident wherever the field is met.
- **Non-goal:** a PR-number form — explicitly dropped; `commit:` is hashes only.

## Accepted forms for `commit:`
- Single short hash — `commit: 85aca6c`
- YAML list for multiple — `commit: [a1b2c3d, e4f5g6h]`
- Optional. Quote an all-digit hash so YAML keeps it a string.

## Approach (done — landed in `d273bbd`)
- **`index.md`:** added a **Frontmatter fields** table as the durable reference.
  Obsidian's Properties panel hides YAML `#` comments (and can strip them on edit),
  so a rendered table — not an inline comment — is the authoritative "in the spot" doc.
- **`templates/plan.md` & `templates/decision.md`:** inline comments on `type`,
  `status`, and `commit` so the frontmatter self-documents at authoring time (and for
  raw / grep / GitHub readers).
- **`CLAUDE.md` §7:** reworded the finishing step to name both forms.
- Added a `vault` tag to the `index.md` registry for vault-meta docs; this doc is
  its first user.

## Decisions & trade-offs
- Two forms only (hash or list); no PR ref, per the user's call.
- Inline template comments are a convenience, not the source of truth — the
  `index.md` table is, because Obsidian hides/strips YAML comments.

## Status log
- 2026-07-03 — done; changes landed in `d273bbd`. This record cites that commit.
