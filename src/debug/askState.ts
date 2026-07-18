// Pure reducer over the debug window's event stream — all ordering rules live
// here so they're unit-testable without mounting anything.
//
// Shape of a session: `ask://status "searching"` fires BEFORE
// `debug://ask-started` (the overlay emits it right before the report is
// created), so a status with no live ask is buffered and applied when the
// header lands. Completed `debug://phase` rows are authoritative; the buffered
// status only drives the provisional "running" row.

import type {
  AskStatus,
  DebugAskStarted,
  DebugCandidates,
  DebugFinished,
  DebugPageEntry,
  DebugPages,
  DebugPhase,
  DebugUsage,
} from "../types";

/** Everything known about one ask, newest-first in `DebugState.asks`. */
export interface DebugAskEntry {
  askId: number;
  game: string;
  question: string;
  query: string;
  answerModel: string;
  rewriteModel: string | null;
  phases: DebugPhase[];
  candidates: string[];
  usage: { rewrite?: DebugUsage; answer?: DebugUsage };
  pages: DebugPageEntry[];
  /** Present once `debug://finished` lands; its absence means "live". */
  finished?: DebugFinished;
  /** Latest `ask://status` while live — drives the provisional row. */
  lastStatus?: AskStatus;
}

export interface DebugState {
  asks: DebugAskEntry[];
  /** A status that arrived before its ask's header (see module doc). */
  pendingStatus: AskStatus | null;
}

export type DebugAction =
  | { type: "ask-started"; payload: DebugAskStarted }
  | { type: "phase"; payload: DebugPhase }
  | { type: "candidates"; payload: DebugCandidates }
  | { type: "usage"; payload: DebugUsage }
  | { type: "pages"; payload: DebugPages }
  | { type: "finished"; payload: DebugFinished }
  | { type: "status"; payload: AskStatus };

export const INITIAL_STATE: DebugState = { asks: [], pendingStatus: null };

/** Session history bound — old asks fall off the end, newest stay on top. */
export const HISTORY_CAP = 25;

/** What the coarse overlay status means in phase terms, for the provisional
 * row's label ("searching" covers the joined raw search + rewrite). */
export const STATUS_LABELS: Record<AskStatus, string> = {
  searching: "search + rewrite",
  understanding: "cand search",
  retrying: "retry",
  reading: "fetch",
  answering: "answer",
};

/** The ask a status/provisional row belongs to: the newest entry, only while
 * it hasn't finished. */
export function liveAsk(state: DebugState): DebugAskEntry | undefined {
  const newest = state.asks[0];
  return newest && !newest.finished ? newest : undefined;
}

function patchAsk(
  state: DebugState,
  askId: number,
  patch: (entry: DebugAskEntry) => DebugAskEntry,
): DebugState {
  // Events for an unknown askId are dropped — the accepted first-ask race
  // when the page loads mid-ask (no replay buffer by design).
  if (!state.asks.some((a) => a.askId === askId)) return state;
  return {
    ...state,
    asks: state.asks.map((a) => (a.askId === askId ? patch(a) : a)),
  };
}

export function reduce(state: DebugState, action: DebugAction): DebugState {
  switch (action.type) {
    case "ask-started": {
      const p = action.payload;
      const entry: DebugAskEntry = {
        askId: p.askId,
        game: p.game,
        question: p.question,
        query: p.query,
        answerModel: p.answerModel,
        rewriteModel: p.rewriteModel,
        phases: [],
        candidates: [],
        usage: {},
        pages: [],
        lastStatus: state.pendingStatus ?? undefined,
      };
      return {
        pendingStatus: null,
        asks: [entry, ...state.asks].slice(0, HISTORY_CAP),
      };
    }
    case "phase":
      return patchAsk(state, action.payload.askId, (a) => ({
        ...a,
        phases: [...a.phases, action.payload],
      }));
    case "candidates":
      return patchAsk(state, action.payload.askId, (a) => ({
        ...a,
        candidates: action.payload.candidates,
      }));
    case "usage":
      return patchAsk(state, action.payload.askId, (a) => ({
        ...a,
        usage: { ...a.usage, [action.payload.kind]: action.payload },
      }));
    case "pages":
      return patchAsk(state, action.payload.askId, (a) => ({
        ...a,
        pages: action.payload.pages,
      }));
    case "finished":
      return patchAsk(state, action.payload.askId, (a) => ({
        ...a,
        finished: action.payload,
        lastStatus: undefined,
      }));
    case "status": {
      const live = liveAsk(state);
      if (!live) return { ...state, pendingStatus: action.payload };
      return patchAsk(state, live.askId, (a) => ({
        ...a,
        lastStatus: action.payload,
      }));
    }
  }
}
