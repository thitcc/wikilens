import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { HistoryMenu } from "./HistoryMenu";
import { HISTORY } from "../test/backend";

// Direct-render suite for the menu's own contract (filter, keyboard, clear).
// App.history.test.tsx covers the user-visible flows around it (chip gating,
// restore, refresh).

function renderMenu(overrides: Partial<Parameters<typeof HistoryMenu>[0]> = {}) {
  const onPick = vi.fn();
  const onClear = vi.fn(() => Promise.resolve());
  const onClose = vi.fn();
  render(
    <HistoryMenu
      entries={HISTORY}
      onPick={onPick}
      onClear={onClear}
      onClose={onClose}
      chipRef={{ current: null }}
      {...overrides}
    />,
  );
  return { onPick, onClear, onClose };
}

function highlighted(): Element[] {
  return [...document.querySelectorAll(".model-row.is-highlighted")];
}

function filterBox(): HTMLElement {
  return screen.getByRole("textbox", { name: "Filter questions" });
}

test("rows render newest first: question over a game meta line", () => {
  renderMenu();
  const rows = [...document.querySelectorAll(".model-row")];
  expect(rows).toHaveLength(2);
  expect(rows[0].textContent).toContain("best winter crops");
  expect(rows[0].textContent).toContain("Stardew Valley ·");
  expect(rows[1].textContent).toContain("question about a removed game");
  // The removed game still names itself — gameName is denormalized.
  expect(rows[1].textContent).toContain("Gone Game ·");
});

test("the initial highlight is the newest entry, and Enter picks it", async () => {
  const { onPick } = renderMenu();
  expect(highlighted()).toHaveLength(1);
  expect(highlighted()[0].textContent).toContain("best winter crops");

  const user = userEvent.setup();
  await user.click(filterBox());
  await user.keyboard("{Enter}");
  expect(onPick).toHaveBeenCalledWith(HISTORY[0]);
});

test("arrow movement then Enter picks the highlighted entry", async () => {
  const { onPick } = renderMenu();
  const user = userEvent.setup();
  await user.click(filterBox());

  await user.keyboard("{ArrowDown}");
  expect(highlighted()[0].textContent).toContain("removed game");
  await user.keyboard("{Enter}");
  expect(onPick).toHaveBeenCalledWith(HISTORY[1]);
});

test("the filter narrows by question text", async () => {
  renderMenu();
  const user = userEvent.setup();
  await user.type(filterBox(), "winter");
  const rows = [...document.querySelectorAll(".model-row")];
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("best winter crops");
});

test("a no-match filter shows the note and Enter is a no-op", async () => {
  const { onPick } = renderMenu();
  const user = userEvent.setup();
  await user.type(filterBox(), "zzz");
  expect(screen.getByText("No matches")).toBeTruthy();
  expect(highlighted()).toHaveLength(0);
  await user.keyboard("{Enter}");
  expect(onPick).not.toHaveBeenCalled();
});

test("Esc closes the menu", async () => {
  const { onClose } = renderMenu();
  const user = userEvent.setup();
  await user.click(filterBox());
  await user.keyboard("{Escape}");
  expect(onClose).toHaveBeenCalled();
});

test("the pinned Clear history action fires onClear", async () => {
  const { onClear } = renderMenu();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Clear history" }));
  expect(onClear).toHaveBeenCalled();
});

test("a rejected clear surfaces inline and the menu stays up", async () => {
  renderMenu({
    onClear: vi.fn(() =>
      Promise.reject(new Error("history file is locked")),
    ),
  });
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Clear history" }));
  await screen.findByText(/history file is locked/);
  // Still a dialog with its rows — nothing closed or emptied locally.
  expect(screen.getByRole("dialog", { name: "Answer history" })).toBeTruthy();
  expect(document.querySelectorAll(".model-row")).toHaveLength(2);
});
