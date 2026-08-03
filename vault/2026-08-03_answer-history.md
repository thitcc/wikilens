---
title: Answer history — recall past answers from a header menu
type: plan
status: done
created: 2026-08-03
updated: 2026-08-03
tags: [frontend, rust, tauri, overlay]
related:
  - "[[2026-07-05_game-picker-owned-menu]]"
  - "[[2026-07-18_visual-debug-window]]"
  - "[[2026-07-19_menu-keyboard-nav]]"
  - "[[2026-08-03_overlay-hug-and-clear]]"
commit: [dfbcdd5, 20f2459, 5a61b60, 2430650]
---

# Answer history — recall past answers from a header menu

## Context / problem

A finished answer lives only in React state: the next submit and Start over
wipe it (App.tsx handleSubmit / handleClear), and a restart loses everything —
players re-ask questions they already asked. "Answer history" has been on the
roadmap since the scaffold. Scope fixed with the owner: past Q&A history
(one-shot asks stay one-shot), persisted across restarts, opened from a
header chip + owned menu.

## Goal / non-goals

- Goal: after an answered ask, a History icon-chip in the header (beside the
  gear; rendered only when history is non-empty, disabled while busy) opens
  an owned menu of past questions — newest first, filterable; picking one
  instantly restores that answer + sources + question (no re-ask, no status
  phases) and switches to the entry's game when it still exists; history
  survives restarts in `history.json` (app-data), capped at 50 entries, with
  a pinned "Clear history" footer action.
- Non-goal: multi-turn threads; recording no-result / pages-unreadable /
  cancelled asks; per-entry delete (clear-all only); persisting wiki page
  text, token usage, or the screenshot (presence flag only); showing vendor
  model ids in Default mode.

## Approach

1. **Store** — new `src-tauri/src/history.rs`: `HistoryStore` cloning the
   `wiki/user.rs` pattern exactly (load never fails: missing→empty,
   corrupt→`.bak`+empty, unreadable→refuse-mutations; clone-mutate-persist-
   commit under one write lock; tmp+atomic-rename persist). Bare newest-first
   JSON array like `wikis.json`. `HistoryEntry` (one struct, disk + IPC,
   camelCase): `id` (store-minted `{unix_millis}-{seq}`), `createdMs`
   (SystemTime — no chrono dep), `gameId`, `gameName` (denormalized —
   survives game removal), `question`, `answer` (LLM markdown), `sources`
   (title+url; derive `Deserialize` on `commands::Source`), `providerName`
   (`targets.answer.name` — "Default" in Default mode), `model`
   (`null` in Default mode: vendor ids stay dev-only), `hadImage`.
   `HISTORY_CAP = 50` (front-insert, truncate). `error.rs` sibling variant
   `History("Couldn't save your answer history: {0}")`. Manage in `lib.rs`
   setup after the keys store, strictly before the window-creation loop
   (pinned boot-race order).
2. **Commands** — `list_history` (plain `Vec<HistoryEntry>`, newest first,
   like `list_games`) + `clear_history`; register in `generate_handler`.
   `ask` gains the `HistoryStore` handle (update its "five managed handles"
   doc comment). Best-effort append at the `run_ask` success tail
   (commands.rs:1115-1133) — `eprintln!` on failure, never fails the ask;
   the two canned-answer returns and cancelled asks skip by construction.
   No `record_history` command — the webview can't forge entries.
3. **Frontend** — `types.ts`/`api.ts`: `HistoryEntry`, `listHistory`,
   `clearHistory`. New pure `src/historyTime.ts` `relativeTime(thenMs,
   nowMs)` ("just now"/"Nm ago"/"Nh ago"/"Nd ago"/"Jul 30"). New
   `src/components/HistoryMenu.tsx` on the GameMenu contract (`.menu
   menu--top` dialog, `.menu-search` filter over question text,
   menuNav/menuScroll keyboard model, capture-phase Esc, pinned Clear
   footer); rows = question one-liner + muted "game · relative time" meta;
   no tiles, no selected state, no chat styling. `App.tsx`: `openMenu` /
   `openMenuRef` unions gain `"history"` (70% height pinning comes free);
   `history` state fetched on mount + refreshed fire-and-forget in
   `handleSubmit`'s `finally`; icon-chip in `.brand-cluster`;
   `handleHistoryPick` bumps `askEpochRef` (a draining delta must not append
   onto the restored answer), sets question/answer/sources, clears
   error/status/announcement/slowHint, leaves `attachment`, switches game
   via `handleGameChange` when it still exists, `closeMenu()`.
4. **Tests** — Rust: `history.rs` suite mirroring `user.rs:146-260` (missing/
   append+reload/cap-drops-oldest/clear/corrupt-backup/unreadable-refuses/
   unique ids); `history_model` unit (Default → None); guardrail pin
   `history_entry_serializes_exactly_the_known_fields` (+ nested source set).
   Frontend: `backend.ts` fixtures with `list_history: () => []` default;
   `App.history.test.tsx` (chip gating, record→refresh, restore, removed-game
   pick, clear, busy-disable, epoch gate after Stop);
   `HistoryMenu.test.tsx`; `historyTime.test.ts`.
5. **Docs** — CLAUDE.md §2 map (`history.rs`, `HistoryMenu.tsx`,
   `historyTime.ts`, commands list) + §6 roadmap line removal; smoke
   checklist §6 recall box + §10 survives-restart box; DESIGN.md frontmatter
   only if a row-meta token lands (menu-card/menu-row/icon-chip all reused).

One PR (`feat/answer-history`), steps in order, `/check` before push.

## Decisions & trade-offs

- Recording is Rust-side at the success tail: Rust owns the clock and id,
  cancelled asks skip for free (the future is dropped before the tail), and
  an answer the frontend dropped via the Start-over epoch is still preserved.
  Best-effort — a failed history write never fails the ask.
- Q&A text persists as plaintext JSON: wiki-derived game content, not secret
  material (keys stay DPAPI). The debug contract still holds — no wiki page
  text on disk, sources are title+url only.
- Bare array, no version envelope (unlike `keys.json`): worst case is losing
  convenience data; the corrupt→`.bak` path is the net.

## Status log

- 2026-08-03 — created (todo); scope fixed with owner: Q&A history,
  persisted, header chip + owned menu. Implementation not started.
- 2026-08-03 — approved; implementation started on `feat/answer-history`.
- 2026-08-03 — implemented (dfbcdd5 + docs 20f2459); all gates green
  (tsc, 190 Vitest, 38 node, clippy -D warnings, 306 cargo). Adversarial
  review follow-ups: the clear failure now uses the `.menu-error` voice
  (5a61b60) and the design sidecar mirrors `history-row-meta` (2430650).
  Capability note: app commands aren't per-webview gated in this Tauri
  setup (pre-existing, applies to all commands alike); the forge guard is
  the absence of any record command. Done.
