import { act, screen, waitFor } from "@testing-library/react";
import { expect, test } from "vitest";
import { MODEL_STORAGE_PREFIX, PROVIDER_STORAGE_KEY } from "./modelPick";
import { installBackend } from "./test/backend";
import { renderApp } from "./test/harness";

test("corrupted localStorage everywhere still renders with sane fallbacks", async () => {
  localStorage.setItem("wikilens.recentGames", "{definitely not json");
  localStorage.setItem("wikilens.selectedGame", "no-such-game");
  localStorage.setItem(PROVIDER_STORAGE_KEY, "anthropic");
  localStorage.setItem(MODEL_STORAGE_PREFIX + "anthropic", "{broken too");

  const backend = installBackend();
  await renderApp();

  // The stale game id fell back to the first listed game.
  expect(screen.getByRole("button", { name: "Game: Terraria" })).toBeTruthy();
  // The corrupted pick fell back to the provider's default label.
  expect(
    screen.getByRole("button", { name: "Model: Anthropic Claude Sonnet 5" }),
  ).toBeTruthy();
  // No pick → nothing to self-heal → no model-list fetch on mount.
  await act(async () => {});
  expect(backend.callsTo("list_models")).toHaveLength(0);
});

test("a pre-badges pick self-heals its vision flag with exactly one list_models", async () => {
  localStorage.setItem(PROVIDER_STORAGE_KEY, "anthropic");
  localStorage.setItem(
    MODEL_STORAGE_PREFIX + "anthropic",
    JSON.stringify({ id: "claude-haiku-4-5", label: "Claude Haiku 4.5" }),
  );

  const backend = installBackend();
  await renderApp();

  await waitFor(() => {
    const raw = localStorage.getItem(MODEL_STORAGE_PREFIX + "anthropic");
    expect(JSON.parse(raw ?? "null")).toEqual({
      id: "claude-haiku-4-5",
      label: "Claude Haiku 4.5",
      vision: true,
    });
  });
  expect(backend.callsTo("list_models")).toEqual([{ providerId: "anthropic" }]);

  // The heal must terminate: an extra flush fires no second fetch (the
  // provider re-read effect keeps the previous pick when a re-read is
  // content-equal — without that, the fresh object re-arms the heal).
  await act(async () => {});
  expect(backend.callsTo("list_models")).toHaveLength(1);
});
