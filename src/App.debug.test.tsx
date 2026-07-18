// The footer Debug chip: present only when the backend says the debug window
// exists (WIKILENS_DEBUG at startup), toggles it, and stays usable mid-ask.

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";

import type { AskResult } from "./types";
import { deferred, installBackend } from "./test/backend";
import { renderApp } from "./test/harness";

test("no Debug chip when the debug window doesn't exist", async () => {
  installBackend(); // debug_available defaults to false
  await renderApp();
  expect(screen.queryByRole("button", { name: "Debug" })).toBeNull();
});

test("Debug chip renders when available and toggles the window", async () => {
  const backend = installBackend({ debug_available: () => true });
  const user = userEvent.setup();
  await renderApp();

  const chip = await screen.findByRole("button", { name: "Debug" });
  await user.click(chip);
  expect(backend.callsTo("toggle_debug_window")).toHaveLength(1);
  await user.click(chip);
  expect(backend.callsTo("toggle_debug_window")).toHaveLength(2);
});

test("Debug chip stays clickable while an ask is in flight", async () => {
  const backend = installBackend({ debug_available: () => true });
  const gate = deferred<AskResult>();
  backend.onCommand("ask", () => gate.promise);
  const user = userEvent.setup();
  await renderApp();

  await user.type(
    screen.getByRole("textbox", { name: "Question" }),
    "how do I fish{Enter}",
  );

  const chip = await screen.findByRole("button", { name: "Debug" });
  expect((chip as HTMLButtonElement).disabled).toBe(false);
  await user.click(chip);
  expect(backend.callsTo("toggle_debug_window")).toHaveLength(1);
});
