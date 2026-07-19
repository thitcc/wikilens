// Select-all suppression on re-summon: overlay://shown selects the old
// question so the first keystroke starts a new one — unless the panel was
// hidden moments ago (overlay://hidden), where the "old question" is a live
// draft (the capital-C trap: typing `C` fires the global Shift+C toggle).
// Own file: this is a fake-timer regime, same rules as App.slowHint.test.tsx.

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { SELECT_SUPPRESS_MS } from "./App";
import { installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

/** Mount, type a draft, then hide the overlay. */
async function draftThenHide() {
  installBackend();
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  await renderApp();
  await user.type(questionBox(), "how do I fish");
  await fireBackendEvent("overlay://hidden");
  return user;
}

test("a re-summon right after a hide keeps the draft unselected", async () => {
  const user = await draftThenHide();

  vi.advanceTimersByTime(500);
  await fireBackendEvent("overlay://shown");

  // Suppressed select-all: the next keystroke appends instead of replacing.
  await user.keyboard("?");
  expect(questionBox().value).toBe("how do I fish?");
});

test("a re-summon after the window selects the old question as always", async () => {
  const user = await draftThenHide();

  // Comfortably past the threshold (the creeping clock forbids 1ms margins).
  vi.advanceTimersByTime(SELECT_SUPPRESS_MS + 2_000);
  await fireBackendEvent("overlay://shown");

  // Standard summon contract: the old question is selected, first keystroke
  // starts the new one.
  await user.keyboard("?");
  expect(questionBox().value).toBe("?");
});
