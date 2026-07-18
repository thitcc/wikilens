---
title: Catch CLAUDE.md up with the shipped screenshot-capture feature
type: plan
status: done
created: 2026-07-17
updated: 2026-07-17
tags: [capture]
related:
  - "[[2026-07-06_screenshot-capture-to-prompt]]"
  - "[[2026-07-17_graphify-findings-review]]"
commit: d2057c8
---

# Catch CLAUDE.md up with the shipped screenshot-capture feature

## Context / problem

The graphify findings review ([[2026-07-17_graphify-findings-review]]) verified
the graph's "surprising connection" from the screenshot-capture plan to
`crop_to_attachment()` — the edge holds (`capture.rs:271-303` matches the plan's
step-4 semantics exactly) — but the check exposed real doc drift: **CLAUDE.md
was never told about the capture feature.**
[[2026-07-06_screenshot-capture-to-prompt]] shipped in `88facae` (2026-07-06)
with no CLAUDE.md step in its plan (sibling plans normally carry one), and
CLAUDE.md has been edited three times since without picking it up. Today it has
zero capture-feature mentions, and the §2 architecture map omits several files
that exist on disk.

Missing from the §2 map — backend: `src-tauri/src/capture.rs`,
`config_guardrails.rs`, `debug.rs` (in §4 prose only), `test_support.rs`,
`wiki/titles.rs`, `capabilities/capture.json` (map names only `default.json`),
root `capture.html`. Frontend: `src/capture/main.ts`,
`src/components/Badge.tsx`, `src/menuPlacement.ts` (in §5 gotchas only),
`src/menuScroll.ts`, plus the colocated `*.test.*` files. Also stale: §2's
`commands.rs` line omits `begin_capture` / `finish_capture` / `cancel_capture` /
`clear_capture` (registered in `lib.rs:86-89`); §4's namespaced-events list
omits `capture://hotkey` (`lib.rs:54`), `capture://armed` (`capture.rs:183`),
`capture://attached` / `capture://error` (`commands.rs:277,280`); the
`state.rs` one-liner omits the pending-shot / attachment / title-cache state
(`state.rs:32-40`); the `hotkey.rs` one-liner says Shift+C only, though
`hotkey.rs:29-30,46` also registers Ctrl+Shift+C for capture.

## Goal / non-goals

- Goal: make CLAUDE.md §1/§2/§4 match the real source tree and the shipped
  capture feature, so both agents (Claude and Codex) stop planning against a
  map that predates it.
- Non-goal: any code or behavior change — docs only.
- Non-goal: re-listing retrieval env vars (README's "Retrieval tuning" table
  stays the single source, per §4).
- Non-goal: editing the closed capture plan docs (filenames and histories are
  permanent).

## Approach

1. Re-diff the §2 map against a fresh Glob of `src/`, `src-tauri/src/`,
   `src-tauri/capabilities/`, and the repo root at execution time (the file
   list above is the 2026-07-17 snapshot); add one-line map entries in the
   existing comment style, and note the colocated `*.test.*` files once rather
   than per-file.
2. Extend the §2 `commands.rs` line with the four capture commands.
3. Add the `capture://` events (`hotkey`, `armed`, `attached`, `error`) with
   payloads to §4's namespaced-events list.
4. Refresh the `state.rs` one-liner (pending-shot / attachment / title-cache
   state) and the `hotkey.rs` one-liner (Ctrl+Shift+C capture shortcut), and
   add a one-sentence §1 mention that a screenshot region can be attached to
   the prompt.
5. Run `node .claude/skills/sync-agents/generate.mjs` (Codex reads CLAUDE.md
   directly) and land via PR per the delivery workflow; `/vault-lint` on close.

## Decisions & trade-offs

- This doc is the missing "update CLAUDE.md" step the capture plan never
  carried — recorded as its own plan rather than folded into the findings
  review, so the review could close while this work stays deliberately
  unexecuted (`status: todo`).
- Keep the map at one line per file, matching the existing style; the capture
  flow's detail lives in the closed capture plan trilogy, not in CLAUDE.md.

## Status log

- 2026-07-17 — created from the graphify findings review (the `conn-capture`
  verdict, the review's only **fix**); work deliberately not started.
- 2026-07-17 — **executed and closed.** CLAUDE.md caught up per the approach:
  §1 gained the Ctrl+Shift+C capture sentence; the §2 map gained entries for
  every missing file (capture flow, test-only Rust modules, menu helpers,
  `wiki/titles.rs`, `Badge.tsx`, both capability files, `capture.html` +
  `src/capture/main.ts`) plus refreshed one-liners for `tauri.conf.json`,
  `state.rs`, `hotkey.rs`, and the four capture commands on `commands.rs`;
  §4 gained the four `capture://` events with payloads (including the
  thumbnail-only-over-IPC property). Colocated `*.test.*` suites noted once
  on the `src/` line. Re-diffed against a fresh glob at execution time — the
  2026-07-17 snapshot list was still accurate, plus two adversarial
  doc-vs-code agents verified the new text. Codex adapters regenerated via
  sync-agents; landed via PR.
