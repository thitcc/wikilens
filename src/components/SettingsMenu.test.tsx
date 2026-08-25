// The Settings panel's contract. Answers half: one game-style STEPPER picks
// the mode — ◁ Value ▷, arrows cycling with wrap (stepper.ts), the center
// value opening a floating option popover, the scoped third altitude
// (vault/2026-08-13_stepper-popover-third-altitude.md). The provider lines
// under it are never choices: they only set and clear keys, because which
// provider answers is picked from the footer chip. Two rules follow and both
// are pinned below: a key never moves the value
// (vault/2026-07-29_keys-are-not-a-mode-choice.md), and the key lines exist
// exactly while Custom API answers — visibility derives from the stored mode,
// so it moves only when the fresh settings come back through onSaved. A keyed
// line is static — a "Set" pill and the trash, no button — so the only re-key
// path is trash, then the unkeyed line
// (vault/2026-08-02_keyed-lines-are-static.md). A pasted key crosses IPC
// exactly once via set_api_key and is never displayed back. Recorder half:
// arming suspends the OS hotkeys and swallows keys at capture phase; every
// exit path (save, refuse, Esc, hide, unmount) resumes them exactly once.
// Esc is four layers exactly: recording, popover, menu, overlay.
// Keyboard goes through userEvent only (see harness.tsx — a raw window
// KeyboardEvent would invert the capture/bubble ordering these tests exist to
// pin).

import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { SettingsMenu } from "./SettingsMenu";
import {
  APP_VERSION,
  KEY_STATUS,
  SETTINGS,
  SETTINGS_LOCAL,
  SETTINGS_LOCAL_KEYED,
  SETTINGS_POSITION_LOCKED,
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
 * normalizes to Custom, which renders the key lines. A Default-mode fixture
 * renders no key lines at all and needs `settle: settleStatuses` (or a text
 * the landed statuses produce). That coupling is silent, so it is spelled out
 * here: it is what broke three tests when the disclosure landed. The version
 * fetch is a second open-time promise, settled by awaiting its row so it
 * can't land as a stray act() warning mid-test; `noVersion` opts out for the
 * failed-lookup test, whose row never appears.
 *
 * `rerenderWith` re-renders with fresh settings, playing App's onSaved →
 * setSettings loop — the panel is a controlled view, so a mode flip only
 * moves the key lines once the fresh SettingsInfo comes back through it. */
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
  const props = (settings: SettingsInfo) => (
    <SettingsMenu
      settings={settings}
      theme={over?.theme ?? "default"}
      onThemeChange={over?.onThemeChange ?? (() => {})}
      onSaved={over?.onSaved ?? (() => {})}
      onKeysChanged={over?.onKeysChanged}
      onClose={over?.onClose ?? (() => {})}
      triggerRef={triggerRef}
    />
  );
  const result = render(props(over?.settings ?? SETTINGS));
  await (over?.settle?.() ??
    screen.findByRole("button", { name: "Add a key for Anthropic" }));
  if (!over?.noVersion) await screen.findByText(`v${APP_VERSION}`);
  return {
    ...result,
    rerenderWith: (settings: SettingsInfo) => result.rerender(props(settings)),
  };
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

/** SETTINGS with Custom API stored as the mode (the never-chosen fixture
 * already behaves as Custom; this one pins the explicit choice). */
const ON_CUSTOM: SettingsInfo = { ...SETTINGS, mode: "custom" };

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

/** Settle a Local-mode render: the registry key lines aren't rendered there
 * and nothing else visibly changes when the statuses land, so flush the fetch
 * inside act() instead of awaiting an artifact. */
const settleStatuses = () => act(async () => {});

/** Open a provider's key field (a key line expands in place). */
async function expandKeyForm(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
) {
  await user.click(screen.getByRole("button", { name: `Add a key for ${name}` }));
  return screen.getByLabelText(`${name} API key`) as HTMLInputElement;
}

/** The Answers stepper's center value. */
const answersValue = () =>
  screen.getByRole("button", { name: "Choose the answer source" });

/** Open the Answers option popover. */
async function openAnswers(user: ReturnType<typeof userEvent.setup>) {
  await user.click(answersValue());
}

test("renders the four sections in order with both shortcut chips", async () => {
  installBackend();
  await renderMenu();

  const dialog = screen.getByRole("dialog", { name: "Settings" });
  const text = dialog.textContent ?? "";
  const order = [
    text.indexOf("Answers"),
    text.indexOf("Theme"),
    text.indexOf("Position"),
    text.indexOf("Shortcuts"),
  ];
  expect(Math.min(...order)).toBeGreaterThanOrEqual(0);
  expect([...order].sort((a, b) => a - b)).toEqual(order);
  expect(text).toContain("Summon");
  expect(text).toContain("Ctrl+`");
  expect(text).toContain("Capture");
  expect(text).toContain("Ctrl+Shift+C");
  // The stepper's value speaks the new short names; the long row labels are
  // gone from the visible text (they live on as aria-labels).
  expect(text).toContain("Custom API");
  expect(text).not.toContain("Your own provider");
  expect(text).not.toContain("Built into WikiLens");
  // The merged sections took their old vocabulary with them.
  expect(text).not.toContain("Model source");
  expect(text).not.toContain("API keys");
  expect(text).not.toContain("Key set");
  // "Key added" went with the one-list IA. An UNKEYED line has no note or
  // mark at all (this fixture is all-unkeyed — no "Set" pill either); the
  // keyed pill is pinned in the keyed tests below.
  expect(text).not.toContain("Key added");
  expect(screen.queryByText("Set")).toBeNull();
});

// ---- The theme stepper ------------------------------------------------------
// The panel is a controlled view of App's theme state: the stepper reports the
// pick via onThemeChange and NOTHING else moves — no IPC (a theme is chrome,
// not behavior config) and no storage write (that ownership stays in App,
// pinned below so a future refactor can't split the write across both).

test("the active theme wears the check in the popover, following the prop", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  expect(
    screen.getByRole("button", { name: "Choose the theme" }).textContent,
  ).toContain("Default");
  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
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

test("a micrographics render moves the value and the check", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu({ theme: "micrographics" });

  expect(
    screen.getByRole("button", { name: "Choose the theme" }).textContent,
  ).toContain("Micrographics");
  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
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

test("picking a theme in the popover reports the pick — zero IPC, storage untouched", async () => {
  const backend = installBackend();
  const picks: ThemeId[] = [];
  const user = userEvent.setup();
  await renderMenu({ onThemeChange: (next) => picks.push(next) });

  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
  const before = backend.calls.length;
  await user.click(
    screen.getByRole("button", { name: "Use the Micrographics theme" }),
  );

  expect(picks).toEqual(["micrographics"]);
  expect(backend.calls.length).toBe(before);
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBeNull();
  // The pick closed the popover behind it.
  expect(
    screen.queryByRole("button", { name: "Use the default theme" }),
  ).toBeNull();
});

test("cycling the theme with an arrow reports the pick with zero IPC", async () => {
  const backend = installBackend();
  const picks: ThemeId[] = [];
  const user = userEvent.setup();
  await renderMenu({ onThemeChange: (next) => picks.push(next) });

  const before = backend.calls.length;
  await user.click(screen.getByRole("button", { name: "Switch to the next theme" }));

  expect(picks).toEqual(["micrographics"]);
  expect(backend.calls.length).toBe(before);
});

// ---- The answer modes and their key lines ---------------------------------

test("an unkeyed key line expands into a masked field with a gated Save", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  // At rest the list is lines only — no key field is mounted anywhere.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  // A key line carries its name and nothing else; the only "needs a key" in
  // the panel sits on the stepper value it actually blocks.
  expect(
    screen.getByRole("button", { name: "Add a key for Anthropic" }).textContent,
  ).toBe("Anthropic");
  expect(answersValue().textContent).toContain("Needs a key");

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

test("every keyed line is storage, not a choice; the mode keeps the only check", async () => {
  installBackend({ list_key_status: () => BOTH_KEYED });
  const user = userEvent.setup();
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

  expect(answersValue().textContent).not.toContain("Needs a key");
  await openAnswers(user);
  expect(
    screen
      .getByRole("button", { name: "Answer with your own provider" })
      .getAttribute("aria-current"),
  ).toBe("true");
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
  // Local AI is one arrow away, so a mode flip WOULD be possible — the key
  // save still commits nothing (a key is storage, not a choice).
  await renderMenu({
    settings: ON_CUSTOM,
    onSaved: (next) => saved.push(next),
  });

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(backend.callsTo("set_api_key")).toEqual([
    { providerId: "anthropic", key: "sk-ant-test" },
  ]);
  // The whole point of the split: a key is storage, not a choice. It commits
  // no mode, reports no settings, and leaves the value where it was.
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  expect(saved).toHaveLength(0);
  expect(answersValue().textContent).toContain("Custom API");
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

test("picking Local AI calls set_mode and reports the fresh settings", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  await renderMenu({
    onSaved: (next) => saved.push(next),
  });

  await openAnswers(user);
  await user.click(
    screen.getByRole("button", { name: "Answer with a local AI server" }),
  );
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "local" }]);
  expect(saved).toHaveLength(1);
  // The pick closed the popover behind it.
  expect(
    screen.queryByRole("button", { name: "Answer with a local AI server" }),
  ).toBeNull();
});

test("the stored local mode shows Local AI and keeps the key lines away", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu({ settings: SETTINGS_LOCAL, settle: settleStatuses });

  expect(answersValue().textContent).toContain("Local AI");
  // The registry key lines exist exactly while Custom API answers — none
  // here, so nothing below the stepper can contradict the value.
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  await openAnswers(user);
  expect(
    screen
      .getByRole("button", { name: "Answer with a local AI server" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen
      .getByRole("button", { name: "Answer with your own provider" })
      .getAttribute("aria-current"),
  ).toBeNull();
});

test("a fresh keyless install has both modes live and the needs-a-key note on Custom", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  // Two options always exist (Local AI is always nominally configured), so
  // the arrows never disable.
  expect(
    screen
      .getByRole("button", { name: "Switch to the previous answer source" })
      .hasAttribute("disabled"),
  ).toBe(false);
  expect(
    screen
      .getByRole("button", { name: "Switch to the next answer source" })
      .hasAttribute("disabled"),
  ).toBe(false);
  // The note is Custom's alone — Local AI has no key precondition.
  expect(answersValue().textContent).toContain("Needs a key");
  await openAnswers(user);
  expect(
    screen
      .getByRole("button", { name: "Answer with your own provider" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(
    screen.getByRole("button", { name: "Answer with a local AI server" }),
  ).toBeTruthy();
});

// ---- The keys follow the mode ----------------------------------------------
// Visibility is derived from the STORED mode: a pick moves the key lines only
// when the fresh SettingsInfo comes back through onSaved and App re-renders
// this controlled panel (rerenderWith plays that loop).

test("picking Custom API reveals the key lines once the fresh settings land", async () => {
  const backend = installBackend({ set_mode: () => ON_CUSTOM });
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  const view = await renderMenu({
    settings: SETTINGS_LOCAL,
    onSaved: (next) => saved.push(next),
    settle: settleStatuses,
  });

  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  await openAnswers(user);
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  view.rerenderWith(saved[0]);
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
});

test("picking Local AI swaps the key lines for the server rows", async () => {
  const backend = installBackend({ set_mode: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  const view = await renderMenu({
    settings: ON_CUSTOM,
    onSaved: (next) => saved.push(next),
  });

  await openAnswers(user);
  await user.click(
    screen.getByRole("button", { name: "Answer with a local AI server" }),
  );

  expect(backend.callsTo("set_mode")).toEqual([{ mode: "local" }]);
  view.rerenderWith(saved[0]);
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  // The Local rows are there instead: the address field carries the
  // effective URL, the optional key line its rail note, and the eye its off
  // state.
  expect(
    (
      screen.getByRole("textbox", {
        name: "Local AI server address",
      }) as HTMLInputElement
    ).value,
  ).toBe("http://localhost:11434/v1");
  expect(screen.getByText("Optional")).toBeTruthy();
  expect(
    screen.getByRole("button", {
      name: "Mark the local model as able to read images",
    }),
  ).toBeTruthy();
});

test("leaving Custom API drops an open key draft", async () => {
  installBackend({ set_mode: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  await renderMenu({ settings: ON_CUSTOM });

  await user.type(await expandKeyForm(user, "Anthropic"), "sk-ant-test");
  await openAnswers(user);
  await user.click(
    screen.getByRole("button", { name: "Answer with a local AI server" }),
  );

  // The key-residency rule, one level out: the typed key must not outlive
  // the list, and it is gone even before the fresh settings land.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
  expect((await expandKeyForm(user, "Anthropic")).value).toBe("");
});

test("a failed mode flip says so and moves nothing", async () => {
  const backend = installBackend();
  backend.onCommand("set_mode", () => {
    throw "Couldn't save your choice: the disk is full";
  });
  const user = userEvent.setup();
  await renderMenu({ settings: SETTINGS_LOCAL, settle: settleStatuses });

  await openAnswers(user);
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  expect(await screen.findByText(/the disk is full/)).toBeTruthy();
  // The flip never happened: the value still reads Local AI and the key
  // lines stay away.
  expect(answersValue().textContent).toContain("Local AI");
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
});

// ---- The Local AI rows -----------------------------------------------------
// Rendered exactly while Local AI answers (the key-lines rule, one mode
// over): the server address (Rust normalizes; empty clears to the baked
// default), the optional key line, and the vision eye on the heading's rail.
// Configuring never moves the mode — the keys ADR extended to an address.

test("saving the server address goes through Rust and reports fresh settings", async () => {
  const saved: SettingsInfo[] = [];
  const savedSettings: SettingsInfo = {
    ...SETTINGS_LOCAL,
    localMode: { ...SETTINGS_LOCAL.localMode, baseUrl: "http://box.lan:8080/v1" },
  };
  const backend = installBackend({ set_local_base_url: () => savedSettings });
  const user = userEvent.setup();
  const view = await renderMenu({
    settings: SETTINGS_LOCAL,
    onSaved: (next) => saved.push(next),
    settle: settleStatuses,
  });

  const field = screen.getByRole("textbox", {
    name: "Local AI server address",
  }) as HTMLInputElement;
  await user.clear(field);
  await user.type(field, "box.lan:8080");
  await user.click(
    screen.getByRole("button", { name: "Save the local AI server address" }),
  );

  // The raw paste crosses; Rust owns normalization (scheme, /v1).
  expect(backend.callsTo("set_local_base_url")).toEqual([
    { baseUrl: "box.lan:8080" },
  ]);
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  expect(saved).toHaveLength(1);
  // The fresh settings re-sync the field to the normalized form.
  view.rerenderWith(saved[0]);
  expect(field.value).toBe("http://box.lan:8080/v1");
});

test("a failed address save shows the error under the row and keeps the draft", async () => {
  const backend = installBackend();
  backend.onCommand("set_local_base_url", () => {
    throw "That doesn't look like a server address — use something like http://localhost:11434.";
  });
  const user = userEvent.setup();
  await renderMenu({ settings: SETTINGS_LOCAL, settle: settleStatuses });

  const field = screen.getByRole("textbox", {
    name: "Local AI server address",
  }) as HTMLInputElement;
  await user.clear(field);
  await user.type(field, "ftp://nope{Enter}");

  expect(
    await screen.findByText(/doesn't look like a server address/),
  ).toBeTruthy();
  expect(field.value).toBe("ftp://nope");
});

test("the eye flips vision through set_local_vision and never the mode", async () => {
  const saved: SettingsInfo[] = [];
  const visionOn: SettingsInfo = {
    ...SETTINGS_LOCAL,
    localMode: { ...SETTINGS_LOCAL.localMode, vision: true },
  };
  const backend = installBackend({ set_local_vision: () => visionOn });
  const user = userEvent.setup();
  const view = await renderMenu({
    settings: SETTINGS_LOCAL,
    onSaved: (next) => saved.push(next),
    settle: settleStatuses,
  });

  await user.click(
    screen.getByRole("button", {
      name: "Mark the local model as able to read images",
    }),
  );
  expect(backend.callsTo("set_local_vision")).toEqual([{ vision: true }]);
  expect(backend.callsTo("set_mode")).toHaveLength(0);

  // The fresh settings flip the glyph and the accessible name.
  view.rerenderWith(saved[0]);
  const on = screen.getByRole("button", {
    name: "Mark the local model as text-only",
  });
  expect(on.getAttribute("aria-pressed")).toBe("true");
});

test("the local key line saves through its own command and never the mode", async () => {
  const saved: SettingsInfo[] = [];
  const backend = installBackend({
    set_local_api_key: () => SETTINGS_LOCAL_KEYED,
  });
  const user = userEvent.setup();
  const view = await renderMenu({
    settings: SETTINGS_LOCAL,
    onSaved: (next) => saved.push(next),
    settle: settleStatuses,
  });

  // Unkeyed wears the rail note; opening the form reveals the local help.
  expect(screen.getByText("Optional")).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Add a key for the local AI server" }),
  );
  expect(screen.getByText(/Only needed if your server requires one/)).toBeTruthy();
  await user.type(
    screen.getByLabelText("Local AI server API key"),
    "local-tok",
  );
  await user.click(
    screen.getByRole("button", { name: "Save the local AI server's API key" }),
  );

  expect(backend.callsTo("set_local_api_key")).toEqual([{ key: "local-tok" }]);
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  // The keyed line is static: the Set pill marks it, the trash is the only
  // control, and the form is gone.
  view.rerenderWith(saved[0]);
  expect(screen.getByText("Set")).toBeTruthy();
  expect(screen.queryByLabelText("Local AI server API key")).toBeNull();
  expect(
    screen.queryByRole("button", { name: "Add a key for the local AI server" }),
  ).toBeNull();
  expect(
    screen.getByRole("button", {
      name: "Remove the local AI server's API key",
    }),
  ).toBeTruthy();
});

test("the local trash removes the key through its own command", async () => {
  const saved: SettingsInfo[] = [];
  const backend = installBackend({
    remove_local_api_key: () => SETTINGS_LOCAL,
  });
  const user = userEvent.setup();
  const view = await renderMenu({
    settings: SETTINGS_LOCAL_KEYED,
    onSaved: (next) => saved.push(next),
    settle: settleStatuses,
  });

  await user.click(
    screen.getByRole("button", { name: "Remove the local AI server's API key" }),
  );
  expect(backend.callsTo("remove_local_api_key")).toHaveLength(1);
  view.rerenderWith(saved[0]);
  expect(
    screen.getByRole("button", { name: "Add a key for the local AI server" }),
  ).toBeTruthy();
});

// ---- The steppers ----------------------------------------------------------
// Game-style enum rows: ◁ Value ▷. The arrows cycle with WRAP (stepper.ts, a
// deliberate deviation from the menus' clamped highlight); the center value
// opens a floating option popover, the scoped third altitude. Esc is four
// layers exactly: recording, popover, menu, overlay.

test("the arrows cycle the mode with wrap", async () => {
  const backend = installBackend({ set_mode: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  const saved: SettingsInfo[] = [];
  const view = await renderMenu({
    onSaved: (next) => saved.push(next),
  });

  // Custom API is the FIRST option — the next arrow steps to Local AI…
  await user.click(
    screen.getByRole("button", { name: "Switch to the next answer source" }),
  );
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "local" }]);

  // …and from Local AI, the last option, the same arrow wraps back around.
  view.rerenderWith(saved[0]);
  await user.click(
    screen.getByRole("button", { name: "Switch to the next answer source" }),
  );
  expect(backend.callsTo("set_mode")).toEqual([
    { mode: "local" },
    { mode: "custom" },
  ]);
});

test("the value opens the popover: current checked, focused, aria-wired", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  const value = answersValue();
  expect(value.getAttribute("aria-expanded")).toBe("false");
  await user.click(value);

  expect(value.getAttribute("aria-expanded")).toBe("true");
  const current = screen.getByRole("button", {
    name: "Answer with your own provider",
  });
  expect(current.getAttribute("aria-current")).toBe("true");
  // Focus lands on the current option so AT hears the name and its state.
  expect(document.activeElement).toBe(current);
  // The wiring the caret used to carry: value → popover by id.
  expect(value.getAttribute("aria-controls")).toBe(
    current.closest(".stepper-popover")?.id,
  );
});

test("picking the current option closes the popover without IPC", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await openAnswers(user);
  const before = backend.calls.length;
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
  expect(backend.calls.length).toBe(before);
  expect(backend.callsTo("set_mode")).toHaveLength(0);
});

test("ArrowDown and ArrowUp walk the popover options, clamped", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await openAnswers(user);
  const custom = screen.getByRole("button", {
    name: "Answer with your own provider",
  });
  const local = screen.getByRole("button", {
    name: "Answer with a local AI server",
  });
  // Focus opened on the current option — Custom API, the first row.
  expect(document.activeElement).toBe(custom);
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(local);
  // Clamped, not wrapped — only the horizontal cycle wraps.
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(local);
  await user.keyboard("{ArrowUp}");
  expect(document.activeElement).toBe(custom);
});

test("ArrowLeft on the focused value cycles without opening", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  act(() => {
    answersValue().focus();
  });
  await user.keyboard("{ArrowLeft}");

  // From the first option the backward arrow wraps to Local AI.
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "local" }]);
  expect(
    screen.queryByRole("button", { name: "Answer with a local AI server" }),
  ).toBeNull();
});

test("Esc closes the popover first, the menu second", async () => {
  installBackend();
  const user = userEvent.setup();
  let closed = 0;
  await renderMenu({ onClose: () => closed++ });

  await openAnswers(user);
  await user.keyboard("{Escape}");
  expect(closed).toBe(0);
  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
  // The popover handed focus back to the value it came from.
  expect(document.activeElement).toBe(answersValue());

  await user.keyboard("{Escape}");
  expect(closed).toBe(1);
});

test("an outside click closes the popover and leaves the menu open", async () => {
  installBackend();
  const user = userEvent.setup();
  let closed = 0;
  await renderMenu({ onClose: () => closed++ });

  await openAnswers(user);
  await user.click(screen.getByText("Shortcuts"));

  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
  expect(closed).toBe(0);
});

test("scrolling the settings list closes the popover", async () => {
  installBackend();
  const user = userEvent.setup();
  const view = await renderMenu();

  await openAnswers(user);
  const list = view.container.querySelector(".menu-list--settings");
  expect(list).not.toBeNull();
  fireEvent.scroll(list as Element);

  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
});

test("the overlay hiding closes the popover", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await openAnswers(user);
  await fireBackendEvent("overlay://hidden");

  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
});

test("a mode flip in flight gates the whole stepper", async () => {
  const gate = deferred<SettingsInfo>();
  const backend = installBackend();
  backend.onCommand("set_mode", () => gate.promise);
  const user = userEvent.setup();
  await renderMenu();

  await user.click(
    screen.getByRole("button", { name: "Switch to the next answer source" }),
  );
  // In flight: every stepper control waits (busyAll).
  expect(answersValue().hasAttribute("disabled")).toBe(true);
  expect(
    screen
      .getByRole("button", { name: "Switch to the next answer source" })
      .hasAttribute("disabled"),
  ).toBe(true);

  await act(async () => {
    gate.resolve(SETTINGS_LOCAL);
  });
  expect(answersValue().hasAttribute("disabled")).toBe(false);
});

test("arming a recorder closes an open option popover", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await user.click(screen.getByRole("button", { name: "Choose the theme" }));
  expect(
    screen.getByRole("button", { name: "Use the default theme" }),
  ).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Change the Summon shortcut" }),
  );

  expect(
    screen.queryByRole("button", { name: "Use the default theme" }),
  ).toBeNull();
  expect(screen.getByText(/Press the new shortcut/)).toBeTruthy();
});

// ---- What the panel says before it knows -----------------------------------

test("the needs-a-key note stays silent while the status fetch is open", async () => {
  const gate = deferred<KeyStatus[]>();
  const backend = installBackend();
  backend.onCommand("list_key_status", () => gate.promise);
  await renderMenu({ settle: () => screen.findByText("Loading…") });

  // Derived from a bare some(), the note read false while statuses is null —
  // so it flashed on every open and pinned forever when the fetch failed.
  expect(answersValue().textContent).not.toContain("Needs a key");

  await act(async () => {
    gate.resolve(KEY_STATUS);
  });
  expect(answersValue().textContent).toContain("Needs a key");
});

test("a failed key-status read says so instead of reading as no keys", async () => {
  installBackend({
    list_key_status: () => {
      throw "Couldn't read your API keys: the store is unreadable";
    },
  });
  await renderMenu({ settle: () => screen.findByText(/store is unreadable/) });

  // The failure is panel-scope: tagged to a row, it would have gone
  // unrendered on a Local-mode install where no key line exists. And a
  // failed read must not masquerade as "no keys anywhere".
  expect(answersValue().textContent).not.toContain("Needs a key");
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

test("Esc is layered while armed: cancel recording, then close", async () => {
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

// ---- The Position stepper + padlock -----------------------------------------
// The IPC template again (pickMode's twin): each pick round-trips
// set_panel_position and the fresh SettingsInfo comes back through onSaved;
// the padlock is its own single-purpose command (set_position_locked). The
// lock pins the drag gesture only — the stepper stays live while locked.

/** The Position stepper's center value. */
const positionValue = () =>
  screen.getByRole("button", { name: "Choose the panel position" });

/** SETTINGS moved to a stored bottom-left anchor. */
const ON_BOTTOM_LEFT: SettingsInfo = {
  ...SETTINGS,
  position: {
    mode: "anchored",
    anchor: "bottom-left",
    locked: false,
    manualEdge: null,
  },
};

test("the position popover lists all six placements", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await user.click(positionValue());
  for (const name of [
    "Dock the panel to the top right",
    "Dock the panel to the top left",
    "Dock the panel to the bottom right",
    "Dock the panel to the bottom left",
    "Center the panel on the screen",
    "Keep the panel where you drag it",
  ]) {
    expect(screen.getByRole("button", { name })).toBeTruthy();
  }
});

test("a placement pick round-trips set_panel_position through onSaved", async () => {
  const backend = installBackend({ set_panel_position: () => ON_BOTTOM_LEFT });
  let saved: SettingsInfo | null = null;
  const user = userEvent.setup();
  const menu = await renderMenu({ onSaved: (next) => (saved = next) });

  await user.click(positionValue());
  await user.click(
    screen.getByRole("button", { name: "Dock the panel to the bottom left" }),
  );

  expect(backend.callsTo("set_panel_position")).toEqual([
    { choice: "bottom-left" },
  ]);
  expect(saved).toEqual(ON_BOTTOM_LEFT);
  // The controlled view: the value moves once the fresh settings come back.
  menu.rerenderWith(ON_BOTTOM_LEFT);
  expect(positionValue().textContent).toContain("Bottom Left");
});

test("the arrows cycle the placement with wrap", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu();

  // top-right is the first option — the previous arrow wraps to Manual.
  await user.click(
    screen.getByRole("button", { name: "Switch to the previous position" }),
  );
  expect(backend.callsTo("set_panel_position")).toEqual([{ choice: "manual" }]);
});

test("a failed placement pick renders its error under the stepper", async () => {
  installBackend({
    set_panel_position: () => {
      throw "the settings file is read-only";
    },
  });
  const user = userEvent.setup();
  await renderMenu();

  await user.click(positionValue());
  await user.click(
    screen.getByRole("button", { name: "Center the panel on the screen" }),
  );
  await screen.findByText("the settings file is read-only");
});

test("the padlock toggles set_position_locked and reflects the stored state", async () => {
  const backend = installBackend({
    set_position_locked: () => SETTINGS_POSITION_LOCKED,
  });
  let saved: SettingsInfo | null = null;
  const user = userEvent.setup();
  const menu = await renderMenu({ onSaved: (next) => (saved = next) });

  const lock = screen.getByRole("button", { name: "Lock the panel position" });
  expect(lock.getAttribute("aria-pressed")).toBe("false");
  await user.click(lock);

  expect(backend.callsTo("set_position_locked")).toEqual([{ locked: true }]);
  expect(saved).toEqual(SETTINGS_POSITION_LOCKED);

  menu.rerenderWith(SETTINGS_POSITION_LOCKED);
  const unlock = screen.getByRole("button", {
    name: "Unlock the panel position",
  });
  expect(unlock.getAttribute("aria-pressed")).toBe("true");
});

test("the stepper stays live while locked — the lock pins the gesture only", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderMenu({ settings: SETTINGS_POSITION_LOCKED });

  await user.click(
    screen.getByRole("button", { name: "Switch to the next position" }),
  );
  expect(backend.callsTo("set_panel_position")).toEqual([
    { choice: "top-left" },
  ]);
});

test("a placement pick in flight gates the panel's other controls", async () => {
  const gate = deferred<SettingsInfo>();
  installBackend({ set_panel_position: () => gate.promise });
  const user = userEvent.setup();
  await renderMenu();

  await user.click(positionValue());
  await user.click(
    screen.getByRole("button", { name: "Dock the panel to the bottom left" }),
  );
  expect(
    screen.getByRole("button", { name: "Lock the panel position" }),
  ).toHaveProperty("disabled", true);
  expect(answersValue()).toHaveProperty("disabled", true);

  gate.resolve(ON_BOTTOM_LEFT);
  await act(async () => {});
  expect(answersValue()).toHaveProperty("disabled", false);
});

test("opening the position popover closes the answers popover", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderMenu();

  await openAnswers(user);
  expect(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  ).toBeTruthy();

  await user.click(positionValue());
  expect(
    screen.queryByRole("button", { name: "Answer with your own provider" }),
  ).toBeNull();
  expect(
    screen.getByRole("button", { name: "Keep the panel where you drag it" }),
  ).toBeTruthy();
});
