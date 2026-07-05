// Shared types mirroring the Rust command layer (src-tauri/src/commands.rs).

/** A supported game, from the `list_games` command. */
export interface GameInfo {
  id: string;
  name: string;
}

/** A supported LLM provider, from the `list_providers` command. */
export interface ProviderInfo {
  id: string;
  name: string;
}

/** A wiki page used as a source for an answer. */
export interface Source {
  title: string;
  url: string;
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
