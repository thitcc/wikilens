---
title: Visual debug window for the ask pipeline
type: plan
status: todo
created: 2026-07-18
updated: 2026-07-18
tags: [frontend, tauri, rust]
related: ["[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_concurrent-candidate-searches-and-status]]"]
commit:
---

# Visual debug window for the ask pipeline

## Context / problem

The `WIKILENS_DEBUG=1` stderr table ([[2026-07-10_ask-debug-instrumentation]]) answers "where
did this ask spend its time" — but only in a terminal, and only when the ask ends
(print-on-Drop). Mid-game, with the overlay over a borderless window, nobody is watching a
console; comparing phase costs across a few asks means scrolling raw text. Wanted: a visual
surface — live progress per pipeline phase while the ask runs, then drill-down details
(queries/candidates, page titles + char counts, token usage) and a session history — without
giving up the table.

## Goal / non-goals

- Goal: with `WIKILENS_DEBUG` truthy, an always-on-top **opaque** debug window opens at app
  launch and visualizes each ask live: per-phase progress bars, timings/hit/page/token counts,
  collapsible detail sections, and a newest-first history of the session's asks.
- Goal: the window never steals keyboard focus from the game — not at creation, not on
  re-show, not on click. Closing it hides it; a tray item re-shows it.
- Goal: the new events obey `debug.rs`'s contract — never wiki text, never keys.
- Non-goal: replacing the stderr table. It stays byte-identical; the golden render tests do
  not change.
- Non-goal: persistence beyond the session; reporting pre-flight failures (they print no
  table today — the window matches); any invoke/network/fs surface for the new webview.

## Approach

1. `debug.rs` — event source. Six camelCase `Serialize` payload structs (the `AttachmentInfo`
   precedent), each carrying an `askId` from a `static NEXT_ASK_ID: AtomicU64` (the
   `capture.rs` `NEXT_ID` pattern) claimed in `DebugReport::new`: `debug://ask-started`
   (game, question, query, answer model, rewrite model only when it differs — the table's
   header rows), `debug://phase` (name, `elapsedMs: number|null` where null = skipped,
   detail), `debug://candidates`, `debug://usage` (kind `"rewrite"|"answer"`, optional
   input/output — map `llm::TokenUsage`'s fields here; don't touch `llm.rs`), `debug://pages`
   (title + chars only), `debug://finished` (totalMs, outcome, aborted) emitted from `Drop`
   alongside the `eprintln!` — `Emitter::emit_to` is synchronous, so errors, soft-fails, and
   cancelled futures all signal, mirroring the partial table. Emission via an injected
   `DebugSink` closure (`Box<dyn Fn(&str, serde_json::Value) + Send + Sync>`) attached by a
   new `attach_sink`; setters/`phase*`/`Drop` emit through it when enabled. Promote the flag
   read to `pub(crate) debug_enabled()`; add `#[cfg(test)] with_enabled(bool)` (the guardrail
   suite forbids env mutation). Public signatures unchanged.
2. New `debug_window.rs` (keeps `window.rs` overlay-only): `LABEL = "debug"`, `create(app)`
   via `WebviewWindowBuilder` → `debug.html` — 480×640 logical, resizable, decorated,
   `transparent(false)`, `always_on_top(true)`, `skip_taskbar(true)`, `focus(false)`, and
   `focusable(false)` (→ `WS_EX_NOACTIVATE`; see trade-offs). `position_top_left` mirrors
   `window::position_top_right`'s DPI discipline (monitor origin + scale factor); top-left
   cannot overlap the right-docked overlay. Plus `show(app)` and
   `sink(app) -> Option<DebugSink>` wrapping `emit_to(LABEL, …)`.
3. `commands.rs` — exactly one new line after `DebugReport::new`: attach the sink.
4. `lib.rs` `.setup()`: `if debug::debug_enabled() { debug_window::create(...) }` — window
   existence is startup-decided (env can't change mid-process, so it always agrees with the
   per-ask read). Close is already handled: the global `CloseRequested` handler hides any
   window, and the hidden webview keeps receiving events. `tray.rs`: append a
   "Show debug panel" item + handler arm only when the flag is on.
5. New `capabilities/debug.json`: `windows: ["debug"]`, permissions
   `["core:default", "core:event:default"]` — listen-only, strictly smaller than capture's
   (the page invokes no commands). No `tauri.conf.json` or CSP change.
   `config_guardrails.rs`: the `capabilities/*.json` glob auto-covers the http/fs pin; add
   `debug_capability_stays_listen_only` pinning the window list + permission subset.
6. Frontend — third Vite rollup input (`debug.html` → `src/debug/main.tsx`), React because
   the page is list-rendering over live state and the Vitest harness then applies for free.
   `src/debug/events.ts` is this bundle's own typed IPC boundary (per-bundle precedent:
   `src/capture/main.ts`); `askState.ts` is a pure reducer (live ask = entry without
   `finished`; history capped ~25); `DebugApp.tsx` + `AskCard.tsx` (header, phase table,
   three collapsibles: queries/candidates, pages, tokens — reusing the
   `.group-header`/`.group-caret` pattern); own `styles.css` copying ~15 token values plus an
   opaque `--surface-window` so the pixel-paired overlay stylesheet stays byte-untouched.
   Liveness: the page also listens to the existing `ask://status` broadcast for a provisional
   "running" row (`searching` fires before `ask-started`, so buffer the last status);
   completed `debug://phase` rows are authoritative and replace it. Bars: running phase =
   indeterminate shimmer (first `@keyframes` in the codebase — gate behind
   `prefers-reduced-motion`); completed = width `elapsedMs / max(elapsedMs so far)`, so the
   widest bar is the slowest phase; skipped = muted `—`. TS mirrors in `src/types.ts`.
7. Tests. Rust: payload field-set pins (exact camelCase key sets — the structural
   no-text/no-keys guarantee) and emission order/gating via a recording sink +
   `with_enabled` (ask-started on attach; per-setter events; drop-without-finish →
   `finished{aborted: true}`; disabled → zero; no sink → no panic). Frontend:
   `askState.test.ts` (happy path, mid-ask abort, history cap, status-before-header
   buffering) and `DebugApp.test.tsx` driven purely by `fireBackendEvent`. Manual additions
   to `docs/smoke-checklist.md`: window top-left opaque above the borderless game;
   click/scroll never steals keyboard focus; bars animate then settle; stderr table
   unchanged; close hides / tray re-shows without activating; no flag → no window, no tray
   item; DPI re-check on a scaled monitor.
8. Docs: CLAUDE.md §2 (tree entries, three rollup inputs), §4 (`debug://…` events bullet +
   "Debugging an ask" update), §5 (two gotchas: tao's `show()` issues an activating
   `SW_SHOW`, so `focusable(false)` is load-bearing — `focus(false)` only covers creation;
   `emit_to("debug", …)` is a silent no-op when the window doesn't exist). README: short
   "Debugging" note near (not inside) the retrieval-tuning table.

Open questions / risks: `WS_EX_NOACTIVATE` × WebView2 wheel-scroll and text selection is the
one empirical unknown — fallback is `focusable(true)` + `focus(false)`, accepting focus-steal
on tray re-show only. First-ask page-load race (no event replay buffer) accepted; if it bites,
add a small history in `AppState` + a `debug_history` command. An optional `ask://delta`
streamed-chars ticker (lengths only) is nice-to-have; drop it if payload discipline should
stay maximally clean.

## Decisions & trade-offs

- Separate window over an in-overlay drawer (user pick): doesn't compete with the panel's
  height budget, survives across asks, and stays readable while the overlay is hidden.
- Sink closure inside `DebugReport` over per-call-site emits in `commands.rs`: only `Drop`
  can signal aborts after `?` propagation; zero new per-phase call sites; the
  no-text/no-keys rule stays enforceable in one module; `debug.rs` stays tauri-free and
  testable with a recording closure.
- Dynamic window creation in `.setup()` over a static `tauri.conf.json` entry: no third
  webview cost for non-debug users, and existence ≡ flag truthy by construction.
- Opaque decorated window over glass/transparent: sidesteps the black-rectangle and
  transparent-gap-mouse gotchas and any coupling to the paired geometry constants.
- Copied style tokens over extracting a shared `tokens.css`: keeps the comment-paired overlay
  stylesheet untouched; revisit if a third page appears.
- Liveness from the existing `ask://status` over new `debug://phase-started` emissions:
  coarser (one `searching` covers the joined raw search + rewrite) but zero new backend call
  sites; completed rows land seconds later and are authoritative.

## Status log

- 2026-07-18 — created; design settled (event schema, sink emission, window lifecycle,
  listen-only capability, React page + reducer). Implementation not started.
