// Shared types mirroring the Rust command layer (src-tauri/src/commands.rs).

/** A supported game, from the `list_games` command. */
export interface GameInfo {
  id: string;
  name: string;
  /** `true` for user-added wikis (removable); `false` for built-ins. */
  custom: boolean;
}

/** Payload of `overlay://shown`. Carries what Rust learned about the window
 * the panel just covered. */
export interface ShownInfo {
  /** Id of the game whose process owns the foreground window, or `null` when
   * nothing matched — the player was over a browser, over one of our own
   * windows, or over a game with no rule. A **suggestion**: the frontend
   * offers it and only a click applies it. `null` means "unknown, keep the
   * player's pick", never "no game". */
  detectedGame: string | null;
}

/** A probe-verified wiki, from the `suggest_wikis` command. Endpoints are
 * canonical (derived Rust-side from the wiki's own siteinfo). */
export interface WikiCandidate {
  /** The wiki's own sitename, e.g. "Terraria Wiki". */
  name: string;
  apiUrl: string;
  pageUrl: string;
}

/** A supported LLM provider, from the `list_providers` command. Carries the
 * resolved default model (env override applied) so the footer chip can label
 * itself before any model list is fetched. */
export interface ProviderInfo {
  id: string;
  name: string;
  defaultModel: string;
  defaultModelLabel: string;
  /** Whether the resolved default model accepts image input — lets the chip
   * resolve vision before any model list is fetched. */
  defaultModelVision: boolean;
}

/** One selectable model, from the `list_models` command. */
export interface ModelInfo {
  id: string;
  label: string;
  /** Whether the model accepts image input (drives the "Image" badge). */
  vision: boolean;
  /** Whether the model thinks before answering: `true` → "Reasoning" badge
   * (and Rust skips the pre-search rewrite for it), `false` → "Fast" badge,
   * absent → unknown (no badge; the backend omits the field). */
  reasoning?: boolean;
}

/** The user's persisted model pick (localStorage `wikilens.selectedModel.<id>`).
 * `vision` is optional: entries saved before this feature lack it, and healing
 * that is the guardrails plan's concern. */
export interface StoredModelPick {
  id: string;
  label: string;
  vision?: boolean;
}

/** A provider's model list. `source === "fallback"` means the built-in
 * offline list (no key, fetch failed, or offline — the backend doesn't say
 * which). */
export interface ModelList {
  models: ModelInfo[];
  source: "live" | "fallback";
}

/** A wiki page used as a source for an answer. */
export interface Source {
  title: string;
  url: string;
}

/** A captured screenshot attached to the prompt, from the `capture://attached`
 * event. Only the small `thumbUri` (a PNG data-URI) crosses IPC; the full image
 * stays in Rust until the next `ask` sends it. `id` is echoed back on `ask`. */
export interface AttachmentInfo {
  id: string;
  thumbUri: string;
  width: number;
  height: number;
}

/** Final result of the `ask` command. */
export interface AskResult {
  answer: string;
  sources: Source[];
}

/** One answered ask, from the `list_history` command (newest first). Recorded
 * Rust-side on success only — no entry can be forged from the frontend.
 * `model` is `null` only on rows the removed Default mode wrote (its vendor
 * id was deliberately dev-only); new entries always carry the id. */
export interface HistoryEntry {
  id: string;
  /** Unix millis when the answer landed; render via `relativeTime`. */
  createdMs: number;
  gameId: string;
  /** Denormalized so a row still names its game after the game is removed. */
  gameName: string;
  question: string;
  /** The full streamed answer, as markdown. */
  answer: string;
  sources: Source[];
  providerName: string;
  model: string | null;
  hadImage: boolean;
}

/** Which configurable shortcut a `set_hotkey` call targets. */
export type HotkeyRole = "summon" | "capture";

/** One configurable shortcut, from `get_settings`/`set_hotkey`. The default
 * rides along so the popover can offer "Reset" with no extra command. */
export interface HotkeyInfo {
  /** Canonical accelerator form (mirrors Rust `hotkey::to_accelerator`),
   * e.g. `"Ctrl+Backquote"` — the identity the recorder compares against. */
  accelerator: string;
  /** Player-facing label, e.g. "Ctrl+`". */
  label: string;
  defaultAccelerator: string;
  defaultLabel: string;
  isDefault: boolean;
}

/** The model-source choice ("custom" | "local"), mirroring Rust `Mode`. */
export type Mode = "custom" | "local";

/** Where an anchored overlay docks, mirroring Rust `PanelAnchor`. */
export type PanelAnchor =
  | "top-right"
  | "top-left"
  | "bottom-right"
  | "bottom-left"
  | "center";

/** Whether the overlay follows its anchor or the player's dragged spot,
 * mirroring Rust `PositionMode`. */
export type PositionMode = "anchored" | "manual";

/** The Position stepper's wire value: an anchor, or `"manual"` (mirrors Rust
 * `PositionChoice`). */
export type PositionChoice = PanelAnchor | "manual";

/** The overlay placement, from `get_settings`/`set_panel_position`/
 * `set_position_locked` and the `settings://position` event. `anchor` is the
 * remembered anchor even while `mode` is `"manual"` — stepping back restores
 * it. The dragged coordinates deliberately never cross IPC. */
export interface PositionInfo {
  mode: PositionMode;
  anchor: PanelAnchor;
  /** The padlock: `true` = the header never drags, in every mode. */
  locked: boolean;
  /** Which vertical edge the stored Manual spot pins (mirrors Rust
   * `ManualEdge`); `null` = never dragged. `"bottom"` (a low drop) flips
   * menus and growth upward, the bottom-anchor way. The coordinates
   * themselves never cross IPC. */
  manualEdge: "top" | "bottom" | null;
}

/** One provider's key presence, from `list_key_status` / `set_api_key` /
 * `remove_api_key`. Presence only — key material never crosses back toward
 * the webview in any form. */
export interface KeyStatus {
  id: string;
  name: string;
  hasKey: boolean;
}

/** The Local AI mode's state (mirrors Rust `LocalModeInfo`). `baseUrl` is
 * the *effective* address — the stored one, or the baked Ollama default —
 * so the Settings field always shows where an ask would actually go. */
export interface LocalModeInfo {
  baseUrl: string;
  /** The manual "reads images" toggle — local model catalogs carry no
   * capability metadata, so the player declares it. */
  vision: boolean;
  /** Presence only — key material never crosses back (the KeyStatus rule). */
  hasKey: boolean;
}

/** Extensible settings envelope — future config-panel tenants join here. */
export interface SettingsInfo {
  hotkeys: {
    summon: HotkeyInfo;
    capture: HotkeyInfo;
  };
  /** Persisted model-source choice; `null` = never chosen (treated as
   * Custom everywhere). */
  mode: Mode | null;
  localMode: LocalModeInfo;
  position: PositionInfo;
}

/** Progress phases emitted on the `ask://status` event, in order.
 * `understanding` only appears while the rewrite's candidate searches run;
 * `retrying` only appears when the first search found nothing. */
export type AskStatus =
  | "searching"
  | "understanding"
  | "retrying"
  | "reading"
  | "answering";

// ---- debug:// events (src-tauri/src/debug.rs payloads; debug window only) ----

/** Header rows at ask start, from `debug://ask-started`. Emitted the moment
 * the ask's models are resolved; every later debug event carries the same
 * `askId`. */
export interface DebugAskStarted {
  askId: number;
  /** "Game Name (game-id)" — the table's game row, preformatted. */
  game: string;
  question: string;
  /** The keyword-stripped wiki search query. */
  query: string;
  /** "provider / model" pair answering the question. */
  answerModel: string;
  /** Present only when the rewrite pair differs from the answer pair. */
  rewriteModel: string | null;
}

/** One completed pipeline phase, from `debug://phase`, in execution order.
 * `elapsedMs: null` means the phase was skipped (e.g. rewrite disabled). */
export interface DebugPhase {
  askId: number;
  name: string;
  elapsedMs: number | null;
  detail: string;
}

/** The rewrite's candidate queries, from `debug://candidates`. */
export interface DebugCandidates {
  askId: number;
  candidates: string[];
}

/** Token usage for one LLM call, from `debug://usage`. A `null` field means
 * the call happened but the provider reported nothing (the table's `?`); a
 * call that never happened simply never emits this event (`-`). */
export interface DebugUsage {
  askId: number;
  kind: "rewrite" | "answer";
  input: number | null;
  output: number | null;
}

/** A fetched wiki page: title + extracted-plaintext char count only — the
 * text itself never crosses IPC. */
export interface DebugPageEntry {
  title: string;
  chars: number;
}

/** The fetched pages, from `debug://pages`. */
export interface DebugPages {
  askId: number;
  pages: DebugPageEntry[];
}

/** End of an ask, from `debug://finished` — emitted on every exit path.
 * `aborted` is `true` when no deliberate exit was recorded (an error
 * propagated or the future was cancelled). */
export interface DebugFinished {
  askId: number;
  totalMs: number;
  outcome: string;
  aborted: boolean;
}
