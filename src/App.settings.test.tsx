// The overlay's shortcut copy is config-driven: the prompt placeholder and
// the capture chip title render the combos get_settings reports, falling
// back to the shipped defaults when it fails (the gear then stays disabled —
// the popover would have nothing to edit).

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SETTINGS, installBackend } from "./test/backend";
import { renderApp } from "./test/harness";
import type { SettingsInfo } from "./types";

test("the placeholder and capture title render the configured combos", async () => {
  const custom: SettingsInfo = {
    ...SETTINGS,
    hotkeys: {
      summon: {
        ...SETTINGS.hotkeys.summon,
        accelerator: "Alt+KeyW",
        label: "Alt+W",
        isDefault: false,
      },
      capture: {
        ...SETTINGS.hotkeys.capture,
        accelerator: "Ctrl+Alt+KeyS",
        label: "Ctrl+Alt+S",
        isDefault: false,
      },
    },
  };
  installBackend({ get_settings: () => custom });
  await renderApp();

  expect(screen.getByText(/anytime to open this panel/).textContent).toContain(
    "Alt+W",
  );
  expect(screen.getByTitle("Capture a screenshot (Ctrl+Alt+S)")).toBeTruthy();
});

test("a failed get_settings falls back to the shipped defaults", async () => {
  installBackend({
    get_settings: () => {
      throw new Error("settings unavailable");
    },
  });
  await renderApp();

  expect(screen.getByText(/anytime to open this panel/).textContent).toContain(
    "Ctrl+`",
  );
  expect(screen.getByTitle("Capture a screenshot (Ctrl+Shift+C)")).toBeTruthy();
  // No data to edit — the gear is disabled, not broken.
  const gear = screen.getByRole("button", { name: "Settings" });
  expect(gear.hasAttribute("disabled")).toBe(true);
});

test("the header gear opens the Settings panel", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
  // Settle the open-time key-status fetch on the unkeyed Anthropic row (the
  // key field only exists once that row is clicked open).
  await screen.findByRole("button", { name: "Add a key for Anthropic" });

  // Esc layering holds for the settings menu too.
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);
});

test("the settings menu joins the one-open-menu rule", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
  await screen.findByRole("button", { name: "Add a key for Anthropic" });

  // Clicking the game chip: outside-pointerdown closes settings, the chip
  // click opens the game menu — never both.
  await user.click(screen.getByRole("button", { name: /^Game: / }));
  const dialogs = screen.getAllByRole("dialog");
  expect(dialogs).toHaveLength(1);
  expect(dialogs[0].getAttribute("aria-label")).toBe("Choose a game");
});
