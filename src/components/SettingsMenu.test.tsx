// The recorder's contract: arming suspends the OS hotkeys and swallows keys
// at capture phase; every exit path (save, refuse, Esc, hide, unmount)
// resumes them exactly once. Keyboard goes through userEvent only (see
// harness.tsx — a raw window KeyboardEvent would invert the capture/bubble
// ordering these tests exist to pin).

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SettingsMenu } from "./SettingsMenu";
import { SETTINGS, installBackend } from "../test/backend";
import { fireBackendEvent } from "../test/harness";
import type { SettingsInfo } from "../types";

function renderMenu(over?: {
  settings?: SettingsInfo;
  onSaved?: (next: SettingsInfo) => void;
  onClose?: () => void;
}) {
  const triggerRef = { current: null };
  const onSaved = over?.onSaved ?? (() => {});
  const onClose = over?.onClose ?? (() => {});
  return render(
    <SettingsMenu
      settings={over?.settings ?? SETTINGS}
      onSaved={onSaved}
      onClose={onClose}
      triggerRef={triggerRef}
    />,
  );
}

/** SETTINGS with a non-default summon, for the Reset affordance. */
const CUSTOM_SUMMON: SettingsInfo = {
  hotkeys: {
    ...SETTINGS.hotkeys,
    summon: {
      ...SETTINGS.hotkeys.summon,
      accelerator: "Ctrl+Alt+KeyP",
      label: "Ctrl+Alt+P",
      isDefault: false,
    },
  },
};

test("renders both shortcuts as key chips", () => {
  installBackend();
  renderMenu();

  const dialog = screen.getByRole("dialog", { name: "Shortcuts" });
  expect(dialog.textContent).toContain("Summon");
  expect(dialog.textContent).toContain("Ctrl+`");
  expect(dialog.textContent).toContain("Capture");
  expect(dialog.textContent).toContain("Ctrl+Shift+C");
});

test("recording a valid combo suspends, saves, and resumes", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  renderMenu({ onSaved: (next) => saved.push(next) });

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  expect(backend.callsTo("suspend_hotkeys")).toHaveLength(1);
  expect(screen.getByText(/Press the new shortcut/)).toBeTruthy();

  await user.keyboard("{Control>}{Alt>}p{/Alt}{/Control}");
  expect(backend.callsTo("set_hotkey")).toEqual([
    { role: "summon", accelerator: "Ctrl+Alt+KeyP" },
  ]);
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(1);
  expect(saved).toHaveLength(1);
  // Disarmed again: the wait prompt is gone.
  expect(screen.queryByText(/Press the new shortcut/)).toBeNull();
});

test("a Shift-only combo is refused with the trap rationale and stays armed", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  await user.keyboard("{Shift>}c{/Shift}");

  expect(screen.getByText(/block typing/).textContent).toContain(
    "add Ctrl or Alt",
  );
  expect(backend.callsTo("set_hotkey")).toHaveLength(0);
  // Still armed (Shift released, so the wait prompt is back).
  expect(screen.getByText(/Press the new shortcut/)).toBeTruthy();
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(0);
});

test("recording the other role's combo is refused", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  await user.keyboard("{Control>}{Shift>}c{/Shift}{/Control}");

  expect(
    screen.getByText(/already your Capture shortcut/),
  ).toBeTruthy();
  expect(backend.callsTo("set_hotkey")).toHaveLength(0);
});

test("a rejected save shows the message and still resumes", async () => {
  const backend = installBackend();
  backend.onCommand("set_hotkey", () => {
    throw "Couldn't claim Ctrl+Alt+P — another app may already be using it.";
  });
  const user = userEvent.setup();
  renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  await user.keyboard("{Control>}{Alt>}p{/Alt}{/Control}");

  expect(screen.getByText(/another app may already be using it/)).toBeTruthy();
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(1);
});

test("Esc is three-layered while armed: cancel recording, then close", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  let closed = 0;
  renderMenu({ onClose: () => closed++ });

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  await user.keyboard("{Escape}");
  // First Esc cancels the recording only — the menu stays, hotkeys resume.
  expect(closed).toBe(0);
  expect(screen.queryByText(/Press the new shortcut/)).toBeNull();
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(1);
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);

  await user.keyboard("{Escape}");
  expect(closed).toBe(1);
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);
});

test("unmounting while armed resumes the hotkeys", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const { unmount } = renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  expect(backend.callsTo("suspend_hotkeys")).toHaveLength(1);

  unmount();
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(1);
});

test("the overlay hiding while armed disarms and resumes", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );
  await fireBackendEvent("overlay://hidden");

  expect(screen.queryByText(/Press the new shortcut/)).toBeNull();
  expect(backend.callsTo("resume_hotkeys")).toHaveLength(1);
});

test("Reset saves the default without recording or suspension", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  renderMenu({ settings: CUSTOM_SUMMON, onSaved: (next) => saved.push(next) });

  await user.click(
    screen.getByRole("button", {
      name: "Reset the Summon shortcut to Ctrl+`",
    }),
  );
  expect(backend.callsTo("set_hotkey")).toEqual([
    { role: "summon", accelerator: "Ctrl+Backquote" },
  ]);
  expect(backend.callsTo("suspend_hotkeys")).toHaveLength(0);
  expect(saved).toHaveLength(1);
});

test("a default shortcut offers no Reset", () => {
  installBackend();
  renderMenu();
  expect(
    screen.queryByRole("button", { name: /Reset the Summon shortcut/ }),
  ).toBeNull();
});
