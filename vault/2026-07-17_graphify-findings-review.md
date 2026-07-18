---
title: Review the findings from the graphify knowledge-graph build
type: plan
status: done
created: 2026-07-17
updated: 2026-07-17
tags: []
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-14_post-merge-dead-code-cleanup]]", "[[2026-07-17_claude-md-capture-catchup]]"]
commit: 9f4230e
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
  **Verdict: dismiss — id-format mismatch, zero stale docs.** The built graph
  is clean (`graphify diagnose multigraph`: 0 dangling endpoints); the 194
  existed only in the original build's console output. Reconstructed from the
  semantic cache: 30 dropped edges / 20 missing ids today, all 30 id-format
  mismatches — semantic ids drop the struct segment
  (`…_record_rewrite_outcome` vs AST `…_appstate_record_rewrite_outcome`),
  consts like `GOLDEN_CASES` get no AST node, and 18 invented `claude_*`
  concept slugs never match the extractor's heading-based CLAUDE.md ids. Every
  referenced symbol and concept exists in the repo; the mismatch is internal
  to the external graphify extractor.
- 124 collapsed parallel edges — expected artifact of the undirected build
  (mostly repeated struct-field references). Probably dismiss.
  **Verdict: dismiss — confirmed expected.** Reconstructed from the AST cache:
  111 collapsed extras today, dominated by repeated struct-field references
  (`AppState → Mutex` ×4, etc.); only edge multiplicity is lost, and
  `graphify diagnose multigraph` exists precisely to audit this.

### God nodes (most connected)

`String` (119 edges — likely Rust-type noise), `AppError` (48), `AppState`
(37), `GameWiki` (32), `UserWikiStore` (23), `fetch_pages()` (22),
`answer_streaming()` (21), `run_ask()` (20), `ask()` (19), `Provider` (19).
Sanity-check the real ones against the CLAUDE.md architecture map — do the
hubs match what we believe the core abstractions are?

**Verdict: document — recorded here.** Hubs 2–10 all map to §2/§4 core
abstractions (`AppError` referenced in 13 Rust files; `GameWiki`,
`UserWikiStore`, `Provider`, `AppState`, plus the ask-pipeline spine), with
zero numeric drift in the d2e558a rebuild. The caveats worth keeping for
future graph reads — why expected hubs are *missing* from the top-10:
path-qualified Rust calls are invisible to the AST extractor
(`crate::http::build_client()` has 28 cross-file call sites in 7 files — most
in `#[cfg(test)]`, production at `state.rs:54` and in `llm.rs` — yet 0
cross-file edges); repeated in-function references collapse (`ProviderKind`
8 edges vs 63 occurrences); cross-language IPC mirrors split (`GameInfo` is
two unlinked Rust/TS nodes); and funnel-point utilities (`to_plaintext`,
`search()`) are deliberate choke points, so their low degree *confirms* the
architecture. Read low degree as extraction shape, not low centrality.

### Surprising connections (verify each still holds in code)

- `Deterministic empty-merge fallbacks` → `run_ask()` (docs/ai-workflow.html → commands.rs)
  **Holds** (`ai-workflow.html:346,409` ↔ the fallback net at
  `commands.rs:598-661`, each stage gated on empty) — dismiss.
- `Eager LLM query rewrite + merge` → `rewrite_query()` (docs/ai-workflow.html → llm.rs)
  **Holds** (`llm.rs:423`, joined concurrently with the raw search at
  `commands.rs:517`) — dismiss.
- `srwhat=text rule` → `search()` (CLAUDE.md → wiki/search.rs)
  **Holds** (`search.rs:118-125` + pinned unit tests) — dismiss; the edge
  itself dropped out of the d2e558a rebuild (ranking churn, not a code change).
- `WikiLens overlay application` ↔ `All HTTP happens in Rust` (README.md ↔ CLAUDE.md)
  **Holds and is test-enforced** (`config_guardrails.rs` iterates every
  capability file; no frontend fetch exists) — dismiss.
- Screenshot-capture plan → `crop_to_attachment()` ([[2026-07-06_screenshot-capture-to-prompt]] → capture.rs)
  **Holds** (`capture.rs:271-303` matches the plan's semantics) — but the
  check exposed the review's one real problem: CLAUDE.md never mentions the
  shipped capture feature, and its §2 map omits `capture.rs`,
  `config_guardrails.rs`, `debug.rs`, `test_support.rs`, `wiki/titles.rs`,
  `capabilities/capture.json`, `capture.html`, and four frontend files.
  **Verdict: fix** — spun out to [[2026-07-17_claude-md-capture-catchup]].

### Bridge nodes (high betweenness)

- [[2026-07-15_slow-wiki-status-hint]] bridges *Rewrite & Debug Plans* ↔
  *Testing & Hardening Plans* ↔ *Frontend IPC Bridge* (0.118) — a plan doc
  reaching directly into frontend code via `isWikiBoundStatus()`; unusual, most
  vault docs only link other docs or Rust symbols.
  **Verdict: dismiss.** The doc matches the code exactly
  (`App.tsx:76,80,241,436` + `App.slowHint.test.tsx`), and the "unusual"
  premise is false — a dozen-plus vault docs name frontend symbols; this is
  normal topology for a vault whose plans cite the symbols they specify. Not
  re-surfaced by the d2e558a rebuild.
- `isWikiBoundStatus()` bridges *Frontend IPC Bridge* ↔ *Rewrite & Debug
  Plans* (0.117).
  **Verdict: dismiss** — the same doc→symbol edge viewed from the symbol end;
  nothing about the 2-line helper suggests a split, naming issue, or gap.
- `String` (0.250) — probably type-noise; dismiss unless the review says
  otherwise.
  **Verdict: dismiss** — all 119 edges are AST type mentions from 19 Rust
  files (the node anchors at `error.rs:100`'s `impl From<AppError> for
  String`); betweenness 0.231 in the rebuild; graphify exposes no exclude
  mechanism, so there is nothing to do repo-side.

### Structure questions

- 160 weakly-connected nodes — possible documentation gaps or missing edges.
  **Verdict: dismiss — extraction granularity, not doc gaps.** 182 in the
  rebuild; the degree ≤1 population is individual test functions (~150),
  config keys (69), stdlib/external types (43), skill-script internals (34),
  npm deps (26), doc section headings (36), Props types (~15), and icons (13)
  — the report's own headline examples are this very doc's section headings.
  The only oddity (`run()` / `main()` at degree 1) is Tauri builder/macro
  wiring the AST extractor can't see.
- Cohesion 0.068 for the *Wiki Page Fetch Core* community and 0.043 for *NPM
  Package Dependencies* — split-worthy modules, or clustering artifact?
  **Verdict: dismiss — clustering artifacts.** *NPM Package Dependencies* is
  exactly a spanning tree (46 nodes / 45 intra edges — the minimum possible
  density for a connected community; a parsed manifest cannot score higher).
  *Wiki Page Fetch Core* (0.108 in the rebuild) is diluted by degree-1 test
  leaves (377 of `fetch.rs`'s 635 lines are `#[cfg(test)]`) and a merged-in
  `games.rs`; the implementation is ~258 lines behind one public entry —
  cohesive — and the sequential-fetch etiquette forecloses a
  parallelization-motivated split anyway.

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
- 2026-07-17 — **reviewed and closed.** All 13 findings verified against code
  at HEAD `2e9c525` (five triage agents plus one adversarial re-check per
  verdict; none overturned): 11 dismiss, 1 document (the god-node caveats,
  recorded inline above), 1 fix spun out to
  [[2026-07-17_claude-md-capture-catchup]] — deliberately left unexecuted
  (`status: todo`). Build drift noted while verifying: this doc transcribed
  the original build (1094 nodes / 2261 edges / 65 communities); the archived
  2026-07-17 snapshot is 1086/2243; the current d2e558a rebuild is
  1099/2255/66 — with 194→30 dangling cache edges, 124→111 collapsed
  parallels, 160→182 isolated nodes, and fetch-core cohesion 0.068→0.108.
  The counts in the finding headings are the original build's; verdicts cite
  today's equivalents.
