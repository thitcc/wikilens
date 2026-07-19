// The single boundary between the frontend and Rust. Components import from
// here only — never from `@tauri-apps/*` directly. This keeps all IPC calls,
// event names, and payload types in one typed place.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  AskResult,
  AskStatus,
  AttachmentInfo,
  GameInfo,
  ModelList,
  ProviderInfo,
  WikiCandidate,
} from "./types";

// ---- Commands -------------------------------------------------------------

/** List the supported games: built-ins, then user-added wikis. */
export function listGames(): Promise<GameInfo[]> {
  return invoke<GameInfo[]>("list_games");
}

/** Probe the common wiki hosts for a game name; resolves with only the
 * verified wikis. Sequential probes Rust-side — can take a few seconds. */
export function suggestWikis(name: string): Promise<WikiCandidate[]> {
  return invoke<WikiCandidate[]>("suggest_wikis", { name });
}

/** Probe-validate a wiki URL and save it as a user-added game. Rejects with
 * a user-readable message when no MediaWiki API answers there. */
export function addGame(name: string, url: string): Promise<GameInfo> {
  return invoke<GameInfo>("add_game", { name, url });
}

/** Remove a user-added game (built-ins are refused Rust-side). */
export function removeGame(id: string): Promise<void> {
  return invoke<void>("remove_game", { id });
}

/** List the supported LLM providers with their resolved default models. */
export function listProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("list_providers");
}

/** List a provider's selectable models (live when possible, curated fallback
 * otherwise — see `ModelList.source`). */
export function listModels(providerId: string): Promise<ModelList> {
  return invoke<ModelList>("list_models", { providerId });
}

/** Hide the overlay window (focus returns to the game). */
export function hideOverlay(): Promise<void> {
  return invoke<void>("hide_overlay");
}

/** Show the overlay window if it's hidden; a visible panel is untouched (no
 * `overlay://shown` re-fire, so an open draft is never re-selected). */
export function showOverlay(): Promise<void> {
  return invoke<void>("show_overlay");
}

/** Whether the debug window exists this session (WIKILENS_DEBUG at startup) —
 * gates the footer's Debug chip. */
export function debugAvailable(): Promise<boolean> {
  return invoke<boolean>("debug_available");
}

/** Show/hide the debug window (the footer Debug chip's action). */
export function toggleDebugWindow(): Promise<void> {
  return invoke<void>("toggle_debug_window");
}

/** The rejection value `ask` settles with after `cancelAsk` wins — mirrors
 * `ASK_CANCELLED` in `commands.rs`. The one machine-readable command error:
 * compare against it and reset quietly; never render it. */
export const ASK_CANCELLED = "wikilens::ask-cancelled";

/** Ask a question about a game using a chosen provider and model; resolves
 * with the answer and sources. A blank `model` falls back to the provider's
 * default Rust-side. `imageId` optionally attaches a captured screenshot (from
 * `onCaptureAttached`); Rust rejects a stale id. Rejects with
 * [`ASK_CANCELLED`] when `cancelAsk` aborts it. */
export function ask(
  gameId: string,
  providerId: string,
  model: string,
  question: string,
  imageId?: string,
): Promise<AskResult> {
  // Tauri maps camelCase JS keys to the command's snake_case params
  // (providerId → provider_id, imageId → image_id).
  return invoke<AskResult>("ask", {
    gameId,
    providerId,
    model,
    question,
    imageId: imageId ?? null,
  });
}

/** Abort the in-flight ask (the status row's Stop action). Always resolves:
 * a no-op when nothing is running, idempotent on a double click. The `ask`
 * promise itself then rejects with [`ASK_CANCELLED`]. */
export function cancelAsk(): Promise<void> {
  return invoke<void>("cancel_ask");
}

/** Start a region capture: hide the panel and show the crosshair overlay. The
 * result arrives later via `onCaptureAttached` / `onCaptureError`. */
export function beginCapture(): Promise<void> {
  return invoke<void>("begin_capture");
}

/** Drop the attached screenshot (the prompt strip's "×"). */
export function clearCapture(): Promise<void> {
  return invoke<void>("clear_capture");
}

/** Open a URL in the user's default browser (source links). */
export function openExternal(url: string): Promise<void> {
  return openUrl(url);
}

// ---- Events ---------------------------------------------------------------

/** Fired after the overlay is shown; use it to focus the prompt input. */
export function onOverlayShown(callback: () => void): Promise<UnlistenFn> {
  return listen("overlay://shown", () => callback());
}

/** Fired as the `ask` command advances through its phases. */
export function onAskStatus(
  callback: (status: AskStatus) => void,
): Promise<UnlistenFn> {
  return listen<AskStatus>("ask://status", (event) => callback(event.payload));
}

/** Fired for each streamed chunk of answer text. */
export function onAskDelta(
  callback: (chunk: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("ask://delta", (event) => callback(event.payload));
}

/** Fired when a capture finishes: carries the thumbnail + id to attach. */
export function onCaptureAttached(
  callback: (info: AttachmentInfo) => void,
): Promise<UnlistenFn> {
  return listen<AttachmentInfo>("capture://attached", (event) =>
    callback(event.payload),
  );
}

/** Fired when a capture fails (no monitor, OS refused the grab, too small). */
export function onCaptureError(
  callback: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("capture://error", (event) => callback(event.payload));
}

/** Fired when the Ctrl+Shift+C hotkey is pressed (routed through Rust so the
 * frontend stays the single capture entry point). */
export function onCaptureHotkey(callback: () => void): Promise<UnlistenFn> {
  return listen("capture://hotkey", () => callback());
}
