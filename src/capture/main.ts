// Region-capture overlay logic (vanilla TS, no React). This is a separate
// bundle from the main app: the "components import from api.ts only" rule is
// per-bundle, and this file IS its bundle's IPC boundary, so it invokes
// `@tauri-apps/api` directly.
//
// The window is pre-declared (hidden) and reused across captures — the webview
// is never reloaded — so all interaction is gated on `armed`, flipped true by
// the `capture://armed` event Rust emits each time it shows the window, and
// false again the moment we commit (finish/cancel). That gate also stops a
// stray blur during window show/hide from cancelling a capture that already
// finished.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const backdrop = document.getElementById("backdrop") as HTMLDivElement;
const selection = document.getElementById("selection") as HTMLDivElement;

/** Below this drag size (CSS px, either axis) a mouseup is a misclick → cancel.
 * Rust re-checks in physical px as the authoritative backstop. */
const MIN_DRAG_CSS = 5;

let armed = false;
let dragging = false;
let startX = 0;
let startY = 0;

/** Ready the overlay for a fresh capture (Rust just showed the window). */
function arm() {
  armed = true;
  dragging = false;
  selection.style.display = "none";
  backdrop.style.display = "block";
}

/** Commit the capture: invoke the command and disarm so no later event
 * (a blur as focus returns to the panel, a second Esc) fires again. */
function commit(command: "finish_capture" | "cancel_capture", args?: Record<string, unknown>) {
  if (!armed) return;
  armed = false;
  dragging = false;
  void invoke(command, args);
}

function drawRect(curX: number, curY: number) {
  const x = Math.min(startX, curX);
  const y = Math.min(startY, curY);
  selection.style.left = `${x}px`;
  selection.style.top = `${y}px`;
  selection.style.width = `${Math.abs(curX - startX)}px`;
  selection.style.height = `${Math.abs(curY - startY)}px`;
}

void listen("capture://armed", arm);

window.addEventListener("mousedown", (e: MouseEvent) => {
  if (!armed || e.button !== 0) return;
  dragging = true;
  startX = e.clientX;
  startY = e.clientY;
  backdrop.style.display = "none";
  selection.style.display = "block";
  drawRect(e.clientX, e.clientY);
});

window.addEventListener("mousemove", (e: MouseEvent) => {
  if (dragging) drawRect(e.clientX, e.clientY);
});

window.addEventListener("mouseup", (e: MouseEvent) => {
  if (!armed || !dragging || e.button !== 0) return;
  dragging = false;
  // Signed deltas — Rust normalizes right-to-left / bottom-to-top drags.
  const w = e.clientX - startX;
  const h = e.clientY - startY;
  if (Math.abs(w) < MIN_DRAG_CSS || Math.abs(h) < MIN_DRAG_CSS) {
    commit("cancel_capture");
    return;
  }
  commit("finish_capture", {
    rect: { x: startX, y: startY, w, h, dpr: window.devicePixelRatio },
  });
});

// Esc cancels; blur (Alt-Tab out) auto-cancels so the dim can't be left
// orphaned on top of everything (the window is alwaysOnTop).
window.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Escape") commit("cancel_capture");
});
window.addEventListener("blur", () => commit("cancel_capture"));
