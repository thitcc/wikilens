// Theme switching end to end. Ownership under test: the Settings rows only
// report picks; App writes localStorage (wikilens.theme) and mirrors the
// state onto the root element's data-theme — SET for micrographics, DELETED
// for the default, so the bare :root block stays the single source of the
// default appearance (DESIGN.md §8). A theme pick is chrome, never behavior:
// zero IPC rides on it.

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { MICRO_PROMPT_PLACEHOLDER } from "./instrument";
import { APP_VERSION, installBackend } from "./test/backend";
import { renderApp } from "./test/harness";
import { THEME_STORAGE_KEY } from "./theme";

test("a corrupted theme entry still renders, on the default appearance", async () => {
  localStorage.setItem(THEME_STORAGE_KEY, "!!not a theme!!");
  installBackend();
  await renderApp();

  expect(document.documentElement.dataset.theme).toBeUndefined();
  // The reader owned the fallback — nothing "healed" the entry into a write.
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("!!not a theme!!");
});

test("a stored micrographics pick applies on mount", async () => {
  localStorage.setItem(THEME_STORAGE_KEY, "micrographics");
  installBackend();
  await renderApp();

  expect(document.documentElement.dataset.theme).toBe("micrographics");
});

test("the Settings pick round-trips storage and data-theme with zero IPC", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: "Settings" }));
  await screen.findByRole("button", { name: "Add a key for Anthropic" });
  await screen.findByText(`v${APP_VERSION}`);

  const before = backend.calls.length;
  // The theme options live in the stepper value's popover.
  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
  await user.click(
    screen.getByRole("button", { name: "Use the Micrographics theme" }),
  );

  expect(backend.calls.length).toBe(before);
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("micrographics");
  expect(document.documentElement.dataset.theme).toBe("micrographics");

  // Instrument labels: visible text gets underscores, accessible names stay
  // sentence case (CSS owns case, JS owns characters) — the chip is still
  // FOUND by its raw name and RENDERS the transformed one.
  const modelChip = screen.getByRole("button", {
    name: "Model: Anthropic Claude Sonnet 5",
  });
  expect(modelChip.textContent).toContain("Claude_Sonnet_5");
  expect(
    (screen.getByLabelText("Question") as HTMLTextAreaElement).placeholder,
  ).toBe(MICRO_PROMPT_PLACEHOLDER);

  // And back: the default pick removes the attribute rather than writing
  // data-theme="default". The first pick closed the popover — reopen it.
  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
  await user.click(
    screen.getByRole("button", { name: "Use the default theme" }),
  );
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("default");
  expect(document.documentElement.dataset.theme).toBeUndefined();
  expect(backend.calls.length).toBe(before);
});
