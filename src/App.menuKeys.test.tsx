// Arrow-key navigation in the owned menus: Up/Down move a visible highlight,
// Home/End jump, Enter picks the highlighted row — never an invisible first
// match (the 2026-07-19 critique's heuristic-5 finding). All keyboard input
// goes through userEvent (harness rule: a raw KeyboardEvent dispatch runs
// capture+bubble in registration order and silently inverts the Esc layering).

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { installBackend } from "./test/backend";
import { renderApp } from "./test/harness";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

function highlighted(dialog: HTMLElement): Element[] {
  return [...dialog.querySelectorAll(".model-row.is-highlighted")];
}

test("game menu: the selected row opens highlighted and bare Enter re-picks it", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");

  // Enter picks the visibly highlighted row — a harmless re-pick, not an
  // invisible first match — closes the menu, and refocuses the prompt.
  await user.keyboard("{Enter}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("button", { name: /^Game: Terraria/ })).toBeTruthy();
  expect(document.activeElement).toBe(questionBox());
});

test("game menu: arrows move the highlight and Enter picks it, focus staying on the filter", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });
  const filter = screen.getByRole("textbox", { name: "Filter games" });

  // No recents yet, so rows are alphabetical: My Wiki, Stardew Valley,
  // Terraria — the selected game opens highlighted at the bottom.
  await user.keyboard("{ArrowUp}");
  let rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Stardew Valley");
  expect(document.activeElement).toBe(filter);

  await user.keyboard("{Enter}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(
    screen.getByRole("button", { name: /^Game: Stardew Valley/ }),
  ).toBeTruthy();
});

test("game menu: Home and End jump within the game rows only", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });

  await user.keyboard("{Home}");
  expect(highlighted(dialog)[0].textContent).toContain("My Wiki");

  await user.keyboard("{End}");
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");
  // The pinned footer action is Tab territory, never arrow territory.
  expect(
    dialog.querySelector(".menu-action")?.classList.contains("is-highlighted"),
  ).toBe(false);
});

test("game menu: typing keeps filtering and Enter picks the visible first match", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });

  await user.keyboard("sta");
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Stardew Valley");

  await user.keyboard("{Enter}");
  expect(
    screen.getByRole("button", { name: /^Game: Stardew Valley/ }),
  ).toBeTruthy();
});

test("game menu: Enter on a no-match filter is a no-op and the menu stays open", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  await user.keyboard("zzz{Enter}");
  expect(screen.getByRole("dialog", { name: "Choose a game" })).toBeTruthy();
  expect(screen.getByRole("button", { name: /^Game: Terraria/ })).toBeTruthy();
});

test("Esc layering is untouched by a moved highlight", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  await user.keyboard("{ArrowUp}{ArrowUp}");

  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);

  await user.keyboard("{Escape}");
  expect(backend.callsTo("hide_overlay")).toHaveLength(1);
});
