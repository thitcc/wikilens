// Placement mirroring. Ownership under test: App copies the stored placement
// (settings.position) onto the root element as data-anchor-v / data-anchor-h
// / data-position-mode — the data-theme idiom — so CSS can align the panel
// inside a cap-expanded window and pick the entrance motion. Manual carries
// no anchor facets (top-pinned growth from the dragged spot); the top-right
// default carries v="top" h="right" like any other anchor.

import { expect, test } from "vitest";
import type { SettingsInfo } from "./types";
import {
  SETTINGS,
  SETTINGS_POSITION_LOCKED,
  installBackend,
} from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";

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
      withPosition({
        mode: "anchored",
        anchor: "bottom-left",
        locked: false,
        manualEdge: null,
      }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("bottom");
  expect(root.anchorH).toBe("left");
});

test("center is its own facet on both axes", async () => {
  installBackend({
    get_settings: () =>
      withPosition({
        mode: "anchored",
        anchor: "center",
        locked: false,
        manualEdge: null,
      }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("center");
  expect(root.anchorH).toBe("center");
});

test("manual mode drops the anchor facets and flags the mode", async () => {
  installBackend({
    get_settings: () =>
      withPosition({
        mode: "manual",
        anchor: "top-right",
        locked: false,
        manualEdge: "top",
      }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBeUndefined();
  expect(root.anchorH).toBeUndefined();
  expect(root.positionMode).toBe("manual");
});

test("a bottom-pinned manual spot borrows the bottom-anchor facet", async () => {
  // A low drop stores edge=bottom: the menus and the cap-state alignment
  // must flip upward exactly like a bottom anchor (the CSS keys on
  // data-anchor-v) — without it, a menu opened near the screen bottom
  // clips off-screen.
  installBackend({
    get_settings: () =>
      withPosition({
        mode: "manual",
        anchor: "top-right",
        locked: false,
        manualEdge: "bottom",
      }),
  });
  await renderApp();

  const root = document.documentElement.dataset;
  expect(root.anchorV).toBe("bottom");
  expect(root.anchorH).toBeUndefined();
  expect(root.positionMode).toBe("manual");
});

// ---- The drag surface -------------------------------------------------------
// While unlocked, the whole transparent window drags: the header (deep — its
// chips still click), the panel's bare glass (bare attribute — clicks that
// target a child never drag), and the apron band around the panel (bare
// attribute on body). The padlock removes every one of them at the source
// (Rust's Moved snap-back is defense in depth only).

test("the header, the glass, and the apron offer the drag while unlocked", async () => {
  installBackend();
  await renderApp();

  const header = document.querySelector(".panel-header");
  expect(header?.getAttribute("data-tauri-drag-region")).toBe("deep");
  expect(header?.classList.contains("panel-header--draggable")).toBe(true);
  // Bare (not "deep") on the panel and the body: only clicks that target
  // the bare surface itself drag.
  expect(
    document.querySelector(".panel")?.getAttribute("data-tauri-drag-region"),
  ).toBe("");
  expect(document.body.getAttribute("data-tauri-drag-region")).toBe("");
});

test("the padlock removes the whole drag surface", async () => {
  installBackend({ get_settings: () => SETTINGS_POSITION_LOCKED });
  await renderApp();

  const header = document.querySelector(".panel-header");
  expect(header?.hasAttribute("data-tauri-drag-region")).toBe(false);
  expect(header?.classList.contains("panel-header--draggable")).toBe(false);
  expect(
    document.querySelector(".panel")?.hasAttribute("data-tauri-drag-region"),
  ).toBe(false);
  expect(document.body.hasAttribute("data-tauri-drag-region")).toBe(false);
});

// ---- settings://position ----------------------------------------------------
// A drag settles Rust-side and announces the flip; App patches the placement
// in place so the attributes (and the stepper's next open) track it with the
// Settings menu closed.

test("a drag-initiated flip patches the placement in place", async () => {
  installBackend();
  await renderApp();
  expect(document.documentElement.dataset.anchorV).toBe("top");

  await fireBackendEvent("settings://position", {
    mode: "manual",
    anchor: "top-right",
    locked: false,
    manualEdge: "top",
  });

  const root = document.documentElement.dataset;
  expect(root.positionMode).toBe("manual");
  expect(root.anchorV).toBeUndefined();
  expect(root.anchorH).toBeUndefined();
  // The header keeps its drag handle — the flip changed the mode, not the
  // lock.
  expect(
    document
      .querySelector(".panel-header")
      ?.getAttribute("data-tauri-drag-region"),
  ).toBe("deep");
});

test("a low drop's flip carries the bottom facet through the event path", async () => {
  // The live path after a real drag near the screen bottom: the settle
  // persists edge=bottom and the event must flip the menus upward without
  // any refetch.
  installBackend();
  await renderApp();

  await fireBackendEvent("settings://position", {
    mode: "manual",
    anchor: "top-right",
    locked: false,
    manualEdge: "bottom",
  });

  const root = document.documentElement.dataset;
  expect(root.positionMode).toBe("manual");
  expect(root.anchorV).toBe("bottom");
  expect(root.anchorH).toBeUndefined();
});
