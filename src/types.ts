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
 * `retrying` only appears when the first search found nothing. */
export type AskStatus = "searching" | "retrying" | "reading" | "answering";

/**
 * Streaming events emitted by the `ask` command while it runs.
 * - `status`: a new phase (`ask://status`)
 * - `delta`: a chunk of answer text (`ask://delta`)
 */
export type StreamEvent =
  | { kind: "status"; status: AskStatus }
  | { kind: "delta"; text: string };
