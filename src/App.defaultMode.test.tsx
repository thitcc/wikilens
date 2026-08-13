// Default mode's frontend contract: the footer shows only the static
// "Default" chip (vendor invisible), list_providers is never called, asks
// send blank provider/model args (Rust resolves the target), and capture
// gates on settings.defaultMode.vision instead of any model pick. Local
// mount helper — the shared renderApp awaits a `Model:` chip that
// deliberately doesn't exist here.

import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import App from "./App";
import {
  SETTINGS_DEFAULT,
  SETTINGS_DEFAULT_VISION,
  installBackend,
} from "./test/backend";
import { fireBackendEvent } from "./test/harness";
import { MODEL_STORAGE_PREFIX, PROVIDER_STORAGE_KEY } from "./modelPick";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

/** Mount in Default mode and settle on the static chip (the mount IPC's last
 * visible consequence here — keeps state updates inside act). */
async function renderDefaultApp() {
  const result = render(<App />);
  await screen.findByRole("button", { name: /^Game: (?!Loading)/ });
  await screen.findByText("Default");
  return result;
}

test("Default mode renders the static chip and never calls list_providers", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_DEFAULT });
  await renderDefaultApp();

  // A label, not a control: nothing to open, no vendor or model anywhere.
  expect(screen.queryByRole("button", { name: /^Model: / })).toBeNull();
  expect(screen.queryByRole("button", { name: "Set up a model" })).toBeNull();
  expect(backend.callsTo("list_providers")).toHaveLength(0);
});

test("submitting sends blank provider and model args", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_DEFAULT });
  const user = userEvent.setup();
  await renderDefaultApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await screen.findByText(/to raise spawn rates/);
  expect(backend.callsTo("ask")).toEqual([
    {
      gameId: "terraria",
      providerId: "",
      model: "",
      question: "how do I fish",
      imageId: null,
    },
  ]);
});

test("capture is gated off when the Default target is text-only", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_DEFAULT });
  await renderDefaultApp();

  // No footer label for text-only targets — the chip's title carries the why.
  expect(screen.queryByText("text-only model")).toBeNull();
  const capture = screen.getByTitle("This model can't read images");
  expect(capture.hasAttribute("disabled")).toBe(true);

  // The hotkey path is never silent: show the panel, explain with the
  // Default-mode copy (no "Image badge" — there's no menu to pick from).
  await fireBackendEvent("capture://hotkey");
  expect(backend.callsTo("begin_capture")).toHaveLength(0);
  expect(backend.callsTo("show_overlay")).toHaveLength(1);
  expect(screen.getByText(/This model can't read images/)).toBeTruthy();
  expect(screen.queryByText(/Image badge/)).toBeNull();
});

test("capture arms when WIKILENS_DEFAULT_VISION is on", async () => {
  const backend = installBackend({
    get_settings: () => SETTINGS_DEFAULT_VISION,
  });
  await renderDefaultApp();

  await fireBackendEvent("capture://hotkey");
  expect(backend.callsTo("begin_capture")).toHaveLength(1);
});

test("a stale pre-badges pick never fetches models in Default mode", async () => {
  // A visionless stored pick would trigger the self-heal list_models fetch in
  // Custom mode — Default mode must stay vendor-silent.
  localStorage.setItem(PROVIDER_STORAGE_KEY, "anthropic");
  localStorage.setItem(
    MODEL_STORAGE_PREFIX + "anthropic",
    JSON.stringify({ id: "claude-sonnet-5", label: "Claude Sonnet 5" }),
  );
  const backend = installBackend({ get_settings: () => SETTINGS_DEFAULT });
  await renderDefaultApp();

  expect(backend.callsTo("list_models")).toHaveLength(0);
  expect(backend.callsTo("list_providers")).toHaveLength(0);
});

test("Default mode renders no key lines — adding a key means Custom API first", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_DEFAULT });
  const user = userEvent.setup();
  await renderDefaultApp();
  expect(backend.callsTo("list_providers")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Settings" }));
  // Default mode owns the value: Built In is the live answer source…
  const value = await screen.findByRole("button", {
    name: "Choose the answer source",
  });
  expect(value.textContent).toContain("Built In");
  // …and the key lines exist exactly while Custom API answers — none here,
  // and no disclosure control pretends otherwise. (The keying-never-moves-the-
  // mode invariant lives in SettingsMenu.test.tsx, from Custom.)
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    screen.queryByRole("button", { name: /provider keys/ }),
  ).toBeNull();
  // Scoped to the footer: the open Settings dialog's theme stepper also says
  // "Default", and this assertion is about the static mode chip.
  expect(
    within(screen.getByRole("contentinfo")).getByText("Default"),
  ).toBeTruthy();
  // The open-time key-status fetch has no visible artifact in Default mode —
  // flush it inside act so it can't land as a stray warning.
  await act(async () => {});
});

test("picking your own provider leaves Default mode and restores the Model chip", async () => {
  const backend = installBackend({
    get_settings: () => SETTINGS_DEFAULT,
    // The install keeps its configured Default target — the player just
    // stopped answering with it.
    set_mode: () => ({ ...SETTINGS_DEFAULT, mode: "custom" as const }),
  });
  const user = userEvent.setup();
  await renderDefaultApp();
  expect(backend.callsTo("list_providers")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Settings" }));

  // Leaving Default is the stepper's job: open the value's popover and pick
  // Custom API.
  await user.click(
    await screen.findByRole("button", { name: "Choose the answer source" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  // The mode flip re-arms the provider fetch; the footer swaps to the chip.
  expect(await screen.findByRole("button", { name: /^Model: / })).toBeTruthy();
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  expect(backend.callsTo("list_providers")).toHaveLength(1);
  // And the keys follow the mode end-to-end: the fresh settings came back
  // through onSaved → setSettings, and the derived key lines are now there.
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();

  // The open dialog's theme stepper still says "Default", so the footer check
  // needs the panel gone: close it and confirm the static chip really left.
  await user.keyboard("{Escape}");
  expect(screen.queryByText("Default")).toBeNull();
});
