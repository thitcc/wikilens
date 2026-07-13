// A fake Rust backend for the test suite, built on `@tauri-apps/api/mocks`.
//
// `installBackend()` installs mockIPC once per test with `shouldMockEvents`
// on, so the app's real `listen()` calls work and tests fire backend events
// with the real `emit()` (see harness.tsx). NEVER call mockIPC again
// mid-test: the event-listener map lives inside the first install's closure,
// so a re-install silently orphans every listener the app registered — use
// `backend.onCommand()` to swap a handler instead.

import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  AskResult,
  AttachmentInfo,
  GameInfo,
  ModelList,
  ProviderInfo,
  WikiCandidate,
} from "../types";

// ---- Fixtures ---------------------------------------------------------------

export const GAMES: GameInfo[] = [
  { id: "terraria", name: "Terraria", custom: false },
  { id: "stardew-valley", name: "Stardew Valley", custom: false },
  { id: "my-wiki", name: "My Wiki", custom: true },
];

export const PROVIDERS: ProviderInfo[] = [
  {
    id: "anthropic",
    name: "Anthropic",
    defaultModel: "claude-sonnet-5",
    defaultModelLabel: "Claude Sonnet 5",
    defaultModelVision: true,
  },
  {
    // The real id matters: activeModelVision() hard-codes "deepseek" as
    // text-only regardless of any stored flag.
    id: "deepseek",
    name: "DeepSeek",
    defaultModel: "deepseek-chat",
    defaultModelLabel: "DeepSeek Chat",
    defaultModelVision: false,
  },
];

export const MODELS: ModelList = {
  models: [
    { id: "claude-sonnet-5", label: "Claude Sonnet 5", vision: true },
    { id: "claude-haiku-4-5", label: "Claude Haiku 4.5", vision: true },
  ],
  source: "live",
};

export const ASK_OK: AskResult = {
  answer: "Use a **Water Candle** to raise spawn rates.",
  sources: [
    { title: "Water Candle", url: "https://terraria.wiki.gg/wiki/Water_Candle" },
  ],
};

export const ATTACHMENT: AttachmentInfo = {
  id: "cap-1",
  thumbUri: "data:image/png;base64,AAAA",
  width: 320,
  height: 200,
};

export const CANDIDATES: WikiCandidate[] = [
  {
    name: "Hollow Knight Wiki",
    apiUrl: "https://hollowknight.wiki/api.php",
    pageUrl: "https://hollowknight.wiki/w/$1",
  },
];

// ---- Backend ----------------------------------------------------------------

type CommandHandler = (args: unknown) => unknown;

export interface RecordedCall {
  cmd: string;
  args: unknown;
}

export interface Backend {
  /** Every non-event IPC call, in order. */
  calls: RecordedCall[];
  /** The recorded args of every call to one command. */
  callsTo(cmd: string): unknown[];
  /** Swap a command's handler mid-test (the mockIPC-safe way). */
  onCommand(cmd: string, handler: CommandHandler): void;
}

/** A promise with its resolve/reject exposed — gates a command mid-flight. */
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Install the fake backend (happy-path defaults, overridable per command).
 * Unmocked commands throw loudly — a test that trips one is exercising IPC it
 * didn't declare. */
export function installBackend(
  overrides: Record<string, CommandHandler> = {},
): Backend {
  const calls: RecordedCall[] = [];
  const handlers = new Map<string, CommandHandler>(
    Object.entries({
      list_games: () => GAMES,
      list_providers: () => PROVIDERS,
      list_models: () => MODELS,
      ask: () => ASK_OK,
      hide_overlay: () => undefined,
      begin_capture: () => undefined,
      clear_capture: () => undefined,
      suggest_wikis: () => CANDIDATES,
      add_game: () => {
        throw new Error("add_game: override this handler in the test");
      },
      remove_game: () => undefined,
      "plugin:opener|open_url": () => undefined,
      ...overrides,
    }),
  );

  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args });
      const handler = handlers.get(cmd);
      if (!handler) {
        throw new Error(`installBackend: unmocked command "${cmd}"`);
      }
      return handler(args);
    },
    { shouldMockEvents: true },
  );

  return {
    calls,
    callsTo: (cmd) => calls.filter((c) => c.cmd === cmd).map((c) => c.args),
    onCommand: (cmd, handler) => {
      handlers.set(cmd, handler);
    },
  };
}
