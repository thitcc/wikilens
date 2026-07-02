// The single boundary between the frontend and Rust. Components import from
// here only — never from `@tauri-apps/*` directly. This keeps all IPC calls,
// event names, and payload types in one typed place.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AskResult, AskStatus, GameInfo, ProviderInfo } from "./types";

// ---- Commands -------------------------------------------------------------

/** List the supported games (id + display name). */
export function listGames(): Promise<GameInfo[]> {
  return invoke<GameInfo[]>("list_games");
}

/** List the supported LLM providers (id + display name). */
export function listProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("list_providers");
}

/** Hide the overlay window (focus returns to the game). */
export function hideOverlay(): Promise<void> {
  return invoke<void>("hide_overlay");
}

/** Ask a question about a game using a chosen provider; resolves with the answer and sources. */
export function ask(
  gameId: string,
  providerId: string,
  question: string,
): Promise<AskResult> {
  // Tauri maps camelCase JS keys to the command's snake_case params
  // (providerId → provider_id).
  return invoke<AskResult>("ask", { gameId, providerId, question });
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
