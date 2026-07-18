// The debug bundle's typed IPC boundary. The "components import from api.ts
// only" rule is per-bundle (the capture page's precedent), and this file IS
// this bundle's boundary — so it imports @tauri-apps/api directly. The page
// only listens: it invokes no commands (pinned by capabilities/debug.json and
// its guardrail test).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AskStatus,
  DebugAskStarted,
  DebugCandidates,
  DebugFinished,
  DebugPages,
  DebugPhase,
  DebugUsage,
} from "../types";

function on<T>(event: string): (callback: (payload: T) => void) => Promise<UnlistenFn> {
  return (callback) => listen<T>(event, (e) => callback(e.payload));
}

export const onDebugAskStarted = on<DebugAskStarted>("debug://ask-started");
export const onDebugPhase = on<DebugPhase>("debug://phase");
export const onDebugCandidates = on<DebugCandidates>("debug://candidates");
export const onDebugUsage = on<DebugUsage>("debug://usage");
export const onDebugPages = on<DebugPages>("debug://pages");
export const onDebugFinished = on<DebugFinished>("debug://finished");

/** The overlay's app-wide status broadcast, reused for the provisional
 * "running" row — it fires seconds before the authoritative `debug://phase`
 * rows land. */
export const onAskStatus = on<AskStatus>("ask://status");
