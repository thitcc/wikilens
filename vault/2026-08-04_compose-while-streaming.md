---
title: Compose the next question while an answer streams
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [frontend, overlay]
related: []
commit:
---

# Compose the next question while an answer streams

## Context / problem

While an answer streams, the prompt is tied up with the in-flight ask — the
player who already knows their next question waits for the stream (or stops
it) before they can even start typing. Wiki sessions chain questions; the
pause between them is dead time.

## Goal / non-goals

- Goal: the prompt accepts typing while an answer streams; submitting
  resolves sensibly against the one-ask-at-a-time rule.
- Non-goal: concurrent asks — `AppState::ask_in_progress` stays; this is
  about composing, not parallelism.

## Approach

Sketch: decouple the input's enabled state from the ask lifecycle in
`App.tsx`, then decide the submit semantics — queue the question for when
the stream settles, or cancel-and-ask like the status row's Stop. The
select-all-on-show and history-restore interactions need a look once typing
can overlap streaming.

## Decisions & trade-offs

The submit semantics (queue vs cancel-and-ask) is the real fork here; split
it into a decision doc when work starts.

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
