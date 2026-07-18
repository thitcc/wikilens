// Shared types mirroring the Rust command layer (src-tauri/src/commands.rs).

/** A supported game, from the `list_games` command. */
export interface GameInfo {
  id: string;
  name: string;
  /** `true` for user-added wikis (removable); `false` for built-ins. */
  custom: boolean;
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
