---
title: Manual smoke checklist and live golden-suite cadence
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, overlay, hotkey, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-04_golden-query-retrieval-tests]]"]
commit:
---

# Manual smoke checklist and live golden-suite cadence

## In simple terms
Some things a robot can't test: a translucent panel floating over a real game on
a real monitor, a global hotkey, a tray icon. For those, the honest control is a
short written checklist executed before each release — so verification is
repeatable instead of vibes. Separately, the live golden-query tests only run
when someone remembers; this plan gives them a schedule. Priority 6 of the
testing audit.

## Context / problem
`lib.rs`, `window.rs`, `tray.rs`, hotkey registration, and the capture window
flows genuinely require a running Tauri app — the audit correctly wrote them off
for unit tests but no compensating control exists. And the 10 `#[ignore]`d live
tests (including the 19-case golden-query hit@4 suite from
[[2026-07-04_golden-query-retrieval-tests]]) have no run cadence, so wiki drift
(endpoint dies, articlepath changes, ranking shifts) is invisible between ad-hoc
runs. See [[2026-07-13_testing-audit]].

## Goal / non-goals
- **Goal:** a written, repeatable smoke checklist for the runtime-only surface;
  a documented cadence for the live suites; and an offline test enforcing
  "every built-in game has a golden query".
- **Non-goal:** WebDriver E2E automation (explicitly skipped by the audit) or
  automating the checklist itself.

## Approach
- **Checklist doc** (lives in the repo, e.g. `docs/smoke-checklist.md` — not the
  vault; it's a dev guide, not a plan):
  1. Shift+C toggles the overlay over a borderless game; tray Show/Hide works.
  2. Capital `C` cannot be typed while registered (expected; lowercase works).
  3. Panel placement correct on each monitor incl. a >100% DPI display.
  4. No black rectangle (transparency regression), shadow not clipped.
  5. Menus open/close; Esc layering: first Esc closes menu, second hides panel.
  6. Ask round-trip streams an answer with sources; links open in the system
     browser; screenshot attach works on a vision model, blocked on text-only.
  7. Exclusive-fullscreen covers the overlay (expected failure — verify it's
     still documented, not "fixed").
  8. **Packaged build** (`npm run tauri build` artifact): installer launches,
     keys resolve from OS env vars (`.env` is dev-only), links still open
     (opener scope present at runtime).
- **Cadence:** run `cargo test -- --ignored` (a) before each release, (b) when
  adding/changing a game or provider, (c) roughly monthly otherwise; record
  known-gap flips in the golden suite's doc.
- **Golden-coverage invariant:** an offline `#[test]` asserting every id in
  `wiki/games.rs` appears in the golden-query table — enforces the CLAUDE.md
  "anchor each new game with a golden query" rule so it can't silently rot.

## Decisions & trade-offs
- Checklist in `docs/`, referenced from CLAUDE.md — vault docs are plans/records,
  the checklist is a living dev guide.
- Cadence is calendar + trigger based, not CI-scheduled: live suites need the
  network and polite request pacing; a scheduled CI job hitting fan wikis
  monthly buys little over a documented habit here.

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 6.
  No work started.
