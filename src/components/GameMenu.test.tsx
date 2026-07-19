import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { GameMenu } from "./GameMenu";
import { GAMES } from "../test/backend";

// Direct-render suite for the highlight edge cases that are too fiddly at App
// level (the sectioned Recent/All list). App.menuKeys.test.tsx covers the
// user-visible flows.

function renderMenu(overrides: Partial<Parameters<typeof GameMenu>[0]> = {}) {
  const onSelect = vi.fn();
  render(
    <GameMenu
      games={GAMES}
      selectedId="terraria"
      recentIds={["terraria"]}
      onSelect={onSelect}
      onAddGame={() => {}}
      onClose={() => {}}
      chipRef={{ current: null }}
      {...overrides}
    />,
  );
  return { onSelect };
}

function highlighted(): Element[] {
  return [...document.querySelectorAll(".model-row.is-highlighted")];
}

function filterBox(): HTMLElement {
  return screen.getByRole("textbox", { name: "Filter games" });
}

test("the initial highlight is the selected game's Recent copy", () => {
  renderMenu();
  const rows = highlighted();
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");
  // The Recent copy renders before the "All games" heading — it must be the
  // first row overall, not the alphabetical copy further down.
  const allRows = [...document.querySelectorAll(".model-row")];
  expect(allRows.indexOf(rows[0])).toBe(0);
});

test("arrows clamp at both ends instead of wrapping", async () => {
  renderMenu();
  const user = userEvent.setup();
  await user.click(filterBox());

  // Highlight starts at the first row (the Recent Terraria copy).
  await user.keyboard("{ArrowUp}");
  expect(highlighted()[0].textContent).toContain("Terraria");

  // Rows: recent-terraria, then All games alphabetical (My Wiki, Stardew
  // Valley, Terraria). End lands on the last row; ArrowDown stays there.
  await user.keyboard("{End}");
  let rows = highlighted();
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");
  const allRows = [...document.querySelectorAll(".model-row")];
  expect(allRows.indexOf(rows[0])).toBe(allRows.length - 1);

  await user.keyboard("{ArrowDown}");
  rows = highlighted();
  expect([...document.querySelectorAll(".model-row")].indexOf(rows[0])).toBe(
    allRows.length - 1,
  );
});

test("End never reaches the pinned footer action", async () => {
  renderMenu();
  const user = userEvent.setup();
  await user.click(filterBox());
  await user.keyboard("{End}");
  expect(
    document.querySelector(".menu-action")?.classList.contains("is-highlighted"),
  ).toBe(false);
  expect(highlighted()).toHaveLength(1);
});

test("arrow movement then Enter picks the highlighted game", async () => {
  const { onSelect } = renderMenu();
  const user = userEvent.setup();
  await user.click(filterBox());

  // From recent-terraria (row 0), ArrowDown reaches All games' first row.
  await user.keyboard("{ArrowDown}");
  expect(highlighted()[0].textContent).toContain("My Wiki");
  await user.keyboard("{Enter}");
  expect(onSelect).toHaveBeenCalledWith("my-wiki");
});

test("clearing the filter re-anchors the highlight on the selected game", async () => {
  renderMenu();
  const user = userEvent.setup();
  await user.type(filterBox(), "sta");
  expect(highlighted()[0].textContent).toContain("Stardew Valley");

  await user.clear(filterBox());
  const rows = highlighted();
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");
});

test("a no-match filter leaves nothing highlighted and Enter a no-op", async () => {
  const { onSelect } = renderMenu();
  const user = userEvent.setup();
  await user.type(filterBox(), "zzz");
  expect(highlighted()).toHaveLength(0);
  await user.keyboard("{Enter}");
  expect(onSelect).not.toHaveBeenCalled();
});
