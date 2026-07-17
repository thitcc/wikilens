---
title: Review the findings from the graphify knowledge-graph build
type: plan
status: todo
created: 2026-07-17
updated: 2026-07-17
tags: []
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-14_post-merge-dead-code-cleanup]]"]
commit:
---

# Review the findings from the graphify knowledge-graph build

## Context / problem

A full graphify run (2026-07-17) built a knowledge graph of the repo: **1,094
nodes, 2,261 edges, 65 communities** from 154 files (72 code files via AST,
68 docs + 14 images via LLM extraction, ~493k extraction tokens). Querying the
graph averages ~11.5k tokens per question vs ~73k naive context — a 6.4x
reduction. The build surfaced structural findings that deserve a human pass,
but the session ended before any review happened.

`graphify-out/` (graph.json, graph.html, GRAPH_REPORT.md) is **gitignored** —
regenerable locally, rebuilt by the installed git hooks — so this doc is the
durable record of the findings. graphify itself is wired in via a `## graphify`
section in `CLAUDE.md`, PreToolUse hooks in the local Claude settings, and
post-commit/post-checkout git hooks.

## Goal / non-goals

- Goal: triage every finding below to one of **fix / document / dismiss**, in a
  later session.
- Non-goal: acting on any finding now.
- Non-goal: tracking graphify output in git.

## Findings to review

### Graph health

- **194 dangling doc→code edges** — semantic extraction produced code-symbol
  ids that don't match the AST extractor's ids, so those doc-to-code links were
  dropped from the built graph. Decide: stale docs, or just id-format mismatch
  (noise)?
- 124 collapsed parallel edges — expected artifact of the undirected build
  (mostly repeated struct-field references). Probably dismiss.

### God nodes (most connected)

`String` (119 edges — likely Rust-type noise), `AppError` (48), `AppState`
(37), `GameWiki` (32), `UserWikiStore` (23), `fetch_pages()` (22),
`answer_streaming()` (21), `run_ask()` (20), `ask()` (19), `Provider` (19).
Sanity-check the real ones against the CLAUDE.md architecture map — do the
hubs match what we believe the core abstractions are?

### Surprising connections (verify each still holds in code)

- `Deterministic empty-merge fallbacks` → `run_ask()` (docs/ai-workflow.html → commands.rs)
- `Eager LLM query rewrite + merge` → `rewrite_query()` (docs/ai-workflow.html → llm.rs)
- `srwhat=text rule` → `search()` (CLAUDE.md → wiki/search.rs)
- `WikiLens overlay application` ↔ `All HTTP happens in Rust` (README.md ↔ CLAUDE.md)
- Screenshot-capture plan → `crop_to_attachment()` ([[2026-07-06_screenshot-capture-to-prompt]] → capture.rs)

### Bridge nodes (high betweenness)

- [[2026-07-15_slow-wiki-status-hint]] bridges *Rewrite & Debug Plans* ↔
  *Testing & Hardening Plans* ↔ *Frontend IPC Bridge* (0.118) — a plan doc
  reaching directly into frontend code via `isWikiBoundStatus()`; unusual, most
  vault docs only link other docs or Rust symbols.
- `isWikiBoundStatus()` bridges *Frontend IPC Bridge* ↔ *Rewrite & Debug
  Plans* (0.117).
- `String` (0.250) — probably type-noise; dismiss unless the review says
  otherwise.

### Structure questions

- 160 weakly-connected nodes — possible documentation gaps or missing edges.
- Cohesion 0.068 for the *Wiki Page Fetch Core* community and 0.043 for *NPM
  Package Dependencies* — split-worthy modules, or clustering artifact?

## Approach

1. Regenerate the graph locally if stale (`graphify update .`), open
   `graphify-out/GRAPH_REPORT.md` next to this checklist.
2. Walk each finding above; verify against the code (`graphify query` /
   `graphify path` first, per the CLAUDE.md graphify rules).
3. Record each verdict inline here (fix / document / dismiss); spin out
   separate plan docs for anything that becomes real work.
4. Close this doc (`status: done`, `commit:` ref) when every finding has a
   verdict.

## Decisions & trade-offs

- `graphify-out/` stays out of git: the post-commit hook rebuilds it on every
  commit, so tracking it would churn constantly; cache/ and cost.json are
  machine-specific. Trade-off: findings must be transcribed here (done above).
- The graphify PreToolUse hooks live in the local (untracked) Claude settings —
  they hardcode this machine's absolute `graphify.EXE` path; a fresh clone
  re-runs `graphify claude install`.

## Status log

- 2026-07-17 — created from the graphify build; review deferred.
