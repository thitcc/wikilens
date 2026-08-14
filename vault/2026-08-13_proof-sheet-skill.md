---
title: Codify the proof-sheet workflow as a skill
type: plan
status: done
created: 2026-08-13
updated: 2026-08-13
tags: [frontend]
related:
  - "[[2026-08-13_settings-stepper]]"
commit: [33011dc, 8ce0cbc]
---

# Codify the proof-sheet workflow as a skill

## Context / problem

The proof-sheet workflow — variant specimens on real tokens, published as an
artifact, picks asked mid-session, picks and declines recorded in the vault —
ran manually four times in the settings-stepper session and proved itself. Its
recipe currently lives in one oversized CLAUDE.md bullet plus session memory:
too short to carry the full process, and the sheet chrome (theme columns, axis
grid, backdrop, specimen injection) was rebuilt from scratch for every sheet.

## Goal / non-goals

- Goal: a `proof-sheet` skill (`.claude/skills/proof-sheet/SKILL.md`) carrying
  the full loop, building rules, and recording format; a reusable
  `templates/sheet-skeleton.html` asset distilled from the session's four
  sheets; the CLAUDE.md bullet slimmed to a pointer (single-source pattern);
  permanent design-hook ignores for sheet chrome.
- Non-goals: no change to the sheets already built; no new tests (the skill
  has no engine — release-build precedent); no product code.

## Approach

1. This plan doc → vault-lint → commit.
2. Template asset first (the sync-agents generator validates `.claude/...`
   path references in SKILL.md against the filesystem), then SKILL.md, then
   the hook-ignore config with an empirical suppression check.
3. `node .claude/skills/sync-agents/generate.mjs` — the new skill is picked up
   by directory scan; Codex gets the `$proof-sheet` adapter for free.
4. CLAUDE.md Design Context bullet → pointer; generator re-run.
5. Close this doc, `/check`, PR.

## Decisions & trade-offs

- Tag `frontend`, not `vault`: the registry scopes `vault` to the planning
  vault's own tooling; this skill governs front-end design work.
- The skeleton ships token *names* plus copy-instructions, never values —
  copy-pasted literals would go stale and a stale-token sheet lies about what
  the picks will look like in the product.
- Skill, not command: the template asset needs a base directory
  (house pattern: commands are pure instructions, skills carry files).
- Design-hook ignores (`detector.ignoreFiles` in `.impeccable/config.json`,
  new tracked file) for `.impeccable/critique/**` and the skill's templates:
  sheet chrome is deliberately non-product, and the recurring false positives
  cost a manual classification on every sheet edit. User opted in mid-session.

## Status log

- 2026-08-13 — created; branch `chore/proof-sheet-skill`. Mid-plan pick
  (AskUserQuestion): register the permanent hook ignores — chosen over
  keeping the classify-as-intentional routine.
- 2026-08-13 — shipped: `33011dc` (skill + skeleton + hook-ignore config;
  suppression verified empirically — a throwaway off-palette write under
  `.impeccable/critique/` produced zero findings) and `8ce0cbc` (CLAUDE.md
  bullet → pointer). The generator picked the skill up on directory scan
  (`generated: proof-sheet`, 8 adapters). Closed.
