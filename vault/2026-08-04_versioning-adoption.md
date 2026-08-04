---
title: Adopt v0.1.0 and institutionalize the release ritual
type: plan
status: todo
created: 2026-08-04
updated: 2026-08-04
tags: [build]
related:
  - "[[2026-08-04_release-scoped-semver]]"
  - "[[2026-08-02_release-build-skill]]"
  - "[[2026-07-13_pr-delivery-workflow]]"
  - "[[2026-07-13_manual-smoke-checklist-live-cadence]]"
commit:
---

# Adopt v0.1.0 and institutionalize the release ritual

## Context / problem

[[2026-08-04_release-scoped-semver]] settled how WikiLens versions work; this
plan makes the baseline real. Every manifest already reads `0.1.0`, so
adoption is not a bump — it's pinning the number the repo already claims to a
commit, plus teaching the dev docs the ritual so future releases don't
re-derive it.

## Goal / non-goals

- Goal: an annotated `v0.1.0` tag on `main`, and the release ritual recorded
  where an agent will find it (a CLAUDE.md §3 release line, a versioning
  note in the `release-build` skill).
- Non-goal: a GitHub Release object or artifacts for 0.1.0 (tag-only — a
  Release with installers would imply the full ritual ran); anything from
  the follow-up idea docs.

## Approach

1. **Tag (human)** — tagging mutates `main`, so it stays owner-side:
   `git tag -a v0.1.0 -m "WikiLens 0.1.0: first versioned baseline"`, then
   `git push origin v0.1.0`. No manifest edits needed.
2. **Docs PR (agent)** — CLAUDE.md §3 gains a release line pointing at the
   ritual; the `release-build` skill gains a versioning note (bump PR → tag
   → build → smoke → `gh release create`); regenerate the Codex adapters
   with `node .claude/skills/sync-agents/generate.mjs`.
3. **Close** — set this doc and the ADR to `done`, bump `updated:`, set
   `commit:` to the docs-PR hashes.

## Decisions & trade-offs

Order is deliberate: the tag can land before the docs PR (the number is
already everywhere), but the docs shouldn't describe a ritual that has never
produced a tag. All real forks live in the ADR.

## Status log

- 2026-08-04 — created from the versioning explainer session; tag + docs
  pending.
