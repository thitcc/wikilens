// Placement mirroring. Ownership under test: App copies the stored placement
// (settings.position) onto the root element as data-anchor-v / data-anchor-h
// / data-position-mode — the data-theme idiom — so CSS can align the panel
// inside a cap-expanded window and pick the entrance motion. Manual carries
// no anchor facets (top-pinned growth from the dragged spot); the top-right
// default carries v="top" h="right" like any other anchor.

import { expect, test } from "vitest";
import type { SettingsInfo } from "./types";
import { SETTINGS, installBackend } from "./test/backend";
import { renderApp } from "./test/harness";

function withPosition(position: SettingsInfo["position"]): SettingsInfo {
  return { ...SETTINGS, position };
}

test("the top-right default mirrors its anchor facets", async () => {
  installBackend();
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("top");
  expect(root.anchorH).toBe("right");
  expect(root.positionMode).toBe("anchored");
});

test("a stored bottom-left anchor lands as bottom/left", async () => {
  installBackend({
    get_settings: () =>
      withPosition({ mode: "anchored", anchor: "bottom-left", locked: false }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("bottom");
  expect(root.anchorH).toBe("left");
});

test("center is its own facet on both axes", async () => {
  installBackend({
    get_settings: () =>
      withPosition({ mode: "anchored", anchor: "center", locked: false }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("center");
  expect(root.anchorH).toBe("center");
});

test("manual mode drops the anchor facets and flags the mode", async () => {
  installBackend({
    get_settings: () =>
      withPosition({ mode: "manual", anchor: "top-right", locked: false }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBeUndefined();
  expect(root.anchorH).toBeUndefined();
  expect(root.positionMode).toBe("manual");
});
