---
title: Fix the ask lifecycle — focus, cancel, partial answers, silent failures, AT announcements
type: plan
status: done
created: 2026-07-18
updated: 2026-07-18
tags: [frontend, rust, overlay]
related: ["[[2026-07-18_impeccable-design-context]]"]
commit: [760b1fb, 6a8e9c3, 14a6d49, 943311a, 1561080, dc010c0]
---

# Fix the ask lifecycle — focus, cancel, partial answers, silent failures, AT announcements

## Context / problem
The 2026-07-18 impeccable critique of the overlay panel (31/40, snapshot local
under `.impeccable/critique/`) found the visual system disciplined but every
significant failure living in the temporal dimension of an ask — what happens
between pressing Enter and reading the answer:
- **[P1] Focus dies on every ask** — `disabled={busy}` on the focused textarea
  drops focus to `<body>`; the next question starts typing into nothing.
- **[P1] No cancel** — sequential 12s wiki fetches + an SSE stream with no
  overall timeout can lock the product for tens of seconds; the ask-in-progress
  guard rejects new asks; Esc only hides a still-locked panel.
- **[P1] The lifecycle is silent to assistive tech** — no live region on the
  status row, no alert role on errors, selection marked by CSS class only.
- **[P2] Ctrl+Shift+C fails silently on text-only models** — the hotkey
  early-returns with no panel and no message.
- **[P2] Capital C destroys drafts** — the global Shift+C hides the panel;
  re-summon selects-all, so the next keystroke replaces the whole question.
- **Minor** — a mid-stream error hides the already-streamed partial answer
  (`{!error && answer}` gate).

## Goal / non-goals
- Goal: fix all three P1s plus the harden items in one PR: focus retention
  (readOnly during busy), a quiet Stop in the status row wired to a Rust-side
  abort, partial answers surviving mid-stream errors, the capture hotkey
  surfacing its text-only failure, an interim select-all suppression after a
  recent hide, and the cheap aria attributes (role=status/alert, aria-current).
- Non-goal: **Esc keeps its exact layered meaning** (menu close → overlay hide;
  Stop is a click/Tab affordance only). PRODUCT.md's accessibility section
  stays as-is.
- Non-goal (split to follow-up PRs by decision): the hotkey-config panel
  (settings store + key recorder — the real capital-C fix, promoting the §6
  roadmap item; first item of a growing config panel) and arrow-key menu nav +
  authored `:focus-visible` indicators (`feat/menu-keyboard-nav`).

## Approach
1. **(a) Focus** — PromptInput switches `disabled={busy}` → `readOnly={busy}`
   (focus and caret never leave); the Enter guard + `handleSubmit`'s busy check
   keep submission blocked.
2. **(b) Cancel** — per-ask `tokio_util::sync::CancellationToken` stored in
   `AppState` (`Mutex<Option<…>>`, cleared by the ask guard's Drop); `ask`
   races `run_ask` against `token.cancelled()` via `futures_util::future::select`
   so cancellation lands **mid-await** (the hung-wiki case), dropping the
   future; new `cancel_ask` command; sentinel error `wikilens::ask-cancelled`
   mirrored in `api.ts`; frontend Stop button in the status row maps the
   sentinel to a quiet reset that keeps the partial answer. DebugReport's Drop
   already reports `aborted: true` on this path — contract unchanged.
3. **(c) Partial answers** — drop the `!error` gate from AnswerView/SourceList;
   error box renders above the kept partial.
4. **(d) Capture hotkey** — new guarded `show_overlay` command (no re-fire of
   `overlay://shown` when already visible) + the existing "can't read images"
   copy as the error.
5. **(e) Interim guard** — `overlay://hidden` event from the single Rust hide
   path; re-summon within 2s keeps the draft unselected.
6. **(f1) Aria** — `role="status"` span (Stop button outside the live region),
   `role="alert"` on the error box, `aria-current` on selected menu rows.
7. Docs: CLAUDE.md command list/events/concurrency + sync-agents regen; one
   DESIGN.md §5 prose line for the Stop action (no token changes).
8. Verify: `/check`, impeccable detector still exit 0, `/vault-lint`, re-run
   `/impeccable critique the overlay panel` and confirm the P1 rows moved.

## Decisions & trade-offs
- **Future-drop cancellation over cooperative flag checks**: an AtomicBool
  checked between awaits would wait out the current 12s fetch or an idle SSE
  gap — the motivating case. Dropping the raced future cancels at any await
  point and needs zero signature changes in `fetch.rs`/`llm.rs`; safety rests
  on invariants the codebase already maintains (guards release on Drop,
  Mutexes never held across await, reqwest aborts on drop).
- **Sentinel error string over a new event/enum**: commands return
  `Result<T, String>` at the boundary; one deliberately machine-readable
  constant (`wikilens::ask-cancelled`, mirrored in `api.ts`) is the smallest
  honest contract. The frontend never renders it.
- **Capital-C: interim suppression now, config panel next** (user decision):
  the hotkey-config panel defuses the trap only for players who change the
  default, and is a PR-sized feature — it gets its own plan doc and PR; the
  ~20-line select-all suppression protects the default today.
- **(f) split** (user decision): aria attributes ride here; arrow-key nav +
  focus indicators are the largest chunk and move to `feat/menu-keyboard-nav`.

## Status log
- 2026-07-18 — created from the critique's Recommended Actions; scope fixed
  (a–e + aria) with two follow-up PRs split out; status → active.
- 2026-07-18 — **done.** All six items landed as one commit each (see
  `commit:`): readOnly focus retention with focus assertions; the Stop
  action + `cancel_ask` token race (5 new Rust tests, incl. a wiremock pin
  that a hung 30s socket aborts in ~100ms); the un-gated partial answer;
  the capture hotkey's guarded `show_overlay` + shared copy; the
  `overlay://hidden` select-suppression (2s window, fake-timer suite); the
  role=status/alert + aria-current pass. Verified: tsc, Vitest 49/49 (8 new
  frontend tests), clippy clean, cargo 235 passed, impeccable detector
  exit 0. Follow-ups queued as roadmap/next PRs: hotkey-config panel
  (the real capital-C fix), menu keyboard nav + focus indicators.
