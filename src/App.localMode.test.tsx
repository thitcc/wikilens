// Local mode's frontend contract: the footer shows a real "Local" model chip
// fed by the live local catalog, list_providers is never called, asks send a
// blank provider id + the local pick's model id, capture gates on the
// Settings eye (settings.localMode.vision) instead of any model badge, and a
// Local pick never touches the shared selectedProvider slot. Local mount
// helper — the shared renderApp awaits an Anthropic-flavored `Model:` chip.

import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import App from "./App";
import {
  LOCAL_MODELS,
  SETTINGS_LOCAL,
  SETTINGS_LOCAL_VISION,
  installBackend,
} from "./test/backend";
import { fireBackendEvent } from "./test/harness";
import { MODEL_STORAGE_PREFIX, PROVIDER_STORAGE_KEY } from "./modelPick";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

/** A stored local pick, as handleModelSelect writes it. */
function storeLocalPick() {
  localStorage.setItem(
    MODEL_STORAGE_PREFIX + "local",
    JSON.stringify({ id: "llama3.2:3b", label: "llama3.2:3b", vision: false }),
  );
}

/** Mount in Local mode and settle on the Local chip (the mount IPC's last
 * visible consequence here — keeps state updates inside act). */
async function renderLocalApp() {
  const result = render(<App />);
  await screen.findByRole("button", { name: /^Game: (?!Loading)/ });
  await screen.findByRole("button", { name: /^Model: Local / });
  return result;
}

test("Local mode renders the Local chip and never calls list_providers", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  await renderLocalApp();

  // A real chip with the no-pick label — and no registry surface anywhere.
  expect(
    screen.getByRole("button", { name: "Model: Local Choose a model" }),
  ).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Set up a model" })).toBeNull();
  expect(backend.callsTo("list_providers")).toHaveLength(0);
});

test("submitting sends a blank provider id and the local pick's model", async () => {
  storeLocalPick();
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  await renderLocalApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  await screen.findByText(/to raise spawn rates/);
  expect(backend.callsTo("ask")).toEqual([
    {
      gameId: "terraria",
      providerId: "",
      model: "llama3.2:3b",
      question: "how do I fish",
      imageId: null,
    },
  ]);
});

test("submitting without a model pick is gated frontend-side", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  await renderLocalApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  expect(backend.callsTo("ask")).toHaveLength(0);
});

test("capture is gated off while the Settings eye is off", async () => {
  storeLocalPick();
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  await renderLocalApp();

  const capture = screen.getByTitle("This model can't read images");
  expect(capture.hasAttribute("disabled")).toBe(true);

  // The hotkey path is never silent: show the panel, explain with the
  // Local-mode copy (the fix is the Settings eye, not a menu badge).
  await fireBackendEvent("capture://hotkey");
  expect(backend.callsTo("begin_capture")).toHaveLength(0);
  expect(backend.callsTo("show_overlay")).toHaveLength(1);
  expect(screen.getByText(/This model can't read images/)).toBeTruthy();
  expect(screen.getByText(/turn on the eye in Settings/)).toBeTruthy();
  expect(screen.queryByText(/Image badge/)).toBeNull();
});

test("capture arms when the Settings eye is on", async () => {
  storeLocalPick();
  const backend = installBackend({
    get_settings: () => SETTINGS_LOCAL_VISION,
  });
  await renderLocalApp();

  await fireBackendEvent("capture://hotkey");
  expect(backend.callsTo("begin_capture")).toHaveLength(1);
});

test("a stale pre-badges pick never fetches models in Local mode", async () => {
  // A visionless stored Custom pick would trigger the self-heal list_models
  // fetch in Custom mode — Local mode gates on the Settings eye instead, so
  // no fetch may fire (the local pick's own vision flag is never consulted).
  localStorage.setItem(PROVIDER_STORAGE_KEY, "anthropic");
  localStorage.setItem(
    MODEL_STORAGE_PREFIX + "anthropic",
    JSON.stringify({ id: "claude-sonnet-5", label: "Claude Sonnet 5" }),
  );
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  await renderLocalApp();

  expect(backend.callsTo("list_models")).toHaveLength(0);
  expect(backend.callsTo("list_providers")).toHaveLength(0);
});

test("the Local menu lists the live catalog and a pick stays out of the provider slot", async () => {
  localStorage.setItem(PROVIDER_STORAGE_KEY, "anthropic");
  const backend = installBackend({
    get_settings: () => SETTINGS_LOCAL,
    list_models: () => LOCAL_MODELS,
  });
  const user = userEvent.setup();
  await renderLocalApp();

  await user.click(
    screen.getByRole("button", { name: "Model: Local Choose a model" }),
  );
  // The one group is the local server's; its fetch goes through the reserved
  // "local" id.
  await user.click(await screen.findByRole("button", { name: /qwen3:8b/ }));
  expect(backend.callsTo("list_models")).toEqual([{ providerId: "local" }]);

  // The pick lands in the reserved slot, the chip follows…
  expect(localStorage.getItem(MODEL_STORAGE_PREFIX + "local")).toContain(
    "qwen3:8b",
  );
  expect(
    screen.getByRole("button", { name: "Model: Local qwen3:8b" }),
  ).toBeTruthy();
  // …and the remembered Custom provider survives untouched (the poisoning
  // trap: "local" in this key would clobber it on the next mode flip).
  expect(localStorage.getItem(PROVIDER_STORAGE_KEY)).toBe("anthropic");
});

test("Local mode renders no registry key lines — Settings shows the server rows", async () => {
  const backend = installBackend({ get_settings: () => SETTINGS_LOCAL });
  const user = userEvent.setup();
  await renderLocalApp();
  expect(backend.callsTo("list_providers")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Settings" }));
  // Local AI owns the value…
  const value = await screen.findByRole("button", {
    name: "Choose the answer source",
  });
  expect(value.textContent).toContain("Local AI");
  // …the registry key lines exist exactly while Custom API answers — none
  // here — and the Local rows are there instead: address field (prefilled
  // with the effective URL), optional key line, the vision eye.
  expect(
    screen.queryByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeNull();
  expect(
    (
      screen.getByRole("textbox", {
        name: "Local AI server address",
      }) as HTMLInputElement
    ).value,
  ).toBe("http://localhost:11434/v1");
  expect(
    screen.getByRole("button", { name: "Add a key for the local AI server" }),
  ).toBeTruthy();
  expect(
    screen.getByRole("button", {
      name: "Mark the local model as able to read images",
    }),
  ).toBeTruthy();
  // The open-time key-status fetch has no visible artifact in Local mode —
  // flush it inside act so it can't land as a stray warning.
  await act(async () => {});
});

test("picking your own provider leaves Local mode and restores the provider chip", async () => {
  const backend = installBackend({
    get_settings: () => SETTINGS_LOCAL,
    set_mode: () => ({ ...SETTINGS_LOCAL, mode: "custom" as const }),
  });
  const user = userEvent.setup();
  await renderLocalApp();
  expect(backend.callsTo("list_providers")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Settings" }));

  // Leaving Local is the stepper's job: open the value's popover and pick
  // Custom API.
  await user.click(
    await screen.findByRole("button", { name: "Choose the answer source" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Answer with your own provider" }),
  );

  // The mode flip re-arms the provider fetch; the footer swaps to the
  // registry chip.
  expect(
    await screen.findByRole("button", { name: /^Model: Anthropic / }),
  ).toBeTruthy();
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  expect(backend.callsTo("list_providers")).toHaveLength(1);
  // And the keys follow the mode end-to-end: the fresh settings came back
  // through onSaved → setSettings, and the derived key lines are now there.
  expect(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  ).toBeTruthy();
});
