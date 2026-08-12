---
title: The theme pick persists in localStorage, not settings.json
type: decision
status: done
created: 2026-08-12
updated: 2026-08-12
tags: [frontend, overlay]
related:
  - "[[2026-08-12_theme-switching-micrographics]]"
  - "[[2026-08-12_chrome-only-themes]]"
commit: "078bd20"
---

# The theme pick persists in localStorage, not settings.json

## Context

The theme pick must survive restarts, and two persistence homes exist. The
`SettingsStore` (`settings.rs`, `settings.json` in app-data) is doc-commented
as the future config-panel home and already carries the hotkeys and the mode
choice — but adding a field there costs a `settings.rs` field, a command,
`api.ts`, `types.ts`, and an IPC round trip per pick. localStorage is the
established home for every UI pick the frontend owns: the game, the
provider, the model (`wikilens.*` keys, corruption-tolerant readers, lazy
`useState` initializers).

## Decision

The theme persists in localStorage under `wikilens.theme` via `src/theme.ts`,
whose corruption-tolerant reader returns the default itself — a theme is a UI
pick, not behavior config.

## Consequences

- Good: zero Rust/IPC surface (pinned — the Settings theme rows fire no IPC,
  `SettingsMenu.test.tsx` / `App.theme.test.tsx` assert it); the pick is
  same-origin-shared across the three pages for free if scope ever grows;
  one storage idiom for every UI pick. Unlike the game/model readers there
  is no backend list to reconcile against, so the reader owns the default —
  the simplest storage citizen in the app.
- Cost / bad: the pick lives outside the `settings.json` envelope, so a
  future settings export/import or Rust-side config surface would not carry
  it without a migration.
- Follow-ups: if a theme ever needs to reach the debug window (separate
  document, own stylesheet), localStorage being same-origin makes the read
  trivial — the blocker there is the hand-copied `:root`, not persistence
  (the shared-tokens.css trigger in `src/debug/styles.css`'s header).

## Alternatives considered

- `settings.json` via `SettingsStore` — the envelope is the declared future
  config home, but a theme is chrome, not behavior: Rust never reads it, so
  the field + command + IPC would buy nothing today. Rejected; revisit only
  if settings export/import becomes real.
