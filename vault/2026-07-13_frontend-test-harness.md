---
title: Frontend test harness (Vitest + Testing Library + Tauri IPC mocks)
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, frontend]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-13_ci-pipeline-github-actions]]"]
commit:
---

# Frontend test harness (Vitest + Testing Library + Tauri IPC mocks)

## In simple terms
The panel's logic — streaming the answer in, juggling menus, layered Esc
behavior — has zero automated tests. A regression there type-checks fine and
ships silently; only a human clicking around would notice. This plan adds the
missing test tooling and a short, deliberate list of behavior tests. Priority 2
of the testing audit.

## Context / problem
No test script, no test dependencies, no `*.test.*` files; the only gate is
`tsc` inside `npm run build`. Verified example from [[2026-07-13_testing-audit]]:
changing the delta accumulator `setAnswer((prev) => prev + chunk)` (App.tsx) to
`setAnswer(chunk)` type-checks perfectly and breaks streaming.

## Goal / non-goals
- **Goal:** a Vitest harness (`npm test`, wired into the `/check` skill) plus
  behavior tests for the documented-fragile logic below.
- **Non-goal (explicitly not worth testing):** `api.ts` passthroughs (testing
  them tests the mock), trivial rendering (chips/badges/SourceList markup),
  autofocus and scroll-to-selected (jsdom has no layout), all CSS/panel-geometry
  gotchas (live in the window.rs/styles.css pairing, verified by running the app).

## Approach
- devDependencies: `vitest`, `jsdom`, `@testing-library/react`,
  `@testing-library/user-event`; mock IPC via `@tauri-apps/api/mocks`
  (`mockIPC`, `clearMocks` in `afterEach`). Tauri v2 note: `listen()` routes
  through `invoke('plugin:event|listen')`, so mockIPC can capture the registered
  handlers and tests fire `ask://delta` / `ask://status` directly.
- Initial worth-testing list (each anchored to a verified failure example):
  1. **Ask-flow state machine** (App.tsx `handleSubmit`): delta accumulation,
     status transitions, busy resets on both resolve and reject, attachment
     kept-on-error / cleared-on-success.
  2. **Esc layering + one-open-menu invariant:** first Esc closes the menu
     (capture-phase listener, no `hide_overlay` call), second Esc hides the
     overlay; opening a second menu unmounts the first. Guards the bare `true`
     capture flag in three separate `addEventListener` calls.
  3. **AddGameMenu stale-candidate guard:** editing the name clears probed
     candidates — otherwise a mislabeled game gets persisted to wikis.json.
  4. **localStorage robustness:** corrupted/pre-schema stored model + recent
     games don't crash first render; vision self-heal effect fires exactly one
     `list_models` and terminates.
  5. **AnswerView external-link interception:** clicking a link in an answer
     calls the opener, never navigates the webview.
- Prefer extracting pure helpers (`storedModel`, `activeModelVision`, `monogram`)
  into testable modules over heavy component tests where possible.

## Decisions & trade-offs
- `mockIPC` (official) as the default seam; fall back to `vi.mock`-ing `api.ts`
  where event choreography gets awkward (known rough edges — tauri issue #7658).
- Vitest over Jest: Vite-native, shares `vite.config`, the 2026 default.

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 2.
  No work started.
