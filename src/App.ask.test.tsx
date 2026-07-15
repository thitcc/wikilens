import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import type { AskResult } from "./types";
import { PROVIDER_STORAGE_KEY } from "./modelPick";
import { ASK_OK, ATTACHMENT, deferred, installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

test("ask flow: status transitions, delta accumulation, args, busy reset", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);

  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");

  // Submit set the first phase synchronously and disabled the input.
  expect(screen.getByText("Searching the wiki…")).toBeTruthy();
  expect(questionBox().disabled).toBe(true);

  await fireBackendEvent("ask://status", "understanding");
  expect(screen.getByText("Understanding your question…")).toBeTruthy();

  await fireBackendEvent("ask://status", "reading");
  expect(screen.getByText("Reading pages…")).toBeTruthy();

  // Deltas append — the audit regression was a replace instead of prev + chunk.
  await fireBackendEvent("ask://delta", "Cast the ");
  await fireBackendEvent("ask://delta", "fishing rod");
  expect(screen.getByText("Cast the fishing rod")).toBeTruthy();

  expect(backend.callsTo("ask")).toEqual([
    {
      gameId: "terraria",
      providerId: "anthropic",
      model: "claude-sonnet-5",
      question: "how do I fish",
      imageId: null,
    },
  ]);

  await act(async () => {
    gate.resolve(ASK_OK);
  });

  // The final result replaces the stream; busy is over, status gone.
  expect(screen.getByText(/to raise spawn rates/)).toBeTruthy();
  expect(screen.queryByText("Reading pages…")).toBeNull();
  expect(questionBox().disabled).toBe(false);
});

test("a rejected ask shows the error, keeps the attachment, re-enables input", async () => {
  const backend = installBackend({
    ask: () => {
      throw new Error("provider exploded");
    },
  });
  const user = userEvent.setup();
  await renderApp();

  await fireBackendEvent("capture://attached", ATTACHMENT);
  expect(screen.getByAltText("Screenshot to attach")).toBeTruthy();

  await user.type(questionBox(), "what is this{Enter}");

  await screen.findByText(/provider exploded/);
  // Failed ask: the screenshot is NOT spent — the strip survives for a retry.
  expect(screen.getByAltText("Screenshot to attach")).toBeTruthy();
  expect(questionBox().disabled).toBe(false);
  expect(backend.callsTo("ask")).toHaveLength(1);
});

test("a successful ask clears the attachment and echoes its id", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await fireBackendEvent("capture://attached", ATTACHMENT);
  await user.type(questionBox(), "what is this{Enter}");

  await screen.findByText(/to raise spawn rates/);
  expect(screen.queryByAltText("Screenshot to attach")).toBeNull();
  const [args] = backend.callsTo("ask") as [{ imageId: string | null }];
  expect(args.imageId).toBe("cap-1");
});

test("Enter is a no-op when an attachment meets a text-only model", async () => {
  localStorage.setItem(PROVIDER_STORAGE_KEY, "deepseek");
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await fireBackendEvent("capture://attached", ATTACHMENT);
  expect(
    screen.getByText(/This model can't read images/),
  ).toBeTruthy();

  await user.type(questionBox(), "what is this{Enter}");
  expect(backend.callsTo("ask")).toHaveLength(0);
  expect(questionBox().disabled).toBe(false);
});

test("the capture hotkey sees current busy state, not a stale closure", async () => {
  const backend = installBackend();
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await fireBackendEvent("capture://hotkey");
  // Busy: the hotkey must not start a capture mid-ask.
  expect(backend.callsTo("begin_capture")).toHaveLength(0);

  await act(async () => {
    gate.resolve(ASK_OK);
  });
  await fireBackendEvent("capture://hotkey");
  expect(backend.callsTo("begin_capture")).toHaveLength(1);
});
