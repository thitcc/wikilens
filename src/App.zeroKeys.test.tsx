// The zero-keys Custom state: with no keyed providers, the footer offers a
// "Set up a model" chip that opens Settings instead of a dead end, submit
// stays blocked, and saving a key in the panel re-fetches the provider list.
// Local mount helper — the shared renderApp awaits a `Model:` chip that
// doesn't exist with an empty provider list.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import App from "./App";
import { KEY_STATUS, PROVIDERS, installBackend } from "./test/backend";
import type { ProviderInfo } from "./types";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

/** Mount with an empty provider list and settle on the CTA chip. */
async function renderZeroKeysApp() {
  const result = render(<App />);
  await screen.findByRole("button", { name: /^Game: (?!Loading)/ });
  await screen.findByRole("button", { name: "Set up a model" });
  return result;
}

test("zero keyed providers shows the Set up a model chip", async () => {
  installBackend({ list_providers: () => [] });
  await renderZeroKeysApp();

  expect(screen.queryByRole("button", { name: /^Model: / })).toBeNull();
});

test("the chip opens the Settings panel", async () => {
  installBackend({ list_providers: () => [] });
  const user = userEvent.setup();
  await renderZeroKeysApp();

  await user.click(screen.getByRole("button", { name: "Set up a model" }));
  expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
  // The dead end ends in the answer-source list: a row per provider, each one
  // an invitation to key it.
  await screen.findByRole("button", { name: "Add a key for Anthropic" });
});

test("submit stays blocked with no provider", async () => {
  const backend = installBackend({ list_providers: () => [] });
  const user = userEvent.setup();
  await renderZeroKeysApp();

  await user.type(questionBox(), "how do I fish{Enter}");
  expect(backend.callsTo("ask")).toHaveLength(0);
});

test("the whole first run: pick a provider, key it, get the Model chip", async () => {
  // The integration pin for the first-run flow — CTA → row → field → Save →
  // chip. Stateful backend: the provider list is empty until a key is saved.
  let keyed = false;
  const backend = installBackend({
    list_providers: (): ProviderInfo[] => (keyed ? PROVIDERS : []),
    set_api_key: () => {
      keyed = true;
      return KEY_STATUS.map((s) =>
        s.id === "anthropic" ? { ...s, hasKey: true } : s,
      );
    },
  });
  const user = userEvent.setup();
  await renderZeroKeysApp();
  expect(backend.callsTo("list_providers")).toHaveLength(1);

  await user.click(screen.getByRole("button", { name: "Set up a model" }));

  // Clicking the unkeyed row is a pure UI move — it opens that row's field
  // and touches no IPC until there is something to store.
  await user.click(
    await screen.findByRole("button", { name: "Add a key for Anthropic" }),
  );
  expect(backend.callsTo("set_api_key")).toHaveLength(0);

  await user.type(screen.getByLabelText("Anthropic API key"), "sk-ant-test");
  await user.click(
    screen.getByRole("button", { name: "Save the Anthropic API key" }),
  );

  // The CTA is replaced by the real chip, named for the provider just keyed,
  // on its default model.
  expect(
    await screen.findByRole("button", {
      name: "Model: Anthropic Claude Sonnet 5",
    }),
  ).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Set up a model" })).toBeNull();
  // That swap took two things: onKeysChanged bumping the provider fetch (the
  // list was empty before the key existed)...
  expect(backend.callsTo("list_providers")).toHaveLength(2);
  // ...and the same Save committing the answer source — the mode leaves its
  // never-chosen state without the player ever naming it.
  expect(backend.callsTo("set_mode")).toEqual([{ mode: "custom" }]);
  // The field collapsed on success — the key never lingers on screen.
  expect(screen.queryByLabelText("Anthropic API key")).toBeNull();
});
