---
title: Use CLAUDE.md directly in Codex and mirror Claude workflows via a sync skill
type: plan
status: todo
created: 2026-07-16
updated: 2026-07-16
tags: []
related: ["[[2026-07-13_pr-delivery-workflow]]"]
commit:
---

# Use CLAUDE.md directly in Codex and mirror Claude workflows via a sync skill

## Context / problem

The repo is Claude-first, but Codex gets used occasionally. A user-triggered
Codex import cloned `CLAUDE.md` / `.claude/` into `AGENTS.md` / `.agents/` with
a naive `Claude -> Codex` rewrite: it corrupted paths (`.Codex/commands/`,
`node .Codex/skills/vault-lint/lint.mjs`), duplicated the vault-lint engine,
and skipped `/plan-doc` and `/decision-doc`. The checked-in Claude config must
remain the single source rather than drift against a second hand-edited copy.

Codex can read `CLAUDE.md` directly: a trusted repo-scoped
`.codex/config.toml` may set
`project_doc_fallback_filenames = ["CLAUDE.md"]`. Discovery checks
`AGENTS.override.md`, then `AGENTS.md`, then configured fallbacks, so generating
`AGENTS.md` would shadow the fallback and preserve the stale-copy risk. The
official [instruction-discovery guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md)
and [skills guide](https://learn.chatgpt.com/docs/build-skills) therefore support
a split design: direct discovery for guidance, generated adapters only for
workflows that Codex must find under `.agents/skills/`.

Current state (2026-07-16): root `AGENTS.md` is absent, `.agents/skills/` holds
only empty import-leftover directories, and `git status` is clean. `CLAUDE.md`
is 19,321 bytes, below Codex's default 32 KiB combined project-instruction
limit. Project config is loaded only for a trusted checkout, and config or
instruction-discovery changes require a fresh Codex session.

## Goal / non-goals

- Goal: `CLAUDE.md` stays the only hand-edited repository guidance; a tracked
  `.codex/config.toml` makes Codex load it directly without an `AGENTS.md`
  mirror.
- Goal: `.claude/` stays the only hand-edited workflow source; a committed
  Claude skill generates gitignored `.agents/skills/` Codex adapters on demand.
- Goal: generated adapters reference committed `.claude/` engines rather than
  copying code assets, reconcile stale output safely, and report a no-op when
  already current.
- Non-goal: Codex cloud/CI or a shared generated mirror. The tracked discovery
  bridge is portable, but generated skills remain local-only while this is a
  solo project.
- Non-goal: sync on every file save. The bootstrap command is explicit and the
  source skill may trigger after config edits, but implicit invocation is not a
  correctness guarantee.

## Approach

1. **Direct instruction discovery and narrow ignore rules.**
   - Add tracked `.codex/config.toml` containing exactly
     `project_doc_fallback_filenames = ["CLAUDE.md"]`.
   - Keep root `AGENTS.md` absent and *unignored*: an import-created file must be
     visible in `git status`, and the sync script must fail because it would
     shadow `CLAUDE.md`.
   - Ignore only `/.agents/skills/`, leaving the rest of `.agents/` available
     for future checked-in configuration. Remove any stale local import output
     before the first generation; do not describe the already-absent
     `AGENTS.md` as awaiting deletion.

2. **Committed source skill and deterministic adapter generator.**
   - Add `.claude/skills/sync-agents/SKILL.md`, `generate.mjs`, and focused
     Node built-in tests. The first run is
     `node .claude/skills/sync-agents/generate.mjs`; after generation Claude may
     invoke `/sync-agents` and Codex may invoke `$sync-agents` (or select it
     through `/skills`).
   - Read source directories in stable sorted order. Generate exactly `check`,
     `plan-doc`, `decision-doc`, `vault-lint`, and `sync-agents` under
     `.agents/skills/<name>/SKILL.md`.
   - For source skills, require valid `name` and `description`. For commands,
     synthesize `name` from the filename and preserve `description`. Strip only
     allowlisted Claude-only fields (`allowed-tools`, `argument-hint`), report
     what was stripped, and fail on unknown metadata; Codex sandbox and
     approval policy remain the actual permission boundary.
   - Translate known Claude command references (`/check`, `/vault-lint`,
     `/plan-doc`, `/decision-doc`, `/sync-agents`) to Codex `$name` mentions in
     emitted bodies. Replace `$ARGUMENTS` with prose telling Codex to use text
     accompanying the skill mention, or infer it from the recent discussion
     when absent. Never rewrite `.claude/...` paths or `CLAUDE.md` references.
   - Declare `vault-lint`'s source skill as the canonical winner over its
     same-named command and report that collision. Fail on every undeclared
     collision. Normalize the canonical skill wording so its engine is
     unambiguously `node .claude/skills/vault-lint/lint.mjs`, not “in this
     skill folder.”
   - Track generator-owned outputs in a local manifest. Remove stale *owned*
     adapters when sources disappear, fail rather than overwrite an unowned
     collision, and leave matching files untouched so a no-op changes neither
     content nor mtimes.

3. **Fail-loud guardrails.**
   - Reject a present root `AGENTS.md`, invalid Codex skill frontmatter,
     `.Codex` paths, leftover `$ARGUMENTS`, unresolved known `/name` command
     references, unexpected metadata/collisions, and missing committed engine
     paths.
   - Check the source instruction size and warn as the *combined instruction
     chain* approaches `project_doc_max_bytes`; a file-size check alone must not
     claim that Codex cannot truncate the full chain.
   - Print the generated/updated/removed adapter names, declared collisions,
     stripped metadata, and either a change summary or `up to date`.

4. **Shared guidance and bootstrap.**
   - Add a compact `CLAUDE.md` convention: it is canonical for both agents;
     Claude `/name` workflows map to Codex `$name` skills; after changes to
     `CLAUDE.md` or `.claude/**`, run the exact bootstrap command above.
   - Make shared role rules agent-neutral (for example, “the agent never
     merges”) while preserving `.claude/` paths. This avoids feeding Codex
     incorrect Claude-only wording through the fallback.
   - Document that the repo must be trusted and a new Codex session must be
     opened after `.codex/config.toml` or instruction-discovery changes.

5. **Verification and delivery.**
   - Unit-test frontmatter conversion, `/name` and `$ARGUMENTS` translation,
     the declared collision, rejection of unknown metadata/collisions,
     generator-owned stale cleanup, unowned-output protection, missing paths,
     and idempotence using temporary fixtures.
   - Run the generator twice: the first run emits the five adapters; the second
     reports `up to date` without rewriting them.
   - From a fresh Codex session in a trusted checkout with no `AGENTS.md`, ask a
     doc-only question (why Shift+C is problematic) to prove `CLAUDE.md` loaded;
     confirm all five `$name` skills are discoverable and run `$vault-lint`.
   - Confirm `git status --ignored` shows `.agents/skills/` ignored,
     `.codex/config.toml` tracked, and no `AGENTS.md`; run the full `/check` and
     vault lint gates.
   - Capture the configuration fork in a decision doc when implementation
     starts, cross-link it here, and deliver via PR per
     [[2026-07-13_pr-delivery-workflow]].

## Decisions & trade-offs

- **Direct `CLAUDE.md` fallback over a generated `AGENTS.md`:** both preserve a
  Claude-first source, but `AGENTS.md` has higher discovery precedence and can
  become stale. Direct fallback removes the instruction transform entirely;
  only workflow adapters still need generation.
- **Tracked project config over `~/.codex/config.toml`:** the project setting is
  scoped, reviewable, and reproducible for every trusted WikiLens checkout. A
  global fallback would affect unrelated repos and cannot be delivered by the
  project PR.
- **Skills over Codex custom prompts:** custom prompts are deprecated and local
  only; skills are Codex's current reusable workflow surface and can be selected
  explicitly or matched by description.
- **Deterministic transform over an LLM rewrite:** a small allowlisted transform
  makes argument and invocation differences explicit and fails on new syntax.
  Silent “helpful” rewriting is the failure mode this plan is removing.
- **Generated adapters remain local:** this keeps `.claude/` canonical and avoids
  reviewing derived files, at the cost of a bootstrap step on a fresh clone.

## Status log

- 2026-07-16 — created; initial generated-`AGENTS.md` approach agreed in a
  Cowork session; execution not started.
- 2026-07-16 — reviewed against the current Codex manual and repository state:
  replaced the `AGENTS.md` mirror with tracked direct `CLAUDE.md` fallback,
  narrowed generation to skill adapters, and specified Codex invocation,
  argument conversion, bootstrap, ownership, and trust behavior.
