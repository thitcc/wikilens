import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ModelMenu } from "./ModelMenu";
import type { ModelList } from "../types";
import { MODELS, PROVIDERS, deferred, installBackend } from "../test/backend";

// Direct-render suite for the highlight edge cases the async lists create.
// App.menuKeys.test.tsx covers the user-visible flows (incl. the critique pin).

function renderMenu() {
  const onSelect = vi.fn();
  render(
    <ModelMenu
      providers={PROVIDERS}
      selected={{ providerId: "anthropic", modelId: "claude-sonnet-5" }}
      onSelect={onSelect}
      onClose={() => {}}
      chipRef={{ current: null }}
    />,
  );
  return { onSelect };
}

function highlighted(): Element[] {
  return [...document.querySelectorAll(".model-row.is-highlighted")];
}

function filterBox(): HTMLElement {
  return screen.getByRole("textbox", { name: "Filter models" });
}

test("arrows before the lists arrive are safe; the highlight lands on the selected row once they do", async () => {
  const backend = installBackend();
  const gate = deferred<ModelList>();
  backend.onCommand("list_models", () => gate.promise);
  renderMenu();
  const user = userEvent.setup();

  await user.click(filterBox());
  await user.keyboard("{ArrowDown}{End}");
  expect(highlighted()).toHaveLength(0);

  await act(async () => {
    gate.resolve(MODELS);
  });

  // Both fixture providers share the MODELS list, so the label appears twice;
  // only the *selected* provider's copy is highlighted.
  const rows = highlighted();
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Claude Sonnet 5");
  expect(rows[0].getAttribute("aria-current")).toBe("true");
});

test("collapsing the highlighted group drops the highlight; the next arrow restarts at the top visible row", async () => {
  installBackend();
  renderMenu();
  const user = userEvent.setup();
  await screen.findAllByRole("button", { name: /Claude Haiku 4\.5/ });

  await user.click(filterBox());
  await user.keyboard("{ArrowDown}");
  expect(highlighted()[0].textContent).toContain("Claude Haiku 4.5");

  // Collapse Anthropic — the highlighted row vanishes with its group.
  await user.click(screen.getByRole("button", { name: /Anthropic/ }));
  expect(highlighted()).toHaveLength(0);

  // Every remaining row belongs to DeepSeek; back on the filter (the click
  // parked focus on the header), ArrowDown re-enters at its top.
  await user.click(filterBox());
  await user.keyboard("{ArrowDown}");
  const rows = highlighted();
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain("Claude Sonnet 5");
});

test("the group holding the selection opens expanded instead of collapsed", async () => {
  const backend = installBackend({
    list_models: (args) =>
      (args as { providerId: string }).providerId === "openrouter"
        ? {
            models: [{ id: "openrouter/auto", label: "Auto Router", vision: false }],
            source: "live",
          }
        : MODELS,
  });
  render(
    <ModelMenu
      providers={[
        ...PROVIDERS,
        {
          id: "openrouter",
          name: "OpenRouter",
          defaultModel: "openrouter/auto",
          defaultModelLabel: "Auto Router",
          defaultModelVision: false,
        },
      ]}
      selected={{ providerId: "openrouter", modelId: "openrouter/auto" }}
      onSelect={vi.fn()}
      onClose={() => {}}
      chipRef={{ current: null }}
    />,
  );

  // The selection's group renders expanded and its fetch is NOT deferred —
  // the picked row must be visible on open, not hidden behind a collapsed
  // header (the pre-fix behavior).
  const row = await screen.findByRole("button", { name: /Auto Router/ });
  expect(
    screen
      .getByRole("button", { name: /OpenRouter/ })
      .getAttribute("aria-expanded"),
  ).toBe("true");
  expect(
    backend
      .callsTo("list_models")
      .map((a) => (a as { providerId: string }).providerId),
  ).toContain("openrouter");
  // And it is the initial keyboard highlight, like any selected row.
  expect(row.classList.contains("is-highlighted")).toBe(true);
  expect(row.getAttribute("aria-current")).toBe("true");
});

test("Enter hands the highlighted provider and model to onSelect", async () => {
  installBackend();
  const { onSelect } = renderMenu();
  const user = userEvent.setup();
  await screen.findAllByRole("button", { name: /Claude Haiku 4\.5/ });

  await user.click(filterBox());
  await user.keyboard("{ArrowDown}{Enter}");
  expect(onSelect).toHaveBeenCalledWith(
    "anthropic",
    expect.objectContaining({ id: "claude-haiku-4-5" }),
  );
});
