// The Settings panel's contract. Recorder half: arming suspends the OS
// hotkeys and swallows keys at capture phase; every exit path (save, refuse,
// Esc, hide, unmount) resumes them exactly once. Keys half: a pasted key
// crosses IPC exactly once via set_api_key and is never displayed back — a
// stored key renders as presence + Remove only. Keyboard goes through
// userEvent only (see harness.tsx — a raw window KeyboardEvent would invert
// the capture/bubble ordering these tests exist to pin).

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SettingsMenu } from "./SettingsMenu";
import { KEY_STATUS, SETTINGS, installBackend } from "../test/backend";
import { fireBackendEvent } from "../test/harness";
import type { KeyStatus, SettingsInfo } from "../types";

/** Mounts the menu and settles the open-time list_key_status fetch (the
 * default settle target is the keyless Anthropic input; tests that override
 * the key fixtures pass their own). */
async function renderMenu(over?: {
  settings?: SettingsInfo;
  onSaved?: (next: SettingsInfo) => void;
  onKeysChanged?: () => void;
  onClose?: () => void;
  settle?: () => Promise<unknown>;
}) {
  const triggerRef = { current: null };
  const onSaved = over?.onSaved ?? (() => {});
  const onClose = over?.onClose ?? (() => {});
  const result = render(
    <SettingsMenu
      settings={over?.settings ?? SETTINGS}
      onSaved={onSaved}
      onKeysChanged={over?.onKeysChanged}
      onClose={onClose}
      triggerRef={triggerRef}
    />,
  );
  await (over?.settle?.() ?? screen.findByLabelText("Anthropic API key"));
  return result;
}

/** SETTINGS with a non-default summon, for the Reset affordance. */
const CUSTOM_SUMMON: SettingsInfo = {
  ...SETTINGS,
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

/** KEY_STATUS with the Anthropic key stored. */
const ANTHROPIC_KEYED: KeyStatus[] = KEY_STATUS.map((s) =>
  s.id === "anthropic" ? { ...s, hasKey: true } : s,
);

test("renders the three sections in order with both shortcut chips", async () => {
  installBackend();
  await renderMenu();

  const dialog = screen.getByRole("dialog", { name: "Settings" });
  const text = dialog.textContent ?? "";
  const order = [
    text.indexOf("Model source"),
    text.indexOf("API keys"),
    text.indexOf("Shortcuts"),
  ];
  expect(Math.min(...order)).toBeGreaterThanOrEqual(0);
  expect([...order].sort((a, b) => a - b)).toEqual(order);
  expect(text).toContain("Summon");
  expect(text).toContain("Ctrl+`");
  expect(text).toContain("Capture");
  expect(text).toContain("Ctrl+Shift+C");
});

// ---- API keys -------------------------------------------------------------

test("keyless providers render masked inputs with a gated Save", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  const input = screen.getByLabelText("Anthropic API key") as HTMLInputElement;
  expect(input.type).toBe("password");
  expect(input.getAttribute("autocomplete")).toBe("off");
  expect(screen.getByLabelText("DeepSeek API key")).toBeTruthy();

  // Save arms only once there is text.
  const save = screen.getByRole("button", { name: "Save the Anthropic API key" });
  expect(save.hasAttribute("disabled")).toBe(true);
  await user.type(input, "sk-ant-test");
  expect(save.hasAttribute("disabled")).toBe(false);
});

test("a stored key renders as Key set with Remove only", async () => {
  installBackend({ list_key_status: () => ANTHROPIC_KEYED });
  await renderMenu({
    settle: () =>
      screen.findByRole("button", { name: "Remove the Anthropic API key" }),
  });

  expect(screen.getByText("Key set")).toBeTruthy();
  // The key itself never renders — no input for a keyed provider.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  // The keyless provider keeps its input.
  expect(screen.getByLabelText("DeepSeek API key")).toBeTruthy();
});

test("pasting a key and saving sends it once and flips the row", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  await renderMenu();

  await user.type(screen.getByLabelText("Anthropic API key"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(backend.callsTo("set_api_key")).toEqual([
    { providerId: "anthropic", key: "sk-ant-test" },
  ]);
  expect(await screen.findByText("Key set")).toBeTruthy();
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
});

test("a failed key save shows the error and keeps the draft", async () => {
  const backend = installBackend();
  backend.onCommand("set_api_key", () => {
    throw "Couldn't save your API keys: the disk is full";
  });
  const user = userEvent.setup();
  await renderMenu();

  await user.type(screen.getByLabelText("Anthropic API key"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(await screen.findByText(/the disk is full/)).toBeTruthy();
  const input = screen.getByLabelText("Anthropic API key") as HTMLInputElement;
  expect(input.value).toBe("sk-ant-test");
});

test("Remove sends remove_api_key and restores the paste row", async () => {
  const backend = installBackend({
    list_key_status: () => ANTHROPIC_KEYED,
    remove_api_key: () => KEY_STATUS,
  });
  const user = userEvent.setup();
  await renderMenu({
    settle: () =>
      screen.findByRole("button", { name: "Remove the Anthropic API key" }),
  });

  await user.click(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  );
  expect(backend.callsTo("remove_api_key")).toEqual([
    { providerId: "anthropic" },
  ]);
  expect(await screen.findByLabelText("Anthropic API key")).toBeTruthy();
  expect(screen.queryByText("Key set")).toBeNull();
});

test("saving a key fires onKeysChanged once; the open fetch fires none", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  let changed = 0;
  await renderMenu({ onKeysChanged: () => changed++ });
  expect(changed).toBe(0);

  await user.type(screen.getByLabelText("Anthropic API key"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );
  expect(changed).toBe(1);
  expect(backend.callsTo("set_api_key")).toHaveLength(1);
});

test("removing a key fires onKeysChanged once; a failure fires none", async () => {
  const backend = installBackend({
    list_key_status: () => ANTHROPIC_KEYED,
    remove_api_key: () => KEY_STATUS,
  });
  const user = userEvent.setup();
  let changed = 0;
  await renderMenu({
    onKeysChanged: () => changed++,
    settle: () =>
      screen.findByRole("button", { name: "Remove the Anthropic API key" }),
  });

  await user.click(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  );
  expect(changed).toBe(1);

  // A rejected save keeps the providers list untouched.
  backend.onCommand("set_api_key", () => {
    throw "Couldn't save your API keys: nope";
  });
  await user.type(
    await screen.findByLabelText("Anthropic API key"),
    "sk-retry",
  );
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );
  expect(await screen.findByText(/nope/)).toBeTruthy();
  expect(changed).toBe(1);
});

// ---- Model source ---------------------------------------------------------

test("switching the model source calls set_mode and reports the fresh settings", async () => {
  const configured: SettingsInfo = {
    ...SETTINGS,
    defaultMode: { configured: true, vision: false },
  };
  const backend = installBackend();
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  await renderMenu({
    settings: configured,
    onSaved: (next) => saved.push(next),
  });

  await user.click(screen.getByRole("button", { name: "Default" }));
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "default" }]);
  expect(saved).toHaveLength(1);
});

test("the stored mode marks its row selected", async () => {
  const onDefault: SettingsInfo = {
    ...SETTINGS,
    mode: "default",
    defaultMode: { configured: true, vision: false },
  };
  installBackend();
  await renderMenu({ settings: onDefault });

  expect(
    screen
      .getByRole("button", { name: "Default" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen
      .getByRole("button", { name: "Custom API" })
      .getAttribute("aria-current"),
  ).toBeNull();
});

test("the Default row is disabled with a note when unconfigured", async () => {
  installBackend();
  await renderMenu();

  expect(
    screen.getByRole("button", { name: "Default" }).hasAttribute("disabled"),
  ).toBe(true);
  expect(screen.getByText("Default isn't set up in this install.")).toBeTruthy();
  // Never-chosen displays as Custom.
  expect(
    screen
      .getByRole("button", { name: "Custom API" })
      .getAttribute("aria-current"),
  ).toBe("true");
});

// ---- Shortcuts (the recorder) ---------------------------------------------

test("recording a valid combo suspends, saves, and resumes", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  await renderMenu({ onSaved: (next) => saved.push(next) });

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
  await renderMenu();

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
  await renderMenu();

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
  await renderMenu();

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
  await renderMenu({ onClose: () => closed++ });

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
  const { unmount } = await renderMenu();

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
  await renderMenu();

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
  await renderMenu({
    settings: CUSTOM_SUMMON,
    onSaved: (next) => saved.push(next),
  });

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

test("a default shortcut offers no Reset", async () => {
  installBackend();
  await renderMenu();
  expect(
    screen.queryByRole("button", { name: /Reset the Summon shortcut/ }),
  ).toBeNull();
});
