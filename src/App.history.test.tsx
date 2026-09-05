import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import type { AskResult } from "./types";
import { ASK_CANCELLED } from "./api";
import { ASK_OK, HISTORY, deferred, installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

function historyChip(): HTMLButtonElement | null {
  return screen.queryByRole("button", {
    name: "History",
  }) as HTMLButtonElement | null;
}

test("the History chip is absent until history exists", async () => {
  installBackend(); // list_history defaults to []
  await renderApp();
  expect(historyChip()).toBeNull();
});

test("a completed ask refreshes the list and the chip appears", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();
  expect(historyChip()).toBeNull();

  // Rust records the ask; the frontend just re-lists after the submit settles.
  backend.onCommand("list_history", () => HISTORY);
  await user.type(questionBox(), "how do I fish{Enter}");
  await screen.findByText(/to raise spawn rates/);

  await screen.findByRole("button", { name: "History" });
  // Once on mount, once from the submit's finally.
  expect(backend.callsTo("list_history")).toHaveLength(2);
});

test("picking an entry restores answer, sources, question, and game", async () => {
  installBackend({ list_history: () => HISTORY });
  const user = userEvent.setup();
  await renderApp();

  await user.click(historyChip()!);
  const dialog = await screen.findByRole("dialog", { name: "Answer history" });
  expect(dialog.textContent).toContain("best winter crops");

  await user.click(screen.getByRole("button", { name: /best winter crops/ }));

  // The panel looks exactly as it did when that answer landed: prompt holds
  // the question (Enter re-asks), answer + receipts below, right game armed.
  expect(questionBox().value).toBe("best winter crops");
  expect(screen.getByText("Winter Seeds")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Winter" })).toBeTruthy();
  expect(
    screen.getByRole("button", { name: /^Game: Stardew Valley/ }),
  ).toBeTruthy();
  // Menu closed, focus back on the prompt (the closeMenu contract).
  expect(screen.queryByRole("dialog", { name: "Answer history" })).toBeNull();
  expect(document.activeElement).toBe(questionBox());
});

test("picking an entry whose game was removed keeps the selection", async () => {
  installBackend({ list_history: () => HISTORY });
  const user = userEvent.setup();
  await renderApp();

  await user.click(historyChip()!);
  await user.click(
    screen.getByRole("button", { name: /question about a removed game/ }),
  );

  // The answer restores; the game selection stays put — there is no
  // "gone-game" to switch to, and the row already named it via gameName.
  expect(questionBox().value).toBe("question about a removed game");
  expect(screen.getByText("old")).toBeTruthy();
  expect(screen.getByRole("button", { name: /^Game: Terraria/ })).toBeTruthy();
});

test("Clear history unmounts the chip and leaves the panel untouched", async () => {
  const backend = installBackend({ list_history: () => HISTORY });
  const user = userEvent.setup();
  await renderApp();

  // Put a restored answer on the panel first, so "untouched" is observable.
  await user.click(historyChip()!);
  await user.click(screen.getByRole("button", { name: /best winter crops/ }));
  expect(screen.getByText("Winter Seeds")).toBeTruthy();

  await user.click(historyChip()!);
  await user.click(screen.getByRole("button", { name: "Clear history" }));

  expect(backend.callsTo("clear_history")).toHaveLength(1);
  // Chip and menu gone (nothing left to open); the displayed answer stays —
  // clearing history is about the store, not the current panel.
  expect(historyChip()).toBeNull();
  expect(screen.queryByRole("dialog", { name: "Answer history" })).toBeNull();
  expect(screen.getByText("Winter Seeds")).toBeTruthy();
});

test("a rejected clear re-lists from the store and keeps the menu open", async () => {
  const backend = installBackend({
    list_history: () => HISTORY,
    clear_history: () => {
      throw new Error("history.json.bak is in use");
    },
  });
  const user = userEvent.setup();
  await renderApp();
  await user.click(historyChip()!);
  expect(document.querySelectorAll(".model-row")).toHaveLength(2);

  // Rust is the store of record: whatever it lists after the failure is what
  // the menu shows — here one row survived, so the other must disappear.
  backend.onCommand("list_history", () => [HISTORY[0]]);
  await user.click(screen.getByRole("button", { name: "Clear history" }));

  await screen.findByText(/history\.json\.bak is in use/);
  expect(screen.getByRole("dialog", { name: "Answer history" })).toBeTruthy();
  expect(document.querySelectorAll(".model-row")).toHaveLength(1);
  // Once on mount, once from the rejection.
  expect(backend.callsTo("list_history")).toHaveLength(2);
});

test("a rejected clear whose re-list also fails keeps the rows and the clear's error", async () => {
  const backend = installBackend({
    list_history: () => HISTORY,
    clear_history: () => {
      throw new Error("history.json.bak is in use");
    },
  });
  const user = userEvent.setup();
  await renderApp();
  await user.click(historyChip()!);

  // The re-list is a best-effort resync: when it fails too, the rows already
  // shown stay, and the message is the clear's — never the re-list's.
  backend.onCommand("list_history", () => {
    throw new Error("store unreadable");
  });
  await user.click(screen.getByRole("button", { name: "Clear history" }));

  await screen.findByText(/history\.json\.bak is in use/);
  expect(screen.queryByText(/store unreadable/)).toBeNull();
  expect(document.querySelectorAll(".model-row")).toHaveLength(2);
  expect(backend.callsTo("list_history")).toHaveLength(2);
});

test("the chip is disabled while an ask is in flight", async () => {
  const backend = installBackend({ list_history: () => HISTORY });
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  expect(historyChip()!.disabled).toBe(true);

  await act(async () => {
    gate.resolve(ASK_OK);
  });
  expect(historyChip()!.disabled).toBe(false);
});

test("a delta draining after Stop cannot append onto a restored answer", async () => {
  const backend = installBackend({ list_history: () => HISTORY });
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await fireBackendEvent("ask://delta", "partial text ");
  await user.click(screen.getByRole("button", { name: "Stop" }));
  await act(async () => {
    gate.reject(ASK_CANCELLED);
  });
  // The plain-Stop contract: the partial stays.
  expect(screen.getByText(/partial text/)).toBeTruthy();

  await user.click(historyChip()!);
  await user.click(screen.getByRole("button", { name: /best winter crops/ }));
  expect(screen.getByText("Winter Seeds")).toBeTruthy();

  // A straggler delta from the aborted stream lands after the restore — the
  // epoch bump in handleHistoryPick must drop it, not append it.
  await fireBackendEvent("ask://delta", "ZOMBIE");
  expect(screen.queryByText(/ZOMBIE/)).toBeNull();
  expect(screen.getByText("Winter Seeds")).toBeTruthy();
});
