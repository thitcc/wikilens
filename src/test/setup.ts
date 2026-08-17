// Global test setup (wired via vite.config.ts `test.setupFiles`).

import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { installResizeObserver, resetResizeObservers } from "./resizeObserver";

// RTL only flips this on automatically when test globals are injected; ours
// are off, so React would otherwise warn on every act()-wrapped update.
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

// jsdom has no ResizeObserver; App's window-height reporter needs one.
installResizeObserver();

afterEach(() => {
  // Order is load-bearing: unmounting runs the app's unlisten() cleanups,
  // which call into the mocked __TAURI_EVENT_PLUGIN_INTERNALS__ — clear the
  // mocks first and every unmount becomes a TypeError storm.
  cleanup();
  clearMocks();
  localStorage.clear();
  resetResizeObservers();
  // App writes the theme to the root element (outside React's tree, so
  // cleanup() never touches it) — without this a micrographics test leaks
  // its attribute into the next test in the same file.
  delete document.documentElement.dataset.theme;
  // Same story for the placement attributes App mirrors from settings.
  delete document.documentElement.dataset.anchorV;
  delete document.documentElement.dataset.anchorH;
  delete document.documentElement.dataset.positionMode;
});
