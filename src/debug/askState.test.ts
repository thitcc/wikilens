// Reducer rules for the debug window's event stream — ordering, buffering,
// and the history cap, without mounting anything.

import { expect, test } from "vitest";

import type { DebugAskStarted, DebugFinished, DebugPhase } from "../types";
import {
  HISTORY_CAP,
  INITIAL_STATE,
  liveAsk,
  reduce,
  type DebugState,
} from "./askState";

function started(
  askId: number,
  over: Partial<DebugAskStarted> = {},
): DebugAskStarted {
  return {
    askId,
    game: "Stardew Valley (stardew-valley)",
    question: "best crops for winter?",
    query: "best crops winter",
    answerModel: "deepseek / deepseek-chat",
    rewriteModel: null,
    ...over,
  };
}

function phase(
  askId: number,
  name: string,
  elapsedMs: number | null,
): DebugPhase {
  return { askId, name, elapsedMs, detail: "5 hits" };
}

function finished(
  askId: number,
  over: Partial<DebugFinished> = {},
): DebugFinished {
  return { askId, totalMs: 900, outcome: "answered", aborted: false, ...over };
}

test("happy path: buffered status, header, phases, details, finish", () => {
  let state: DebugState = INITIAL_STATE;

  // "searching" fires before the header exists — it must buffer, not vanish.
  state = reduce(state, { type: "status", payload: "searching" });
  expect(state.asks).toHaveLength(0);
  expect(state.pendingStatus).toBe("searching");

  state = reduce(state, { type: "ask-started", payload: started(1) });
  expect(state.pendingStatus).toBeNull();
  expect(state.asks[0].lastStatus).toBe("searching");
  expect(liveAsk(state)?.askId).toBe(1);

  // A status while an ask is live lands on the entry, not the buffer.
  state = reduce(state, { type: "status", payload: "reading" });
  expect(state.asks[0].lastStatus).toBe("reading");
  expect(state.pendingStatus).toBeNull();

  state = reduce(state, { type: "phase", payload: phase(1, "rewrite", null) });
  state = reduce(state, { type: "phase", payload: phase(1, "raw search", 380) });
  expect(state.asks[0].phases.map((p) => p.name)).toEqual([
    "rewrite",
    "raw search",
  ]);

  state = reduce(state, {
    type: "candidates",
    payload: { askId: 1, candidates: ["Winter crops"] },
  });
  state = reduce(state, {
    type: "usage",
    payload: { askId: 1, kind: "answer", input: 15890, output: 312 },
  });
  state = reduce(state, {
    type: "pages",
    payload: { askId: 1, pages: [{ title: "Winter", chars: 9120 }] },
  });
  expect(state.asks[0].candidates).toEqual(["Winter crops"]);
  expect(state.asks[0].usage.answer?.output).toBe(312);
  expect(state.asks[0].usage.rewrite).toBeUndefined();
  expect(state.asks[0].pages[0].title).toBe("Winter");

  state = reduce(state, { type: "finished", payload: finished(1) });
  expect(state.asks[0].finished?.outcome).toBe("answered");
  expect(state.asks[0].lastStatus).toBeUndefined();
  expect(liveAsk(state)).toBeUndefined();
});

test("mid-ask abort keeps the partial entry, flagged aborted", () => {
  let state = reduce(INITIAL_STATE, { type: "ask-started", payload: started(7) });
  state = reduce(state, { type: "phase", payload: phase(7, "raw search", 420) });
  state = reduce(state, {
    type: "finished",
    payload: finished(7, { outcome: "aborted (error or cancelled)", aborted: true }),
  });
  expect(state.asks[0].phases).toHaveLength(1);
  expect(state.asks[0].finished?.aborted).toBe(true);
  expect(liveAsk(state)).toBeUndefined();
});

test("history is newest-first and capped", () => {
  let state: DebugState = INITIAL_STATE;
  for (let id = 1; id <= HISTORY_CAP + 3; id++) {
    state = reduce(state, { type: "ask-started", payload: started(id) });
  }
  expect(state.asks).toHaveLength(HISTORY_CAP);
  expect(state.asks[0].askId).toBe(HISTORY_CAP + 3);
  expect(state.asks[state.asks.length - 1].askId).toBe(4);
});

test("events for an unknown askId are dropped (first-ask page-load race)", () => {
  const state = reduce(INITIAL_STATE, {
    type: "phase",
    payload: phase(99, "raw search", 100),
  });
  expect(state).toBe(INITIAL_STATE);
});

test("a status after the ask finished buffers for the next ask", () => {
  let state = reduce(INITIAL_STATE, { type: "ask-started", payload: started(1) });
  state = reduce(state, { type: "finished", payload: finished(1) });
  state = reduce(state, { type: "status", payload: "searching" });
  expect(state.asks[0].lastStatus).toBeUndefined();
  expect(state.pendingStatus).toBe("searching");

  state = reduce(state, { type: "ask-started", payload: started(2) });
  expect(state.asks[0].lastStatus).toBe("searching");
  expect(state.asks).toHaveLength(2);
});
