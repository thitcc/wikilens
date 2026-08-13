// The Settings panel's contract. Answers half: two MODE rows — the built-in
// model (when this install has one) and your own provider — and exactly one
// wears the check. The provider lines under the caret are never choices: they
// only set and clear keys, because which provider answers is picked from the
// footer chip. Two rules follow and both are pinned below: a key never moves
// the check (vault/2026-07-29_keys-are-not-a-mode-choice.md), and the
// disclosure follows the mode in both directions. A keyed line is static —
// a "Set" pill and the trash, no button — so the only re-key path is trash,
// then the unkeyed line (vault/2026-08-02_keyed-lines-are-static.md). A
// pasted key crosses IPC exactly once via set_api_key and is never displayed
// back. Recorder half:
// arming suspends the OS hotkeys and swallows keys at capture phase; every
// exit path (save, refuse, Esc, hide, unmount) resumes them exactly once.
// Keyboard goes through userEvent only (see harness.tsx — a raw window
// KeyboardEvent would invert the capture/bubble ordering these tests exist to
// pin).

import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SettingsMenu } from "./SettingsMenu";
import {
  APP_VERSION,
  KEY_STATUS,
  SETTINGS,
  deferred,
  installBackend,
} from "../test/backend";
import { fireBackendEvent } from "../test/harness";
import { THEME_STORAGE_KEY, type ThemeId } from "../theme";
import type { KeyStatus, SettingsInfo } from "../types";

/** Mounts the menu and settles the open-time list_key_status fetch.
 *
 * The default settle target is the unkeyed Anthropic key line, and it resolves
 * only because the shared SETTINGS fixture has never chosen a mode — that
 * normalizes to Custom, which mounts the key lines disclosed. A Default-mode
 * fixture mounts them collapsed and needs `settle: settleNoKeys`. That coupling
 * is silent, so it is spelled out here: it is what broke three tests when the
 * caret landed. The version fetch is a second open-time promise, settled by
 * awaiting its row so it can't land as a stray act() warning mid-test;
 * `noVersion` opts out for the failed-lookup test, whose row never appears. */
async function renderMenu(over?: {
  settings?: SettingsInfo;
  theme?: ThemeId;
  onThemeChange?: (next: ThemeId) => void;
  onSaved?: (next: SettingsInfo) => void;
  onKeysChanged?: () => void;
  onClose?: () => void;
  settle?: () => Promise<unknown>;
  noVersion?: true;
}) {
  const triggerRef = { current: null };
  const onSaved = over?.onSaved ?? (() => {});
  const onClose = over?.onClose ?? (() => {});
  const result = render(
    <SettingsMenu
      settings={over?.settings ?? SETTINGS}
      theme={over?.theme ?? "default"}
      onThemeChange={over?.onThemeChange ?? (() => {})}
      onSaved={onSaved}
      onKeysChanged={over?.onKeysChanged}
      onClose={onClose}
      triggerRef={triggerRef}
    />,
  );
  await (over?.settle?.() ??
    screen.findByRole("button", { name: "Add a key for Anthropic" }));
  if (!over?.noVersion) await screen.findByText(`v${APP_VERSION}`);
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

/** Settle a render whose fixtures leave Anthropic keyed. The trash is the
 * only control a keyed line has, so it is the settle target — the line
 * itself is static. */
const settleKeyedAnthropic = () =>
  screen.findByRole("button", { name: "Remove the Anthropic API key" });

/** Settle a Default-mode render, where the key lines mount collapsed. The note
 * is the fetch's own consequence (it is gated on `statuses` landing); awaiting
 * the caret instead would resolve synchronously and leave setStatuses outside
 * act(). */
const settleNoKeys = () => screen.findByText("Needs a key");

/** Disclose the key lines (Default mode mounts them shut). */
async function openKeys(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Show provider keys" }));
}

/** Open a provider's key field (a key line expands in place). */
async function expandKeyForm(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
) {
  await user.click(screen.getByRole("button", { name: `Add a key for ${name}` }));
  return screen.getByLabelText(`${name} API key`) as HTMLInputElement;
}

/** SETTINGS with Default mode chosen and configured behind it. */
const ON_DEFAULT: SettingsInfo = {
  ...SETTINGS,
  mode: "default",
  defaultMode: { configured: true, vision: false },
};

test("renders the three sections in order with both shortcut chips", async () => {
  installBackend();
  await renderMenu();

  const dialog = screen.getByRole("dialog", { name: "Settings" });
  const text = dialog.textContent ?? "";
  const order = [
    text.indexOf("Answers"),
    text.indexOf("Theme"),
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
  // "Key added" went with the one-list IA. An UNKEYED line has no note or
  // mark at all (this fixture is all-unkeyed — no "Set" pill either); the
  // keyed pill is pinned in the keyed tests below.
  expect(text).not.toContain("Key added");
  expect(screen.queryByText("Set")).toBeNull();
});

// ---- The theme rows -------------------------------------------------------
// The panel is a controlled view of App's theme state: rows report the pick
// via onThemeChange and NOTHING else moves — no IPC (a theme is chrome, not
// behavior config) and no storage write (that ownership stays in App, pinned
// below so a future refactor can't split the write across both).

test("the active theme wears the check, following the prop", async () => {
  installBackend();
  await renderMenu();

  expect(
    screen
      .getByRole("button", { name: "Use the default theme" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen
      .getByRole("button", { name: "Use the Micrographics theme" })
      .getAttribute("aria-current"),
  ).toBeNull();
});

test("a micrographics render moves the check to its row", async () => {
  installBackend();
  await renderMenu({ theme: "micrographics" });

  expect(
    screen
      .getByRole("button", { name: "Use the Micrographics theme" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen
      .getByRole("button", { name: "Use the default theme" })
      .getAttribute("aria-current"),
  ).toBeNull();
});

test("picking a theme reports the pick — zero IPC, storage untouched", async () => {
  const backend = installBackend();
  const picks: ThemeId[] = [];
  const user = userEvent.setup();
  await renderMenu({ onThemeChange: (next) => picks.push(next) });

  const before = backend.calls.length;
  await user.click(
    screen.getByRole("button", { name: "Use the Micrographics theme" }),
  );

  expect(picks).toEqual(["micrographics"]);
  expect(backend.calls.length).toBe(before);
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBeNull();
});

// ---- The answer modes and their key lines ---------------------------------

test("an unkeyed key line expands into a masked field with a gated Save", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  // At rest the list is lines only — no key field is mounted anywhere.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  // A key line carries its name and nothing else; the only "needs a key" in
  // the panel sits on the mode row it actually blocks.
  expect(
    screen.getByRole("button", { name: "Add a key for Anthropic" }).textContent,
  ).toBe("Anthropic");
  expect(
    screen.getByRole("button", { name: "Answer with your own provider" })
      .textContent,
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

test("a keyed line is static — Set pill, trash, no replace button, never the key", async () => {
  installBackend({ list_key_status: () => ANTHROPIC_KEYED });
  await renderMenu({ settle: settleKeyedAnthropic });

  // The key itself never renders — no input for a keyed provider.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  ).toBeTruthy();
  // The line is not a control: no replace affordance, and the name sits in
  // no button at all. The Set pill is the mark that the key is stored.
  expect(
    screen.queryByRole("button", { name: "Replace the Anthropic API key" }),
  ).toBeNull();
  expect(screen.getByText("Anthropic").closest("button")).toBeNull();
  expect(screen.getByText("Set")).toBeTruthy();
  // The keyless provider still needs a click before any field exists — and
  // wears no pill.
  expect(screen.queryByLabelText("DeepSeek API key")).toBeNull();
  const deepseek = screen.getByRole("button", { name: "Add a key for DeepSeek" });
  expect(deepseek.textContent).toBe("DeepSeek");
});

test("every keyed line is storage, not a choice; the mode row keeps the only check", async () => {
  installBackend({ list_key_status: () => BOTH_KEYED });
  await renderMenu({ settle: settleKeyedAnthropic });

  // Both providers are keyed. Under the old one-list IA one of them would have
  // been "the source" and the other would have read "Key added"; now neither
  // is pickable at all — both lines are static pill-wearing storage.
  for (const name of ["Anthropic", "DeepSeek"]) {
    expect(
      screen.queryByRole("button", { name: `Replace the ${name} API key` }),
    ).toBeNull();
    expect(screen.getByText(name).closest("button")).toBeNull();
  }
  expect(screen.getAllByText("Set")).toHaveLength(2);
  expect(screen.queryAllByText("Key added")).toHaveLength(0);

  const custom = screen.getByRole("button", {
    name: "Answer with your own provider",
  });
  expect(custom.getAttribute("aria-current")).toBe("true");
  expect(custom.textContent).not.toContain("Needs a key");
});

test("a keyed line offers no click-to-replace; the only re-key path is trash, then the unkeyed line", async () => {
  const backend = installBackend({
    list_key_status: () => ANTHROPIC_KEYED,
    remove_api_key: () => KEY_STATUS,
  });
  const user = userEvent.setup();
  await renderMenu({ settle: settleKeyedAnthropic });

  // Clicking the static line does nothing: no field, no IPC.
  const before = backend.calls.length;
  await user.click(screen.getByText("Anthropic"));
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect(backend.calls.length).toBe(before);

  // The whole re-key loop: trash → the line comes back unkeyed → click →
  // an empty field (the old key was write-only and is gone).
  await user.click(
    screen.getByRole("button", { name: "Remove the Anthropic API key" }),
  );
  await screen.findByRole("button", { name: "Add a key for Anthropic" });
  const input = await expandKeyForm(user, "Anthropic");
  expect(input.value).toBe("");
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
  // The flip's other half: the Set pill lands, and the click-to-open button
  // is gone — the line is storage now, the trash its only control.
  expect(screen.getByText("Set")).toBeTruthy();
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
});

test("saving a key stores it and leaves the mode where it was", async () => {
  const backend = installBackend({ set_api_key: () => ANTHROPIC_KEYED });
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  // Run it from Default, where "stays put" is actually visible.
  await renderMenu({
    settings: ON_DEFAULT,
    onSaved: (next) => saved.push(next),
    settle: settleNoKeys,
  });
  await openKeys(user);

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(backend.callsTo("set_api_key")).toEqual([
    { providerId: "anthropic", key: "sk-ant-test" },
  ]);
  // The whole point of the split: a key is storage, not a choice. It commits
  // no mode, reports no settings, and leaves the check on the built-in model.
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  expect(saved).toHaveLength(0);
  expect(
    screen
      .getByRole("button", { name: "Answer with the built-in model" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    await screen.findByRole("button", {
      name: "Remove the Anthropic API key",
    }),
  ).toBeTruthy();
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

test("Remove sends remove_api_key and the line goes back to Add a key", async () => {
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
  const addButton = await screen.findByRole("button", {
    name: "Add a key for Anthropic",
  });
  expect(
    screen.queryByRole("button", { name: "Remove the Anthropic API key" }),
  ).toBeNull();
  // The pill goes with the key.
  expect(screen.queryByText("Set")).toBeNull();
  // Removal never opens a field on its own.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  // The trash that had focus unmounted — focus lands on the now-unkeyed
  // line, not the dialog, so re-keying is Enter away.
  expect(document.activeElement).toBe(addButton);
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
  installBackend();
  await renderMenu({ settings: ON_DEFAULT, settle: settleNoKeys });

  expect(
    screen
      .getByRole("button", { name: "Answer with the built-in model" })
      .getAttribute("aria-current"),
  ).toBe("true");
  // Default mode mounts the key lines collapsed, and nothing behind that caret
  // is a choice — so nothing there can contradict the check.
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    screen
      .getByRole("button", { name: "Answer with your own provider" })
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
    screen.getByText("Nothing can answer yet — add a key below."),
  ).toBeTruthy();
});

test("a stale default mode keeps the built-in row, selected and marked not set up", async () => {
  const stale: SettingsInfo = {
    ...SETTINGS,
    mode: "default",
    defaultMode: { configured: false, vision: false },
  };
  installBackend();
  // No settle override: nothing is configured behind the chosen Default, so
  // the seed opens the key lines — which is what keeps "add a key below"
  // pointing at something.
  await renderMenu({ settings: stale });

  const builtIn = screen.getByRole("button", {
    name: "Answer with the built-in model",
  });
  expect(builtIn.getAttribute("aria-current")).toBe("true");
  expect(builtIn.textContent).toContain("Not set up here");
  expect(
    screen.getByText("Nothing can answer yet — add a key below."),
  ).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
});

// ---- The disclosure --------------------------------------------------------

test("the caret shows and hides the key lines without touching the mode", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  const caret = screen.getByRole("button", { name: "Hide provider keys" });
  expect(caret.getAttribute("aria-expanded")).toBe("true");
  // The disclosed region names itself, so the relationship survives for AT —
  // the caret floats in a rail over a different row than the one it opens.
  expect(caret.getAttribute("aria-controls")).toBe(
    screen.getByRole("button", { name: "Add a key for Anthropic" }).closest(".keys-nest")?.id,
  );

  await user.click(caret);
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    screen
      .getByRole("button", { name: "Show provider keys" })
      .getAttribute("aria-expanded"),
  ).toBe("false");
  // Looking at your keys is not picking one: the caret commits nothing and
  // leaves the check where it was.
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  expect(
    screen
      .getByRole("button", { name: "Answer with your own provider" })
      .getAttribute("aria-current"),
  ).toBe("true");

  await user.click(screen.getByRole("button", { name: "Show provider keys" }));
  expect(
    screen.getByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
});

test("collapsing the caret drops an open key draft", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  // Same key-residency rule the per-row collapse honors, one level out: a
  // typed key must not sit alive behind a closed disclosure.
  await user.click(screen.getByRole("button", { name: "Hide provider keys" }));
  await openKeys(user);

  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect((await expandKeyForm(user, "Anthropic")).value).toBe("");
});

test("picking your own provider reveals the key lines", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu({ settings: ON_DEFAULT, settle: settleNoKeys });

  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();

  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  // Landing on Custom with nothing keyed used to strand the player: the mode
  // committed, the note said "add a key below", and the list stayed shut.
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "Hide provider keys" }),
  ).toBeTruthy();
});

test("picking the built-in model puts the key lines away", async () => {
  const configured: SettingsInfo = {
    ...SETTINGS,
    mode: "custom",
    defaultMode: { configured: true, vision: false },
  };
  installBackend();
  const user = userEvent.setup();
  await renderMenu({ settings: configured });

  await user.click(
    screen.getByRole("button", { name: "Answer with the built-in model" }),
  );

  // The mirror half, and the one that rots silently: a list left open under a
  // row that isn't answering implies a disclosure relationship that is a lie.
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    screen.getByRole("button", { name: "Show provider keys" }),
  ).toBeTruthy();
});

test("a failed mode flip says so and leaves the disclosure alone", async () => {
  const backend = installBackend();
  backend.onCommand("set_mode", () => {
    throw "Couldn't save your choice: the disk is full";
  });
  const user = userEvent.setup();
  await renderMenu({ settings: ON_DEFAULT, settle: settleNoKeys });

  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  expect(await screen.findByText(/the disk is full/)).toBeTruthy();
  // The reconcile sits after the await inside the try, so a flip that never
  // happened moves nothing — neither the list nor the check.
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    screen
      .getByRole("button", { name: "Answer with the built-in model" })
      .getAttribute("aria-current"),
  ).toBe("true");
});

// ---- What the panel says before it knows -----------------------------------

test("neither key note speaks while the status fetch is still open", async () => {
  const gate = deferred<KeyStatus[]>();
  const backend = installBackend();
  backend.onCommand("list_key_status", () => gate.promise);
  await renderMenu({ settle: () => screen.findByText("Loading…") });

  // Derived from a bare some(), these read false while statuses is null — so
  // they flashed on every open and pinned forever when the fetch failed.
  expect(screen.queryByText(/Nothing can answer yet/)).toBeNull();
  expect(
    screen.getByRole("button", { name: "Answer with your own provider" })
      .textContent,
  ).not.toContain("Needs a key");

  await act(async () => {
    gate.resolve(KEY_STATUS);
  });
  expect(
    await screen.findByText("Nothing can answer yet — add a key below."),
  ).toBeTruthy();
});

test("a failed key-status read says so instead of reading as no keys", async () => {
  installBackend({
    list_key_status: () => {
      throw "Couldn't read your API keys: the store is unreadable";
    },
  });
  await renderMenu({ settle: () => screen.findByText(/store is unreadable/) });

  // The failure is panel-scope: tagged to a row it would have gone unrendered
  // on a fresh install, and behind the caret it would have gone unseen.
  expect(screen.queryByText(/Nothing can answer yet/)).toBeNull();
});

test("an empty provider list says so instead of showing an empty box", async () => {
  installBackend({ list_key_status: () => [] });
  await renderMenu({
    settle: () => screen.findByText("No providers to key on this install."),
  });

  expect(
    screen.queryByRole("button", { name: /^Add a key for/ }),
  ).toBeNull();
});

test("the dialog takes focus on mount", async () => {
  installBackend();
  await renderMenu();

  expect(document.activeElement).toBe(
    screen.getByRole("dialog", { name: "Settings" }),
  );
});

// ---- The version row -------------------------------------------------------

test("shows the running version at the bottom", async () => {
  const backend = installBackend();
  await renderMenu();

  // The fixture is deliberately not the manifest number, so a hardcoded
  // "v0.1.0" in the JSX could never pass this.
  expect(screen.getByText(`v${APP_VERSION}`)).toBeTruthy();
  expect(backend.callsTo("plugin:app|version")).toHaveLength(1);
});

test("a failed version lookup renders no row", async () => {
  installBackend({
    "plugin:app|version": () => {
      throw new Error("no version for you");
    },
  });
  await renderMenu({ noVersion: true });

  expect(screen.queryByText(`v${APP_VERSION}`)).toBeNull();
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
