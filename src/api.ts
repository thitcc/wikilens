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
  HotkeyRole,
  KeyStatus,
  Mode,
  ModelList,
  ProviderInfo,
  SettingsInfo,
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

/** "Size for the cap" sentinel — sent while a menu is open (menus render into
 * the free window space below/above the panel). Rust clamps it to the 70%
 * cap; the frontend never learns the monitor height. */
export const PANEL_HEIGHT_UNBOUNDED = 100_000;

/** Report the panel's desired height in logical CSS px; Rust clamps to the
 * 70% cap, converts per-monitor DPI, and resizes the overlay window — the
 * transparent remainder of an oversized window would eat the game's clicks. */
export function setOverlayHeight(height: number): Promise<void> {
  return invoke<void>("set_overlay_height", { height });
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

/** Current settings (the configured shortcuts), for the settings popover and
 * the overlay's dynamic copy. */
export function getSettings(): Promise<SettingsInfo> {
  return invoke<SettingsInfo>("get_settings");
}

/** Change one shortcut to a canonical accelerator string (from
 * `toAccelerator` in `hotkeys.ts`). Resolves with the fresh settings;
 * rejects with a user-readable message (invalid combo, the other role's
 * combo, or another app owns it). */
export function setHotkey(
  role: HotkeyRole,
  accelerator: string,
): Promise<SettingsInfo> {
  return invoke<SettingsInfo>("set_hotkey", { role, accelerator });
}

/** Drop the OS hotkey registrations while the recorder is armed — otherwise
 * pressing the current combo mid-recording would toggle the overlay.
 * Idempotent. */
export function suspendHotkeys(): Promise<void> {
  return invoke<void>("suspend_hotkeys");
}

/** Restore the configured registrations after recording. Idempotent; rejects
 * with a user-readable message if a combo was taken meanwhile. */
export function resumeHotkeys(): Promise<void> {
  return invoke<void>("resume_hotkeys");
}

/** Key presence per provider, for the Settings panel's API-keys section. */
export function listKeyStatus(): Promise<KeyStatus[]> {
  return invoke<KeyStatus[]>("list_key_status");
}

/** Store (or replace) a provider's API key — the ONE call that carries key
 * material toward Rust; it is never returned, rendered, or logged back.
 * Resolves with fresh statuses so the panel updates in one round trip;
 * rejects with a user-readable message (blank key, unknown provider, store
 * failure). */
export function setApiKey(
  providerId: string,
  key: string,
): Promise<KeyStatus[]> {
  return invoke<KeyStatus[]>("set_api_key", { providerId, key });
}

/** Drop a provider's stored key — the only action offered on a set key.
 * Resolves with fresh statuses. */
export function removeApiKey(providerId: string): Promise<KeyStatus[]> {
  return invoke<KeyStatus[]>("remove_api_key", { providerId });
}

/** Persist the model-source choice — the footer chip and the ask path follow
 * it (Default: one env-configured target; Custom: keyed providers). Resolves
 * with the fresh settings. */
export function setMode(mode: Mode): Promise<SettingsInfo> {
  return invoke<SettingsInfo>("set_mode", { mode });
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

/** Fired after the overlay is hidden (Esc, hotkey toggle, tray, Alt+F4, the
 * capture flow). The hidden webview stays mounted and hears it; the frontend
 * timestamps it to keep a quick re-summon from select-alling the draft. */
export function onOverlayHidden(callback: () => void): Promise<UnlistenFn> {
  return listen("overlay://hidden", () => callback());
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

/** Fired when the capture shortcut (default Ctrl+Shift+C) is pressed (routed
 * through Rust so the frontend stays the single capture entry point). */
export function onCaptureHotkey(callback: () => void): Promise<UnlistenFn> {
  return listen("capture://hotkey", () => callback());
}
