// The slow-wiki hint: a per-phase timer that blames a stalled ask phase on
// the wiki, not WikiLens. Lives in its own file because it is the suite's
// only fake-timer regime — the beforeEach/afterEach here must not leak into
// the real-timer App tests.
//
// Fake-timer rules (first use in this repo):
// - `shouldAdvanceTime: true` is load-bearing: with the clock fully frozen,
//   the harness's waitFor-based mount (`renderApp`) never settles and every
//   test times out. Letting the fake clock creep with real time unblocks it;
//   `advanceTimersByTimeAsync` still jumps the 10s threshold instantly.
// - The creep means real milliseconds leak into fake time between statements,
//   so never assert 1ms from the threshold — keep ≥1s of margin.
// - `userEvent.setup({ advanceTimers: ... })`, or every keystroke's internal
//   delay hangs the test.
// - Advance inside `act` (the timeout callback is a state update).

import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { AskResult } from "./types";
import { SLOW_WIKI_HINT_MS } from "./App";
import { ASK_OK, deferred, installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

const HINT = /responding slowly/;

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

/** Submit a question with the ask gated open, so busy stays true. */
async function submitGatedAsk() {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  await renderApp();
  await user.type(
    screen.getByRole("textbox", { name: "Question" }),
    "how do I fish{Enter}",
  );
  return { backend, gate, user };
}

test("hint appears at the threshold, not before; a duplicate status emit doesn't reset the clock", async () => {
  await submitGatedAsk();
  expect(screen.getByText("Searching the wiki…")).toBeTruthy();

  await advance(SLOW_WIKI_HINT_MS - 1_000);
  expect(screen.queryByText(HINT)).toBeNull();

  // The backend re-emits "searching" after the optimistic submit set it; an
  // identical value is a React state bail-out and must not re-arm the timer.
  await fireBackendEvent("ask://status", "searching");
  await advance(1_000);
  expect(screen.getByText(HINT)).toBeTruthy();
});

test("the timer resets on each phase change", async () => {
  await submitGatedAsk();

  await advance(6_000);
  await fireBackendEvent("ask://status", "reading");
  // 12s wall time, but only 6s into "reading" — a cumulative timer would
  // wrongly fire here.
  await advance(6_000);
  expect(screen.queryByText(HINT)).toBeNull();

  await advance(4_000);
  expect(screen.getByText(HINT)).toBeTruthy();
});

test("never shown while answering: the LLM stream is not the wiki's fault", async () => {
  await submitGatedAsk();
  await fireBackendEvent("ask://status", "reading");
  await advance(SLOW_WIKI_HINT_MS);
  expect(screen.getByText(HINT)).toBeTruthy();

  await fireBackendEvent("ask://status", "answering");
  expect(screen.queryByText(HINT)).toBeNull();
  await advance(SLOW_WIKI_HINT_MS);
  expect(screen.queryByText(HINT)).toBeNull();
});

test("cleared when the ask resolves", async () => {
  const { gate } = await submitGatedAsk();
  await fireBackendEvent("ask://status", "reading");
  await advance(SLOW_WIKI_HINT_MS);
  expect(screen.getByText(HINT)).toBeTruthy();

  await act(async () => {
    gate.resolve(ASK_OK);
  });
  expect(screen.queryByText(HINT)).toBeNull();
  expect(screen.queryByText("Reading pages…")).toBeNull();
});

test("cleared when the ask fails — the error box stands alone", async () => {
  const { gate } = await submitGatedAsk();
  await fireBackendEvent("ask://status", "reading");
  await advance(SLOW_WIKI_HINT_MS);
  expect(screen.getByText(HINT)).toBeTruthy();

  await act(async () => {
    gate.reject(new Error("wiki exploded"));
  });
  expect(screen.getByText(/wiki exploded/)).toBeTruthy();
  expect(screen.queryByText(HINT)).toBeNull();
});

// Tests 4/5 alone are render-gate tautologies: busy=false/status=null (or the
// error) unmount the whole .status block whether or not the slowHint state was
// reset. Only a SECOND ask observes the reset itself — without the effect's
// setSlowHint(false), ask 1's stale true renders the hint at 0ms of ask 2,
// blaming a wiki that hasn't been contacted yet (mutation-verified).
test("a stale hint from the previous ask never leaks into the next one", async () => {
  const { backend, gate, user } = await submitGatedAsk();
  await fireBackendEvent("ask://status", "reading");
  await advance(SLOW_WIKI_HINT_MS);
  expect(screen.getByText(HINT)).toBeTruthy();
  await act(async () => {
    gate.resolve(ASK_OK);
  });

  // Second ask (the question text survives in the box; Enter resubmits it).
  const gate2 = deferred<AskResult>();
  backend.onCommand("ask", () => gate2.promise);
  await user.type(
    screen.getByRole("textbox", { name: "Question" }),
    "{Enter}",
  );
  expect(screen.getByText("Searching the wiki…")).toBeTruthy();
  expect(screen.queryByText(HINT)).toBeNull();

  await advance(SLOW_WIKI_HINT_MS - 1_000);
  expect(screen.queryByText(HINT)).toBeNull();
  await advance(1_000);
  expect(screen.getByText(HINT)).toBeTruthy();
});
