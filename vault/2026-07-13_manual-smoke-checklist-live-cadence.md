---
title: Manual smoke checklist and live golden-suite cadence
type: plan
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [testing, overlay, hotkey, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-04_golden-query-retrieval-tests]]"]
commit: [766be60, 781fb95]
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
- 2026-07-13 — **done.** Notes against the plan as written:
  - **Checklist** shipped as `docs/smoke-checklist.md` (`781fb95`): the 8 items
    as checkbox sections plus a prerequisites block (borderless game, vision +
    text-only models for the attach step, >100% DPI monitor if available,
    dev-run vs packaged-installer split). Referenced from CLAUDE.md §2/§3
    (Release bullet now ends at the checklist) and from README's first-run
    smoke section via a maintainer pointer.
  - **Cadence** landed with its primary home in the checklist doc (the release
    ritual doc), CLAUDE.md §3 pointing at it — the three triggers as planned,
    plus a note from experience: the very first documented run hit a transient
    403 from wiki.guildwars2.com that vanished on rerun, so the doc says to
    rerun once before treating a mid-suite 403 as drift.
  - **Golden-coverage invariant** (`766be60`): the table was a function-local
    `let` inside the live runner, so it was hoisted to a shared `#[cfg(test)]
    const GOLDEN_CASES` at the `wiki` module level; `wiki/mod.rs::tests` now
    holds `every_builtin_game_has_a_golden_query` (with anti-vacuity asserts)
    plus a reverse-direction `golden_case_game_ids_are_registered` (a typo'd
    table id used to surface only when the opt-in live suite panicked). Both
    bite-checked: a commented-out case and a typo'd id each fail naming the
    exact game.
  - **Deviation:** the invariant immediately exposed conanexiles as the one
    built-in (of 15) without a golden case — fixed with a live-verified strict
    anchor ("how do I make steel" → Steel Bar/Steelfire; Steel Bar ranks #1)
    rather than an exemption. Suite is 20 cases: 18 strict OK, 2 known gaps.
    Full live run green (details in
    [[2026-07-04_golden-query-retrieval-tests]]'s log).
