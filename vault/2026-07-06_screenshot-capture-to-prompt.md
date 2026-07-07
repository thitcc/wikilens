---
title: Screenshot region capture attached to the prompt
type: plan
status: done
created: 2026-07-06
updated: 2026-07-06
tags: [capture, tauri, rust, overlay, hotkey]
related: ["[[2026-07-06_model-vision-badges]]", "[[2026-07-06_image-attach-guardrails]]"]
commit: 88facae
---

# Screenshot region capture attached to the prompt

## In simple terms

Press Ctrl+Shift+C (or click the new Capture chip in the panel footer): the
glass panel slips away, the screen dims under a crosshair, you drag a box
around the thing you're asking about, and the panel comes back with that crop
pinned under the prompt as a small thumbnail. Ask the question; the screenshot
rides along to the model, so "what is *this*?" finally has a this.

## Context / problem

Wiki questions are often about something on screen the player can't name — an
unidentified item, an enemy, a UI panel. Today the prompt is text-only.

Research settled the approach (2026-07-06, 17-agent verified workflow):

- **OS snipping is ruled out on Windows.** The legacy `ms-screenclip:` URI is
  formally deprecated ("no longer supported"), was always clipboard-only with
  no completion or cancel signal, and the new documented Snipping Tool
  protocol (status codes incl. 499-cancel + file token) requires an
  MSIX-packaged app — Tauri ships MSI/NSIS. Clipboard harvesting is a
  heuristic, not a contract.
- **Owned capture is proven and small.** Real Tauri v2 apps (Snap2Link,
  ai-lens) ship the exact pattern: pre-created hidden transparent fullscreen
  window over the live desktop, monitor snapshot frozen in memory at trigger
  time, CSS drag-rect, Rust crop. ~400–800 LOC total. It also matches our
  owned-menu philosophy (never OS-drawn UI over the game) and ports later
  (macOS/X11 unchanged via xcap; Wayland needs a portal backend — keep the
  capture backend swappable).

First of three sibling plans; independent of
[[2026-07-06_model-vision-badges]]; [[2026-07-06_image-attach-guardrails]]
lands last and consumes this doc's attachment state.

## Goal / non-goals

- **Goal — capture pipeline:** pre-declared hidden capture window + `xcap`
  monitor snapshot; drag-select a region of the live screen; Rust crops,
  downscales (long edge ≤ 1568 px), PNG-encodes, and holds the result.
- **Goal — two triggers, one flow:** a `.quiet-chip` footer button and a
  global `Ctrl+Shift+C` hotkey both funnel into `handleCaptureRequest()` in
  App.tsx; both end with the panel shown and a thumbnail attached.
- **Goal — attach to ask:** `ask` gains an optional image; Anthropic gets a
  base64 `image` block, OpenAI-compatible providers get an `image_url`
  data-URI part, image before text; the text-only request stays byte-identical
  to today.
- **Goal — lifecycle:** single attachment max; new capture replaces old;
  cleared after a successful answer; kept on error; removable via "×".
- **Goal — DPI/multi-monitor correctness:** crop math in physical pixels,
  `phys = round(css × devicePixelRatio)`, monitor-local coordinates.
- **Non-goal — cross-monitor selection:** the region is bound to the monitor
  under the cursor at trigger time (one webview can't span mixed-DPI monitors
  correctly; game + panel live on one monitor anyway).
- **Non-goal — multiple attachments, annotation, disk persistence:** the image
  lives only in memory and dies with the ask/clear.
- **Non-goal — friendly non-vision errors or button gating:** see
  [[2026-07-06_image-attach-guardrails]]. Here a DeepSeek 400 shows raw in the
  existing error box (acceptable interim).
- **Non-goal — exclusive-fullscreen games:** GDI BitBlt captures
  borderless/windowed fine; exclusive fullscreen may black-frame or minimize
  on focus loss. Documented limitation, same as the overlay itself.

## Approach

1. **Cargo.toml** — add `xcap = "0.9"` (maintained successor of
   `screenshots`), `image` (png feature), `base64`; tokio `time` feature for
   the settle sleep.
2. **tauri.conf.json** — pre-declare a second window: `label: "capture"`,
   `url: "capture.html"`, transparent / alwaysOnTop / skipTaskbar /
   decorations:false / visible:false. Pre-declared because runtime
   WebviewWindow creation inside a command deadlocks on Windows. No CSP
   change — `img-src 'self' data:` already admits the thumbnail.
3. **capabilities/capture.json (new)** — minimal capability scoped to
   `windows: ["capture"]`: `core:default`, `core:event:default`, window
   hide/show/set-focus. The overlay capability stays untouched.
4. **src-tauri/src/capture.rs (new, ~220 LOC)** — the pipeline:
   `PendingShot { image: RgbaImage, monitor_x, monitor_y }` frozen at trigger
   (zero encode on the hot path); `Attachment { id, png, width, height }`;
   `AttachmentInfo { id, thumb_uri, width, height }` (camelCase serde;
   ~10–30 KB thumbnail data-URI); `CropRect { x, y, w, h, dpr }`.
   `begin()`: reject if an ask or capture is in progress → hide overlay →
   ~120 ms settle → cursor position → `Monitor::from_point` →
   `capture_image()` (blocking xcap call — run via `spawn_blocking`) →
   position the capture window with physical coords/size (negative origins
   allowed) → show + focus. `crop_to_attachment()` is a pure function —
   the classic DPI bug (skipping the `dpr` multiply, breaks at 125–150 %
   scaling) is pinned by unit tests. `close_capture_ui()` hides the capture
   window, clears the pending shot, re-shows the overlay.
5. **state.rs** — `pending_shot: Mutex<Option<PendingShot>>` and
   `attachment: Mutex<Option<Attachment>>` on `AppState` (same
   never-across-await Mutex discipline as the model cache).
6. **commands.rs** — `begin_capture`, `finish_capture(rect)` (crop → store
   attachment, replacing any old one → close UI → `emit_to("overlay",
   "capture://attached", info)`; on error close UI + `capture://error`),
   `cancel_capture`, `clear_capture`. `ask` gains `image_id: Option<String>`
   (id mismatch → "That screenshot expired — capture it again."); the slot is
   cleared **only after a successful answer**, enforcing clears-on-success /
   stays-on-error at one point.
7. **llm.rs** — `answer_streaming(…, image_png: Option<&[u8]>)`; new
   `build_user_content(kind, question, pages, image_png)`: `None` → the
   existing plain string (zero regression); `Some` → Anthropic
   `[{type:"image", source:{type:"base64", media_type:"image/png", data}},
   {type:"text", …}]`, OpenAI-compatible `[{type:"image_url",
   image_url:{url:"data:image/png;base64,…"}}, {type:"text", …}]` — image
   before text. SYSTEM_PROMPT (spec guardrail) stays verbatim; a
   `SCREENSHOT_ADDENDUM` const is appended to `system` only when an image is
   attached:
   > The player has attached a screenshot of their game. Use it only to
   > identify what the question is about — the item, enemy, location, or
   > situation shown — and then answer from the wiki excerpts as usual. The
   > excerpts remain your only source of facts. If the screenshot shows
   > something the excerpts do not cover, say plainly that the wiki text
   > provided doesn't cover what's on screen, and suggest what to search
   > instead. Do not describe the screenshot back to the player unless they
   > ask.
   MAX_TOKENS unchanged (images cost input tokens, not output).
8. **window.rs** — extract `show_overlay()` from `toggle_overlay`'s
   else-branch (position + show + focus + `overlay://shown`); reused by
   `close_capture_ui`, so the re-shown panel auto-refocuses the prompt via
   the existing event handler.
9. **hotkey.rs** — `capture_shortcut()` = Ctrl+Shift+C and
   `is_capture_shortcut()` via the documented extension seam; register both,
   each independently non-fatal (tray tooltip on failure). Never a bare
   Shift+letter (the Shift+C gotcha).
10. **lib.rs** — the shortcut handler branch emits `capture://hotkey` to the
    overlay webview instead of calling `capture::begin` directly: the
    frontend stays the single entry point, so the guardrails doc's gating
    needs zero Rust changes. Register the four new commands. The existing
    CloseRequested-hides handler already covers the capture window.
11. **vite.config.ts + capture.html + src/capture/main.ts (new, vanilla TS
    ~120 LOC, no React)** — `rollupOptions.input` with two entries. Crosshair
    cursor; drag rect with `box-shadow: 0 0 0 9999px rgba(0,0,0,.45)`
    punch-out dim; normalize negative drags; mouseup →
    `invoke("finish_capture", …)` with `dpr: window.devicePixelRatio`; Esc
    AND window blur → `cancel_capture` (blur auto-cancel prevents an orphaned
    dim after Alt-Tab). This bundle imports `@tauri-apps/api/core` directly —
    the api.ts-only rule is per-bundle; this file *is* its bundle's boundary.
12. **types.ts** — `AttachmentInfo { id, thumbUri, width, height }`.
13. **api.ts** — `beginCapture`, `clearCapture`, `onCaptureAttached`,
    `onCaptureError`, `onCaptureHotkey`, `ask(..., imageId?)`.
14. **App.tsx** — `attachment` state; event subscriptions;
    `handleCaptureRequest()` (`if (busy) return; setOpenMenu(null);
    beginCapture()`); submit passes `attachment?.id`, success clears, error
    keeps; thumbnail row between `<PromptInput>` and `.content` (never inside
    `.content` — its overflow clips): thumb img + `{w}×{h}` caption + "×"
    remove; footer capture button (`.quiet-chip.capture-chip`,
    `margin-left: auto`, `disabled={busy}`).
15. **styles.css** — `.capture-chip`, `.attachment-row`, `.attachment-thumb`
    (44 px, `--radius-chip`, `--border`), `.attachment-meta`,
    `.attachment-remove` (mirrors `.remove-btn`). Existing tokens only.
16. **Verify** — `cargo check` + `cargo test` (crop_to_attachment: dpr
    1.0/1.5, clamping, negative-drag normalization, <4 px rejection, >1568
    downscale; build_user_content both provider shapes + `None`; shortcut
    matcher disjointness) · `npx tsc --noEmit && npm run build` (both rollup
    entries emit) · manual matrix: button path / hotkey with panel visible /
    hotkey with panel hidden / Esc cancel / Alt-Tab auto-cancel / four drag
    directions / tiny drag rejected / 100 % and 150 % scaling / second
    monitor incl. negative coords / capture-again replaces / success clears /
    error keeps / × removes / Anthropic + OpenRouter vision model answer
    references on-screen content / DeepSeek raw 400 reaches the error box
    (guardrails backstop confirmed reachable).

Size estimate: **L** — ~600–700 LOC across ~14 files.

## Decisions & trade-offs

- **Own capture over OS snipping** — legacy URI deprecated/clipboard-only/no
  cancel signal; supported protocol is MSIX-gated. Not a close call.
- **Freeze-at-trigger over live recapture at mouseup** — zero encode on the
  hot path and the dim layer can't photograph itself; cost is the crop being
  as-of trigger time (a frame or two of game animation). Accepted.
- **Hide + ~120 ms settle over `set_content_protected(true)`** — the panel
  must disappear anyway so the user can select beneath it;
  WDA_EXCLUDEFROMCAPTURE would also hide the overlay from users' own
  OBS/screen-shares. Revisit as an opt-in later.
- **Hotkey routed through the overlay webview (`capture://hotkey`)** — one
  IPC hop buys a single frontend entry point, which is exactly where
  [[2026-07-06_image-attach-guardrails]] hangs its gating.
- **Rust-side attachment slot over base64-through-IPC** — the full PNG never
  leaves Rust (matches "all HTTP happens in Rust"); only the small thumbnail
  crosses IPC; lifecycle is enforced at one point; the one-ask-at-a-time
  guard prevents mid-request mutation; a stale id is caught by the id check.
- **Conditional SYSTEM_PROMPT addendum** — the spec-guardrail prompt stays
  byte-identical for every text-only ask.
- **Crop math as a pure function** — the DPI bug class is pinned by unit
  tests, not manual QA alone.

## Status log

- 2026-07-06 — created. Approach settled by a verified research pass (OS
  snipping ruled out; xcap owned-capture pattern; coordinate math and
  own-panel-exclusion gotchas identified up front).
- 2026-07-06 — implemented (status → active). All Approach steps landed across
  18 files; pinned `xcap = 0.9` (0.9.6, which re-exports `image` 0.25) and
  promoted `tokio` from dev-only for the settle sleep. Two details confirmed
  against real crate source rather than assumed: xcap's Windows `Monitor`
  accessors + `from_point` all work in **physical** virtual-desktop px (so crop
  coords are monitor-local `round(css × dpr)`), and Tauri's
  `AppHandle::cursor_position()` returns the global physical cursor — the exact
  space `from_point` wants. Added an `AppError::Capture` variant (doc 1 needs an
  error type) and a `capture://armed` event so the reused, never-reloaded
  capture webview re-arms each open. Green on all automated checks: `cargo
  check` clean (0 warnings), `cargo test` **101 passed / 0 failed** (incl. the
  crop DPI/clamp/negative-drag/downscale suite, `build_user_content` both
  provider shapes + byte-identical text-only, shortcut disjointness), `npx tsc
  --noEmit`, and `npm run build` (both `index.html` + `capture.html` bundles
  emit). **Pending:** commit, and the §16 manual matrix (drag directions,
  100%/150% scaling, second-monitor negative coords, Alt-Tab auto-cancel,
  live vision-model answers) — those need a physical display and can't run
  headless. On commit: set `status: done` + `commit:`.
- 2026-07-06 — closed (status → done). The §16 manual matrix passed on the dev
  build (user-confirmed): both triggers, drag directions, Esc/tiny-drag/Alt-Tab
  cancels, DPI scaling, lifecycle (replace/clear-on-success/keep-on-error), and
  live vision-model answers referencing on-screen content. Shipped in `88facae`.
