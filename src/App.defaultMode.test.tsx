// Default mode's frontend contract: the footer shows only the static
// "Default" chip (vendor invisible), list_providers is never called, asks
// send blank provider/model args (Rust resolves the target), and capture
// gates on settings.defaultMode.vision instead of any model pick. Local
// mount helper — the shared renderApp awaits a `Model:` chip that
// deliberately doesn't exist here.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import App from "./App";
import {
  KEY_STATUS,
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

  expect(screen.getByText("text-only model")).toBeTruthy();
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

  expect(screen.queryByText("text-only model")).toBeNull();
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

test("keying a provider in Default mode stores the key and stays in Default", async () => {
  const backend = installBackend({
    get_settings: () => SETTINGS_DEFAULT,
    set_api_key: () =>
      KEY_STATUS.map((s) =>
        s.id === "anthropic" ? { ...s, hasKey: true } : s,
      ),
  });
  const user = userEvent.setup();
  await renderDefaultApp();
  expect(backend.callsTo("list_providers")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Settings" }));
  // Default mode owns the check: the built-in row is the live answer source.
  await screen.findByText("Needs a key");
  const builtIn = screen.getByRole("button", {
    name: "Answer with the built-in model",
  });
  expect(builtIn.getAttribute("aria-current")).toBe("true");

  // Default mode mounts the key lines collapsed — reaching them is a click on
  // the caret, and that click is not a mode change.
  await user.click(screen.getByRole("button", { name: "Show provider keys" }));
  await user.click(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  );
  await user.type(screen.getByLabelText("Anthropic API key"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  expect(backend.callsTo("set_api_key")).toEqual([
    { providerId: "anthropic", key: "sk-ant-test" },
  ]);
  // A key is storage, not a choice
  // (vault/2026-07-29_keys-are-not-a-mode-choice.md): the mode never moves, so
  // the provider fetch never arms and the footer keeps its static chip.
  expect(backend.callsTo("set_mode")).toHaveLength(0);
  expect(backend.callsTo("list_providers")).toHaveLength(0);
  expect(
    screen
      .getByRole("button", { name: "Answer with the built-in model" })
      .getAttribute("aria-current"),
  ).toBe("true");
  expect(screen.queryByRole("button", { name: /^Model: / })).toBeNull();
  expect(screen.getByText("Default")).toBeTruthy();
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
  await screen.findByText("Needs a key");

  // Leaving Default is now one thing only: pick the other mode row.
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  // The mode flip re-arms the provider fetch; the footer swaps to the chip.
  expect(await screen.findByRole("button", { name: /^Model: / })).toBeTruthy();
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  expect(backend.callsTo("list_providers")).toHaveLength(1);
  // And the disclosure follows the mode, so the keys you now need are open.
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();

  // Nothing in the panel says "Default" any more — the built-in row reads
  // "Built into WikiLens" — so this is purely about the footer: close the
  // panel and confirm the static chip really left it.
  expect(screen.getByText("Built into WikiLens")).toBeTruthy();
  await user.keyboard("{Escape}");
  expect(screen.queryByText("Default")).toBeNull();
});
