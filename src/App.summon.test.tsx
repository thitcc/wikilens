// Select-all suppression on re-summon: overlay://shown selects the old
// question so the first keystroke starts a new one — unless the panel was
// hidden moments ago (overlay://hidden), where the "old question" is a live
// draft. Born as the capital-C trap fix (the old Shift+C default made typing
// `C` fire the toggle); kept because any accidental hide arms the same loss.
// Own file: this is a fake-timer regime, same rules as App.slowHint.test.tsx.

import { fireEvent, screen } from "@testing-library/react";
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

/** jsdom has no real AnimationEvent: build a bubbling native event and pin
 *  animationName on it — React's synthetic event reads it straight off.
 *  Without window.AnimationEvent, React delegates the webkit-prefixed type
 *  instead of "animationend", so fire both (the second is a no-op). */
function fireAnimationEnd(el: Element, animationName: string) {
  for (const type of ["animationend", "webkitAnimationEnd"]) {
    const ev = new Event(type, { bubbles: true });
    Object.assign(ev, { animationName });
    fireEvent(el, ev);
  }
}

test("summon arms the A-01 entrance class; animationend and hide clear it", async () => {
  installBackend();
  await renderApp();
  const panel = document.querySelector(".panel");
  if (!panel) throw new Error("panel not rendered");

  expect(panel.classList.contains("panel--summoning")).toBe(false);

  await fireBackendEvent("overlay://shown");
  expect(panel.classList.contains("panel--summoning")).toBe(true);

  // The clear is name-filtered: a child animation's end bubbles up to the
  // panel and must not disarm the summon entrance.
  fireAnimationEnd(panel, "attach-confirm");
  expect(panel.classList.contains("panel--summoning")).toBe(true);

  fireAnimationEnd(panel, "panel-summon");
  expect(panel.classList.contains("panel--summoning")).toBe(false);

  // Reduced-motion path: animationend never fires — hide clears instead.
  await fireBackendEvent("overlay://shown");
  await fireBackendEvent("overlay://hidden");
  expect(panel.classList.contains("panel--summoning")).toBe(false);
});
