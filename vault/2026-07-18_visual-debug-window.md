---
title: Visual debug window for the ask pipeline
type: plan
status: done
created: 2026-07-18
updated: 2026-07-18
tags: [frontend, tauri, rust]
related: ["[[2026-07-10_ask-debug-instrumentation]]", "[[2026-07-10_concurrent-candidate-searches-and-status]]"]
commit: [683fc23, b2d4797, 3368d0e]
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

- Goal: with `WIKILENS_DEBUG` truthy, an always-on-top debug window opens at app
  launch and visualizes each ask live: per-phase progress bars, timings/hit/page/token counts,
  collapsible detail sections, and a newest-first history of the session's asks.
- Goal (iteration 2): the window **looks and works like the overlay** — a transparent window
  drawing the same glass panel (surface/blur/border/radius/shadow tokens) — is **draggable**
  by its header, and toggles from a **Debug chip in the overlay footer** (plus the tray item).
- Goal: the window never steals keyboard focus from the game — not at creation, not on
  re-show, not on click, not on drag. Hiding it (chip, tray, Alt+F4) keeps the history.
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

### Iteration 2 — glass, drag, overlay toggle

User feedback on the shipped window: it should look and work like the overlay, be draggable,
and hide/show from an overlay button. Source-verified groundwork: tauri 2.11.5's drag script
supports `data-tauri-drag-region="deep"` (whole subtree drags; clickable children still
block); tao 0.35.3 implements start-dragging as `WM_NCLBUTTONDOWN`/`HTCAPTION` — no
activation required, so it composes with `focusable(false)`; double-click on a drag region
requests a maximize toggle, denied both by the capability and `maximizable(false)`.

1. `debug_window.rs`: builder → `transparent(true)`, `decorations(false)`, `shadow(false)`
   (CSS draws the shadow), `maximizable(false)`; keep resizable/always-on-top/skip-taskbar/
   `focusable(false)`. `APRON_*` constants (20/32/44/32 — full shadow extent on every side;
   a draggable window has no screen edge to hide a clipped shadow behind) paired by comment
   with the `.debug-panel` margin; window = panel + apron (544×704), pinned at the monitor
   **origin** (the apron is the visual gap — one geometry system). New `toggle(app)`
   mirroring `window::toggle_overlay` minus focus/emit.
2. `commands.rs` + `lib.rs`: `debug_available() -> bool` (= `debug_enabled()`; window
   existence is startup-decided) and `toggle_debug_window(app)` — the `hide_overlay`
   template. App-defined commands aren't ACL-gated, so `default.json` is untouched.
3. `capabilities/debug.json`: + `core:window:allow-start-dragging` (drag silently no-ops
   without it). Guardrail pin renamed `debug_capability_grants_only_events_and_drag` —
   keeping "listen-only" while granting a command permission would make the pin lie.
4. `src/debug/`: transparent `html, body` (black-rectangle gotcha now applies here);
   `.debug-panel` = the overlay's `.panel` recipe with the apron margin; header carries
   `data-tauri-drag-region="deep"` + `user-select: none`; ask list scrolls in an inner
   `.debug-scroll` (page scroll would drag the glass out of the window; `overflow: hidden`
   keeps the scrollbar inside the rounded corner).
5. Overlay: footer Debug chip (`.quiet-chip` in the capture cluster), rendered only when
   `debug_available`, **never disabled while busy** (mid-ask is when you want it), no
   aria-pressed (the frontend can't know visibility — tray/Alt+F4 change it too). New
   `api.ts` wrappers; `installBackend` defaults (`debug_available: () => false`) so
   existing tests stay silent; `App.debug.test.tsx` covers absent/toggles/busy-clickable.

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
- (Iteration 2) Glass parity by copying the panel tokens into the debug sheet over extracting
  a shared tokens.css: the overlay stylesheet stays byte-untouched; the debug page IS the
  third page the original decision reserved judgment for, but a tokens split is its own
  refactor — do it deliberately, not as a rider.
- (Iteration 2) Full shadow apron on all four sides (20/32/44/32) over the overlay's
  edge-docked 12px sides: a draggable window shows every shadow edge mid-screen; the cost is
  a wider invisible click-capturing ring (flag-gated dev tool — accepted, in the checklist).
- (Iteration 2) Drag via `data-tauri-drag-region="deep"` over a JS `startDragging` handler:
  no api surface for the page (it stays app-command-free), and clickable header children
  would still block dragging automatically.

## Status log

- 2026-07-18 — created; design settled (event schema, sink emission, window lifecycle,
  listen-only capability, React page + reducer). Implementation not started.
- 2026-07-18 — implemented as designed (shipped as 683fc23). `debug.rs` emits the six
  events through the injected sink with exact-key-set pin tests; `debug_window.rs`
  creates the top-left `focusable(false)` window; tray gains the flag-gated
  "Show debug panel"; `capabilities/debug.json` pinned listen-only by a new guardrail
  test; third rollup input renders the React cards (reducer + shimmer bars gated on
  `prefers-reduced-motion`). Golden stderr-table tests untouched and green. All gates
  pass: tsc, 38/38 Vitest, clippy `-D warnings`, 230/230 offline cargo tests. Runtime
  focus/DPI/tray behavior recorded as smoke-checklist item 8 (manual surface —
  the `WS_EX_NOACTIVATE` × WebView2 scroll question resolves there; fallback stays
  documented in Approach).
- 2026-07-18 — iteration 2 (user feedback on the shipped window; shipped as b2d4797):
  glass parity with the overlay (transparent undecorated window, panel recipe + full
  shadow apron via paired `APRON_*`/CSS constants, pinned at the monitor origin),
  draggable header (`data-tauri-drag-region="deep"` + `core:window:allow-start-dragging`;
  guardrail pin renamed to `debug_capability_grants_only_events_and_drag`),
  `maximizable(false)` hardening, and an overlay footer Debug chip driving the new
  `toggle_debug_window`/`debug_available` commands. Gates: tsc, 41/41 Vitest (+3 chip
  tests), clippy `-D warnings`, 230/230 offline cargo tests, 3-page build. Drag-under-
  `WS_EX_NOACTIVATE` and invisible-edge resize are the remaining empirical checks —
  smoke-checklist item 8 rewritten for the glass window.
- 2026-07-18 — start-hidden fix (user feedback; shipped as 3368d0e): the window was
  created visible, so with the flag set it appeared at launch while the overlay stayed
  hidden — backwards. `create()` now passes `visible(false)`; the window opens only via
  the Debug chip or tray, matching the overlay's start-hidden behavior. The hidden
  webview still loads and receives `debug://` events (the overlay's own hidden-listener
  mechanism), so pre-first-show asks land in the history. Smoke item 8 + README +
  CLAUDE.md reworded. Gates re-run green (tsc, 41/41 Vitest, clippy, 230/230 cargo).
