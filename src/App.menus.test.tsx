import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

test("Esc is layered: first closes the model menu, second hides the overlay", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Model: / }));
  expect(screen.getByRole("dialog", { name: "Choose a model" })).toBeTruthy();

  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  // The menu's capture-phase handler stopped propagation — the overlay stayed.
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);

  await user.keyboard("{Escape}");
  expect(backend.callsTo("hide_overlay")).toHaveLength(1);
});

test("Esc closes the game and add-game menus without hiding the overlay", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: / }));
  expect(screen.getByRole("dialog", { name: "Choose a game" })).toBeTruthy();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: /^Game: / }));
  await user.click(screen.getByRole("button", { name: /Add a game…/ }));
  expect(screen.getByRole("dialog", { name: "Add a game" })).toBeTruthy();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);
});

test("at most one menu is open at a time", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  // Game menu → add-game menu is a direct swap.
  await user.click(screen.getByRole("button", { name: /^Game: / }));
  await user.click(screen.getByRole("button", { name: /Add a game…/ }));
  let dialogs = screen.getAllByRole("dialog");
  expect(dialogs).toHaveLength(1);
  expect(dialogs[0].getAttribute("aria-label")).toBe("Add a game");

  // Clicking the model chip while add-game is open: outside-pointerdown
  // closes add-game, the chip click opens the model menu — never both.
  await user.click(screen.getByRole("button", { name: /^Model: / }));
  dialogs = screen.getAllByRole("dialog");
  expect(dialogs).toHaveLength(1);
  expect(dialogs[0].getAttribute("aria-label")).toBe("Choose a model");
});

test("overlay://shown closes whatever menu is open", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: / }));
  expect(screen.getByRole("dialog", { name: "Choose a game" })).toBeTruthy();

  await fireBackendEvent("overlay://shown");
  expect(screen.queryByRole("dialog")).toBeNull();
});
