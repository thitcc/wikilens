---
title: Show a hint when the wiki, not WikiLens, is slow
type: plan
status: todo
created: 2026-07-15
updated: 2026-07-15
tags: [wiki, frontend]
related:
  - "[[2026-07-10_concurrent-candidate-searches-and-status]]"
  - "[[2026-07-10_retrieval-path-timeouts]]"
commit:
---

# Show a hint when the wiki, not WikiLens, is slow

## Context / problem

A live ask on 2026-07-15 ("what is a good strategy for winter?", Stardew) took
56 s — 46.6 s of it the fetch phase, four sequential `action=parse` requests
each crawling in at ~11.6 s, just under the 12 s `PARSE_TIMEOUT`. Re-probing
the same requests minutes later: 0.7–1.0 s each, and the search request was
7× faster too. The wiki (or the route to it) was transiently slow; WikiLens
was fine. But during such a stall the overlay silently shows "Reading pages…"
for up to ~48 s (worst case: 4 × 12 s timeouts), and the player can't tell
whether the app is broken. Tell them the slowness is the wiki's fault.

## Goal / non-goals

- Goal: when a single wiki-bound ask phase stalls past a threshold, show a
  muted one-line hint under the status line attributing the wait to the wiki.
- Non-goal: speeding the fetch up (page fetches stay sequential per MediaWiki
  etiquette; a total-fetch budget or the roadmap's SQLite page cache are
  separate work).
- Non-goal: any backend/IPC change (see trade-offs).

## Approach

Frontend-only, all in `src/App.tsx` + one CSS rule. Design validated against
the code on 2026-07-15:

- **Constant** `SLOW_WIKI_HINT_MS = 10_000`, module scope in `App.tsx`,
  exported for tests. Above the healthy worst cases (search 0.3–3 s, 4-page
  fetch 2–8 s) so a healthy ask never sees the hint; below the 12 s
  per-request `PARSE_TIMEOUT` so it fires before even one stalled parse
  resolves. One global threshold — per-status values buy ~5 s at the cost of
  a config table and a bigger test matrix.
- **Helper** `isWikiBoundStatus(s: AskStatus)` — true for every status except
  `"answering"` (that phase is the LLM streaming, not the wiki). Lives next
  to `STATUS_LABEL`; one consumer, no new module. Caveat: the "searching"
  phase also joins the LLM rewrite, but the wiki search is the plausible
  10-second pole (the rewrite is a one-line completion with its own timeout).
- **State + effect**: `slowHint` boolean + a `useEffect` keyed on
  `[status, busy]`: reset `slowHint`, and when busy on a wiki-bound status arm
  a `window.setTimeout(…, SLOW_WIKI_HINT_MS)`, cleared in the effect cleanup.
  Per-phase reset semantics — a cumulative timer would false-positive across
  several healthy phases. Known-good edges: duplicate same-value emits (the
  optimistic `setStatus("searching")` on submit + the backend's re-emit)
  don't reset the clock because React bails on identical state; ask
  end/error disarms via the existing `finally` (`busy=false, status=null`);
  StrictMode double-effects are safe via the cleanup. First timer in `src/` —
  no `setTimeout` exists there today.
- **JSX**: hint nested *inside* the existing `.status` div (so `.content`'s
  flex gap doesn't double), gated on `slowHint && isWikiBoundStatus(status)`.
- **CSS**: `.status-hint` under `.status` in `styles.css`, existing tokens
  only — `--fg-muted`, `--text-label`, `--leading-compact`,
  `margin-top: var(--space-4)`. No new tokens.
- **Copy**: `The wiki is responding slowly — this isn't WikiLens.`
- **Tests**: new colocated `src/App.slowHint.test.tsx` — kept out of
  `App.ask.test.tsx` so the fake-timer regime doesn't leak into the
  real-timer tests. First fake-timer use in the repo; gotchas:
  `vi.useFakeTimers()` + `afterEach(vi.useRealTimers)`,
  `userEvent.setup({ advanceTimers: vi.advanceTimersByTime })`, advance
  inside `act`, gate the ask with `deferred<AskResult>()` and drive phases
  via `fireBackendEvent` (existing harness). Four cases: hint appears at N
  and not before (and a duplicate emit doesn't reset); timer resets on
  status change; never shows under "answering" and clears on transition into
  it; cleared when the ask resolves or errors.
- **Shape when implemented**: one PR off `main`, branch
  `feat/slow-wiki-status-hint`; touches `src/App.tsx`, `src/styles.css`, new
  test file.

## Decisions & trade-offs

- **Frontend timer over a backend mid-phase event.** Every `ask://status` is
  emitted at a phase boundary immediately before one awaited call, so phase
  elapsed time is fully derivable client-side. A backend "still fetching"
  event would need the first mid-phase emit outside the delta stream (racing
  the await with a sleep — a new pattern in `run_ask`) and new IPC surface,
  for zero extra fidelity at 10-second granularity. Implementation-level
  call, recorded here rather than in a decision doc.
- **Hint is diagnostic copy, not a progress verb** — no ellipsis, matches the
  em-dash two-clause house style of existing hint lines.

## Status log

- 2026-07-15 — created; design captured from the slow-ask investigation, work
  deliberately deferred (`status: todo`).
