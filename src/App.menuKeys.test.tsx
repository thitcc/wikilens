// Arrow-key navigation in the owned menus: Up/Down move a visible highlight,
// Home/End jump, Enter picks the highlighted row — never an invisible first
// match (the 2026-07-19 critique's heuristic-5 finding). All keyboard input
// goes through userEvent (harness rule: a raw KeyboardEvent dispatch runs
// capture+bubble in registration order and silently inverts the Esc layering).

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { MODEL_STORAGE_PREFIX } from "./modelPick";
import { MODELS, PROVIDERS, installBackend } from "./test/backend";
import { renderApp } from "./test/harness";

function questionBox(): HTMLTextAreaElement {
  return screen.getByRole("textbox", {
    name: "Question",
  }) as HTMLTextAreaElement;
}

function highlighted(dialog: HTMLElement): Element[] {
  return [...dialog.querySelectorAll(".model-row.is-highlighted")];
}

test("game menu: the selected row opens highlighted and bare Enter re-picks it", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");

  // Enter picks the visibly highlighted row — a harmless re-pick, not an
  // invisible first match — closes the menu, and refocuses the prompt.
  await user.keyboard("{Enter}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("button", { name: /^Game: Terraria/ })).toBeTruthy();
  expect(document.activeElement).toBe(questionBox());
});

test("game menu: arrows move the highlight and Enter picks it, focus staying on the filter", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });
  const filter = screen.getByRole("textbox", { name: "Filter games" });

  // No recents yet, so rows are alphabetical: My Wiki, Stardew Valley,
  // Terraria — the selected game opens highlighted at the bottom.
  await user.keyboard("{ArrowUp}");
  let rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Stardew Valley");
  expect(document.activeElement).toBe(filter);

  await user.keyboard("{Enter}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(
    screen.getByRole("button", { name: /^Game: Stardew Valley/ }),
  ).toBeTruthy();
});

test("game menu: Home and End jump within the game rows only", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });

  await user.keyboard("{Home}");
  expect(highlighted(dialog)[0].textContent).toContain("My Wiki");

  await user.keyboard("{End}");
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Terraria");
  // The pinned footer action is Tab territory, never arrow territory.
  expect(
    dialog.querySelector(".menu-action")?.classList.contains("is-highlighted"),
  ).toBe(false);
});

test("game menu: typing keeps filtering and Enter picks the visible first match", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  const dialog = screen.getByRole("dialog", { name: "Choose a game" });

  await user.keyboard("sta");
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Stardew Valley");

  await user.keyboard("{Enter}");
  expect(
    screen.getByRole("button", { name: /^Game: Stardew Valley/ }),
  ).toBeTruthy();
});

test("game menu: Enter on a no-match filter is a no-op and the menu stays open", async () => {
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  await user.keyboard("zzz{Enter}");
  expect(screen.getByRole("dialog", { name: "Choose a game" })).toBeTruthy();
  expect(screen.getByRole("button", { name: /^Game: Terraria/ })).toBeTruthy();
});

test("model menu: bare Enter re-picks the selected model, not the invisible first match", async () => {
  // The 2026-07-19 critique's heuristic-5 pin. Seed a stored pick for the
  // fixture's SECOND model so selected ≠ first: on main, Enter silently
  // picked the first match (Sonnet) — this test fails there and passes now.
  localStorage.setItem(
    MODEL_STORAGE_PREFIX + "anthropic",
    JSON.stringify({ id: "claude-haiku-4-5", label: "Claude Haiku 4.5", vision: true }),
  );
  installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Model: / }));
  const dialog = screen.getByRole("dialog", { name: "Choose a model" });
  await screen.findAllByRole("button", { name: /Claude Haiku 4\.5/ });

  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Claude Haiku 4.5");

  await user.keyboard("{Enter}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(
    screen.getByRole("button", { name: /^Model: .*Claude Haiku 4\.5/ }),
  ).toBeTruthy();
});

test("model menu: arrows skip a collapsed group and never land on a header", async () => {
  const DEEPSEEK_MODELS = {
    models: [{ id: "deepseek-chat", label: "DeepSeek Chat", vision: false }],
    source: "live",
  };
  const backend = installBackend({
    // OpenRouter sits mid-list and starts collapsed — the arrow order must
    // jump straight from Anthropic's last row to DeepSeek's first.
    list_providers: () => [
      PROVIDERS[0],
      {
        id: "openrouter",
        name: "OpenRouter",
        defaultModel: "openrouter/auto",
        defaultModelLabel: "Auto Router",
        defaultModelVision: false,
      },
      PROVIDERS[1],
    ],
    list_models: (args) =>
      (args as { providerId: string }).providerId === "anthropic"
        ? MODELS
        : DEEPSEEK_MODELS,
  });
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Model: / }));
  const dialog = screen.getByRole("dialog", { name: "Choose a model" });
  await screen.findByRole("button", { name: /DeepSeek Chat/ });

  // Selected (Sonnet) opens highlighted; two ArrowDowns cross the collapsed
  // OpenRouter group without stopping.
  await user.keyboard("{ArrowDown}{ArrowDown}");
  const rows = highlighted(dialog);
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("DeepSeek Chat");
  // The collapsed group's models were never fetched, let alone traversed.
  expect(
    backend
      .callsTo("list_models")
      .map((a) => (a as { providerId: string }).providerId),
  ).not.toContain("openrouter");
});

test("Esc layering is untouched by a moved highlight", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Game: Terraria/ }));
  await user.keyboard("{ArrowUp}{ArrowUp}");

  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(backend.callsTo("hide_overlay")).toHaveLength(0);

  await user.keyboard("{Escape}");
  expect(backend.callsTo("hide_overlay")).toHaveLength(1);
});
