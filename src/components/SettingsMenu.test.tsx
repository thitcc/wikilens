// The Settings panel's contract. Source half: one exclusive "Answers come
// from" list — the built-in model (when this install has one) plus every
// provider; a keyed row commits on click, an unkeyed row opens in place into
// a single key field (one at a time) and saving the key completes the choice.
// A pasted key crosses IPC exactly once via set_api_key and is never displayed
// back — a stored key renders as presence + Remove only. Recorder half: arming
// suspends the OS hotkeys and swallows keys at capture phase; every exit path
// (save, refuse, Esc, hide, unmount) resumes them exactly once. Keyboard goes
// through userEvent only (see harness.tsx — a raw window KeyboardEvent would
// invert the capture/bubble ordering these tests exist to pin).

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SettingsMenu } from "./SettingsMenu";
import { KEY_STATUS, SETTINGS, installBackend } from "../test/backend";
import { fireBackendEvent } from "../test/harness";
import type { KeyStatus, SettingsInfo } from "../types";

/** Mounts the menu and settles the open-time list_key_status fetch (the
 * default settle target is the keyless Anthropic row — at rest there is no
 * key field anywhere; tests that override the key fixtures pass their own). */
async function renderMenu(over?: {
  settings?: SettingsInfo;
  selectedProviderId?: string;
  onSaved?: (next: SettingsInfo) => void;
  onKeysChanged?: () => void;
  onPickProvider?: (id: string) => void;
  onClose?: () => void;
  settle?: () => Promise<unknown>;
}) {
  const triggerRef = { current: null };
  const onSaved = over?.onSaved ?? (() => {});
  const onClose = over?.onClose ?? (() => {});
  const result = render(
    <SettingsMenu
      settings={over?.settings ?? SETTINGS}
      selectedProviderId={over?.selectedProviderId ?? ""}
      onSaved={onSaved}
      onKeysChanged={over?.onKeysChanged}
      onPickProvider={over?.onPickProvider}
      onClose={onClose}
      triggerRef={triggerRef}
    />,
  );
  await (over?.settle?.() ??
    screen.findByRole("button", { name: "Add a key for Anthropic" }));
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

/** Every provider keyed — the only state where a keyed row is NOT the source. */
const BOTH_KEYED: KeyStatus[] = KEY_STATUS.map((s) => ({ ...s, hasKey: true }));

/** Settle a render whose fixtures leave Anthropic keyed. */
const settleKeyedAnthropic = () =>
  screen.findByRole("button", { name: "Answer with Anthropic" });

/** Open a provider's key field (an unkeyed row expands in place). */
async function expandKeyForm(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
) {
  await user.click(screen.getByRole("button", { name: `Add a key for ${name}` }));
  return screen.getByLabelText(`${name} API key`) as HTMLInputElement;
}

test("renders the two sections in order with both shortcut chips", async () => {
  installBackend();
  await renderMenu();

  const dialog = screen.getByRole("dialog", { name: "Settings" });
  const text = dialog.textContent ?? "";
  const order = [
    text.indexOf("Answers come from"),
    text.indexOf("Shortcuts"),
  ];
  expect(Math.min(...order)).toBeGreaterThanOrEqual(0);
  expect([...order].sort((a, b) => a - b)).toEqual(order);
  expect(text).toContain("Summon");
  expect(text).toContain("Ctrl+`");
  expect(text).toContain("Capture");
  expect(text).toContain("Ctrl+Shift+C");
  // The merged sections took their old vocabulary with them.
  expect(text).not.toContain("Model source");
  expect(text).not.toContain("Custom API");
  expect(text).not.toContain("API keys");
  expect(text).not.toContain("Key set");
});

// ---- The answer-source list ----------------------------------------------

test("an unkeyed provider row expands into a masked field with a gated Save", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  // At rest the list is rows only — no key field is mounted anywhere.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect(
    screen.getByRole("button", { name: "Add a key for Anthropic" }).textContent,
  ).toContain("Needs a key");

  const input = await expandKeyForm(user, "Anthropic");
  expect(input.type).toBe("password");
  expect(input.getAttribute("autocomplete")).toBe("off");

  // Save arms only once there is text.
  const save = screen.getByRole("button", { name: "Save the Anthropic API key" });
  expect(save.hasAttribute("disabled")).toBe(true);
  await user.type(input, "sk-ant-test");
  expect(save.hasAttribute("disabled")).toBe(false);
});

test("only one key form is open at a time", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await expandKeyForm(user, "Anthropic");
  await expandKeyForm(user, "DeepSeek");

  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect(screen.getByLabelText("DeepSeek API key")).toBeTruthy();
});

test("clicking an unkeyed row opens its field and fires no IPC", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  const before = backend.calls.length;
  await expandKeyForm(user, "Anthropic");

  expect(backend.calls.length).toBe(before);
  expect(backend.callsTo("set_mode")).toHaveLength(0);
});

test("collapsing a key form drops the draft", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  // Clicking the row again collapses it — the key text goes with it.
  await user.click(
    screen.getByRole("button", { name: "Add a key for Anthropic" }),
  );
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();

  const reopened = await expandKeyForm(user, "Anthropic");
  expect(reopened.value).toBe("");
});

test("a stored key renders as a source row with Remove and no field", async () => {
  installBackend({ list_key_status: () => ANTHROPIC_KEYED });
  await renderMenu({ settle: settleKeyedAnthropic });

  // The key itself never renders — no input for a keyed provider.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  ).toBeTruthy();
  // The only keyed provider is the live source, so the check speaks alone.
  expect(
    screen
      .getByRole("button", { name: "Answer with Anthropic" })
      .getAttribute("aria-current"),
  ).toBe("true");
  // The keyless provider still needs a click before any field exists.
  expect(screen.queryByLabelText("DeepSeek API key")).toBeNull();
  expect(
    screen.getByRole("button", { name: "Add a key for DeepSeek" }),
  ).toBeTruthy();
});

test("a keyed provider that isn't the source reads Key added", async () => {
  installBackend({ list_key_status: () => BOTH_KEYED });
  await renderMenu({
    selectedProviderId: "anthropic",
    settle: settleKeyedAnthropic,
  });

  const deepseek = screen.getByRole("button", { name: "Answer with DeepSeek" });
  expect(deepseek.textContent).toContain("Key added");
  expect(deepseek.getAttribute("aria-current")).toBeNull();
  // The live source carries no note — the accent check is the whole signal.
  expect(
    screen.getByRole("button", { name: "Answer with Anthropic" }).textContent,
  ).not.toContain("Key added");
});

test("picking a keyed provider commits the mode and the pick", async () => {
  const backend = installBackend({ list_key_status: () => BOTH_KEYED });
  const user = userEvent.setup();
  const picks: string[] = [];
  await renderMenu({
    selectedProviderId: "anthropic",
    onPickProvider: (id) => picks.push(id),
    settle: settleKeyedAnthropic,
  });

  await user.click(screen.getByRole("button", { name: "Answer with DeepSeek" }));

  // The fixture has never chosen a mode, so the pick commits Custom too.
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  expect(picks).toEqual(["deepseek"]);
});

test("pasting a key sends it once and flips the row to keyed", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  await renderMenu();

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(backend.callsTo("set_api_key")).toEqual([
    { providerId: "anthropic", key: "sk-ant-test" },
  ]);
  expect(await settleKeyedAnthropic()).toBeTruthy();
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
});

test("saving a key also commits the answer source", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  const picks: string[] = [];
  await renderMenu({ onPickProvider: (id) => picks.push(id) });

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(picks).toEqual(["anthropic"]);
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  expect((await settleKeyedAnthropic()).getAttribute("aria-current")).toBe(
    "true",
  );
});

test("a failed key save shows the error under the row and keeps the draft", async () => {
  const backend = installBackend();
  backend.onCommand("set_api_key", () => {
    throw "Couldn't save your API keys: the disk is full";
  });
  const user = userEvent.setup();
  await renderMenu();

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(await screen.findByText(/the disk is full/)).toBeTruthy();
  const input = screen.getByLabelText("Anthropic API key") as HTMLInputElement;
  expect(input.value).toBe("sk-ant-test");
});

test("the vendor link opens the provider's console externally", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await expandKeyForm(user, "Anthropic");
  await user.click(
    screen.getByRole("button", { name: "console.anthropic.com" }),
  );

  const opens = backend.callsTo("plugin:opener|open_url") as Array<{
    url: string;
  }>;
  expect(opens.map((o) => o.url)).toEqual([
    "https://console.anthropic.com/settings/keys",
  ]);
});

test("Remove sends remove_api_key and the row goes back to needing a key", async () => {
  const backend = installBackend({
    list_key_status: () => ANTHROPIC_KEYED,
    remove_api_key: () => KEY_STATUS,
  });
  const user = userEvent.setup();
  await renderMenu({ settle: settleKeyedAnthropic });

  await user.click(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  );
  expect(backend.callsTo("remove_api_key")).toEqual([
    { providerId: "anthropic" },
  ]);
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
  expect(
    screen.queryByRole("button", { name: "Remove the Anthropic API key" }),
  ).toBeNull();
  // Removal never opens a field on its own.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
});

test("saving a key fires onKeysChanged once; the open fetch fires none", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  let changed = 0;
  await renderMenu({ onKeysChanged: () => changed++ });
  expect(changed).toBe(0);

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
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
    settle: settleKeyedAnthropic,
  });

  await user.click(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  );
  expect(changed).toBe(1);

  // A rejected save keeps the providers list untouched.
  backend.onCommand("set_api_key", () => {
    throw "Couldn't save your API keys: nope";
  });
  await screen.findByRole("button", { name: "Add a key for Anthropic" });
  await user.type(await expandKeyForm(user, "Anthropic"), "sk-retry");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );
  expect(await screen.findByText(/nope/)).toBeTruthy();
  expect(changed).toBe(1);
});

test("picking the built-in row calls set_mode and reports the fresh settings", async () => {
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

  await user.click(
    screen.getByRole("button", { name: "Answer with the built-in model" }),
  );
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "default" }]);
  expect(saved).toHaveLength(1);
});

test("the stored default mode marks the built-in row as the source", async () => {
  const onDefault: SettingsInfo = {
    ...SETTINGS,
    mode: "default",
    defaultMode: { configured: true, vision: false },
  };
  installBackend();
  await renderMenu({ settings: onDefault });

  expect(
    screen
      .getByRole("button", { name: "Answer with the built-in model" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen
      .getByRole("button", { name: "Add a key for Anthropic" })
      .getAttribute("aria-current"),
  ).toBeNull();
});

test("an install with no built-in and no keys lists no built-in row and says nothing can answer", async () => {
  installBackend();
  await renderMenu();

  expect(
    screen.queryByRole("button", { name: "Answer with the built-in model" }),
  ).toBeNull();
  expect(
    screen.getByText("Nothing can answer yet — pick one below and add its key."),
  ).toBeTruthy();
});

test("a stale default mode keeps the built-in row, selected and marked not set up", async () => {
  const stale: SettingsInfo = {
    ...SETTINGS,
    mode: "default",
    defaultMode: { configured: false, vision: false },
  };
  installBackend();
  await renderMenu({ settings: stale });

  const builtIn = screen.getByRole("button", {
    name: "Answer with the built-in model",
  });
  expect(builtIn.getAttribute("aria-current")).toBe("true");
  expect(builtIn.textContent).toContain("Not set up here");
});

test("the dialog takes focus on mount", async () => {
  installBackend();
  await renderMenu();

  expect(document.activeElement).toBe(
    screen.getByRole("dialog", { name: "Settings" }),
  );
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
