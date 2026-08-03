// The header's Start over button: one gesture back to the default panel —
// draft, answer, sources, announcement, error, and screenshot all go, and a
// running ask is stopped. Unlike Stop (which deliberately resets nothing),
// clearing owns the panel: an ask that settles OR STREAMS after a clear must
// not resurrect anything — the epoch guard in handleSubmit (both the resolve
// and the error branch) and the stream gate on the delta/status listeners
// are what these tests pin hardest. The button renders only when there is
// something to clear.

import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import {
  ASK_OK,
  ATTACHMENT,
  deferred,
  installBackend,
} from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";
import { ASK_CANCELLED } from "./api";
import type { AskResult } from "./types";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

const startOver = () => screen.getByRole("button", { name: "Start over" });

test("Start over is absent on the default panel and appears with a draft", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  expect(screen.queryByRole("button", { name: "Start over" })).toBeNull();
  await user.type(questionBox(), "w");
  expect(startOver()).toBeTruthy();
});

test("clearing after an answer restores the default panel and refocuses the prompt", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await screen.findByText(/raise spawn rates/);
  expect(screen.getByRole("status").textContent).toContain("Answer ready");

  await user.click(startOver());

  expect(questionBox().value).toBe("");
  expect(screen.queryByText(/raise spawn rates/)).toBeNull();
  expect(screen.queryByRole("link")).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("");
  expect(screen.getByText(/anytime to open this panel/)).toBeTruthy();
  expect(document.activeElement).toBe(questionBox());
  // Nothing left to clear — the affordance goes with the content.
  expect(screen.queryByRole("button", { name: "Start over" })).toBeNull();
  expect(backend.callsTo("cancel_ask")).toHaveLength(0);
});

test("clearing drops an attached screenshot through clear_capture", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await fireBackendEvent("capture://attached", ATTACHMENT);
  expect(screen.getByAltText("Screenshot to attach")).toBeTruthy();

  await user.click(startOver());

  expect(screen.queryByAltText("Screenshot to attach")).toBeNull();
  expect(backend.callsTo("clear_capture")).toHaveLength(1);
});

test("mid-stream, Start over stops the ask and the cancelled settle resurrects nothing", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await fireBackendEvent("ask://delta", "Cast the ");
  await user.click(startOver());
  expect(backend.callsTo("cancel_ask")).toHaveLength(1);
  // The wipe already landed, without waiting for the settle.
  expect(screen.queryByText(/Cast the/)).toBeNull();

  // A delta already in flight when the clear landed keeps draining until
  // the Rust abort takes — the stream gate must drop it, not repaint the
  // cleared panel with an orphan fragment.
  await fireBackendEvent("ask://delta", "fishing rod");
  expect(screen.queryByText(/fishing rod/)).toBeNull();
  await fireBackendEvent("ask://status", "answering");
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();

  await act(async () => {
    gate.reject(ASK_CANCELLED);
  });

  expect(screen.queryByText(/Cast the/)).toBeNull();
  expect(document.querySelector(".error")).toBeNull();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  expect(screen.getByText(/anytime to open this panel/)).toBeTruthy();
});

test("an ask that completes after Start over does not resurrect its answer", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await user.click(startOver());

  // Rust finished anyway — the cancel lost the race. The epoch guard makes
  // the resolve drop its own result instead of clobbering the cleared panel.
  await act(async () => {
    gate.resolve(ASK_OK);
  });

  expect(screen.queryByText(/raise spawn rates/)).toBeNull();
  expect(screen.queryByRole("link")).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("");
  expect(screen.getByText(/anytime to open this panel/)).toBeTruthy();
});

test("an ask that fails after Start over does not resurrect its error box", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await user.click(startOver());

  // A real failure (not the cancel sentinel) settling after the clear: the
  // catch branch's epoch check must swallow it — the panel was cleared, and
  // a stale error box on it would be the resolve-race bug in error clothes.
  await act(async () => {
    gate.reject("The wiki is down");
  });

  expect(screen.queryByText(/The wiki is down/)).toBeNull();
  expect(document.querySelector(".error")).toBeNull();
  expect(screen.getByText(/anytime to open this panel/)).toBeTruthy();
});

test("clearing wipes an error box", async () => {
  const backend = installBackend();
  backend.onCommand("ask", () => {
    throw "The wiki is down";
  });
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await screen.findByText(/The wiki is down/);

  await user.click(startOver());
  expect(screen.queryByText(/The wiki is down/)).toBeNull();
  expect(screen.getByText(/anytime to open this panel/)).toBeTruthy();
});
