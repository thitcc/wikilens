// The status row's Stop action: a cancelled ask resets quietly — no error
// box, the partial stream stays, the prompt keeps focus. The gate rejects
// with the real ASK_CANCELLED constant so these tests also pin the api.ts
// mirror of the Rust sentinel.

import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import type { AskResult } from "./types";
import { ASK_CANCELLED } from "./api";
import { ATTACHMENT, deferred, installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

test("Stop cancels quietly: no error box, partial answer kept, focus retained", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);

  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await fireBackendEvent("ask://delta", "Cast the ");
  await fireBackendEvent("ask://delta", "fishing rod");

  await user.click(screen.getByRole("button", { name: "Stop" }));
  expect(backend.callsTo("cancel_ask")).toHaveLength(1);

  // Rust settles the pending ask with the sentinel.
  await act(async () => {
    gate.reject(ASK_CANCELLED);
  });

  // Quiet reset: the streamed partial survives, no error, status row gone,
  // input writable and still focused for the next question.
  expect(screen.getByText("Cast the fishing rod")).toBeTruthy();
  expect(document.querySelector(".error")).toBeNull();
  expect(screen.queryByText("Searching the wiki…")).toBeNull();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  expect(questionBox().readOnly).toBe(false);
  expect(document.activeElement).toBe(questionBox());
  // Quiet means quiet for assistive tech too: a cancelled ask never claims
  // "Answer ready" (sources were never set).
  expect(screen.getByRole("status").textContent).not.toContain("Answer ready");
});

test("a double Stop is safe", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);

  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  const stop = screen.getByRole("button", { name: "Stop" });
  await user.click(stop);
  await user.click(stop);
  expect(backend.callsTo("cancel_ask")).toHaveLength(2);

  await act(async () => {
    gate.reject(ASK_CANCELLED);
  });
  expect(document.querySelector(".error")).toBeNull();
  expect(questionBox().readOnly).toBe(false);
});

test("a cancelled ask keeps the attachment, like a failed one", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);

  const user = userEvent.setup();
  await renderApp();

  await fireBackendEvent("capture://attached", ATTACHMENT);
  await user.type(questionBox(), "what is this{Enter}");
  await user.click(screen.getByRole("button", { name: "Stop" }));
  await act(async () => {
    gate.reject(ASK_CANCELLED);
  });

  // Not spent: the strip survives for the retry.
  expect(screen.getByAltText("Screenshot to attach")).toBeTruthy();
});
