// The debug page mounted and driven purely by backend events — no commands
// are ever invoked (the window's capability is listen-only), so the fake
// backend is only here for its mocked event bus.

import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";

import { installBackend } from "../test/backend";
import { fireBackendEvent } from "../test/harness";
import DebugApp from "./DebugApp";

/** Bare render (no StrictMode), like renderApp — then flush the async
 * listen() registrations so the first fired event is already heard. */
async function renderDebugApp() {
  installBackend();
  const result = render(<DebugApp />);
  await screen.findByText(/Waiting for the first ask/);
  await act(async () => {});
  return result;
}

const HEADER = {
  askId: 1,
  game: "Stardew Valley (stardew-valley)",
  question: "best crops for winter?",
  query: "best crops winter",
  answerModel: "deepseek / deepseek-chat",
  rewriteModel: "anthropic / claude-haiku-4-5-20251001",
};

test("an ask streams in: provisional row, phases, then the outcome", async () => {
  await renderDebugApp();

  // Status precedes the header (the real emission order) — it must surface
  // as the provisional row once the card exists.
  await fireBackendEvent("ask://status", "searching");
  await fireBackendEvent("debug://ask-started", HEADER);

  expect(screen.getByText("best crops for winter?")).toBeTruthy();
  expect(screen.getByText("running…")).toBeTruthy();
  expect(screen.getByText("search + rewrite")).toBeTruthy();

  await fireBackendEvent("debug://phase", {
    askId: 1,
    name: "rewrite",
    elapsedMs: null,
    detail: "disabled (WIKILENS_QUERY_REWRITE)",
  });
  await fireBackendEvent("debug://phase", {
    askId: 1,
    name: "raw search",
    elapsedMs: 380,
    detail: "5 hits",
  });
  expect(screen.getByText("raw search")).toBeTruthy();
  expect(screen.getByText("380")).toBeTruthy();
  // Skipped phase renders the muted dash, not a bar.
  expect(screen.getByText("—")).toBeTruthy();

  await fireBackendEvent("debug://finished", {
    askId: 1,
    totalMs: 900,
    outcome: "answered",
    aborted: false,
  });
  expect(screen.getByText("answered")).toBeTruthy();
  expect(screen.getByText("900 ms")).toBeTruthy();
  expect(screen.queryByText("running…")).toBeNull();
  expect(screen.queryByText("search + rewrite")).toBeNull();
});

test("collapsed detail groups open on click and show their contents", async () => {
  const user = userEvent.setup();
  await renderDebugApp();

  await fireBackendEvent("debug://ask-started", HEADER);
  await fireBackendEvent("debug://candidates", {
    askId: 1,
    candidates: ["Winter crops"],
  });
  await fireBackendEvent("debug://pages", {
    askId: 1,
    pages: [{ title: "Winter", chars: 9120 }],
  });
  await fireBackendEvent("debug://usage", {
    askId: 1,
    kind: "rewrite",
    input: 184,
    output: 22,
  });

  // Collapsed by default.
  expect(screen.queryByText("Winter crops")).toBeNull();

  await user.click(
    screen.getByRole("button", { name: /queries & candidates/i }),
  );
  expect(screen.getByText("Winter crops")).toBeTruthy();
  expect(screen.getByText("best crops winter")).toBeTruthy();

  await user.click(screen.getByRole("button", { name: /pages/i }));
  expect(screen.getByText("Winter")).toBeTruthy();
  expect(screen.getByText("(9120)")).toBeTruthy();

  await user.click(screen.getByRole("button", { name: /tokens/i }));
  // Rewrite usage arrived; the answer call never happened → -/-.
  expect(screen.getByText("184/22")).toBeTruthy();
  expect(screen.getByText("-/-")).toBeTruthy();
});
