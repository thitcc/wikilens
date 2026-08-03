// The window-height reporter: App measures the panel and reports it through
// set_overlay_height so the Rust side can size the overlay window to the
// glass (the transparent remainder of an oversized window eats the game's
// clicks). Pinned here: the mount report, the menu-open sentinel (menus
// assume the 70% cap and Rust owns that clamp), the return to measured on
// close, and the ResizeObserver → throttled report path with its ±1px
// dedupe. jsdom rects are all zeros, so the measured reports are 0 unless a
// test stubs the geometry — Rust's MIN_PANEL_HEIGHT floor absorbs that in
// production too.

import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { PANEL_HEIGHT_UNBOUNDED } from "./api";
import { installBackend } from "./test/backend";
import { renderApp } from "./test/harness";
import { fireResizeObservers } from "./test/resizeObserver";

function heights(backend: { callsTo: (cmd: string) => unknown[] }): number[] {
  return (backend.callsTo("set_overlay_height") as { height: number }[]).map(
    (c) => c.height,
  );
}

function lastHeight(backend: { callsTo: (cmd: string) => unknown[] }): number {
  const all = heights(backend);
  return all[all.length - 1];
}

test("mount reports the measured panel height once", async () => {
  const backend = installBackend();
  await renderApp();

  expect(heights(backend)).toEqual([0]);
});

test("opening a menu reports the unbounded sentinel; closing returns to measured", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  await renderApp();

  await user.click(screen.getByRole("button", { name: /^Model: / }));
  await vi.waitFor(() => {
    expect(lastHeight(backend)).toBe(PANEL_HEIGHT_UNBOUNDED);
  });

  await user.keyboard("{Escape}");
  await vi.waitFor(() => {
    expect(lastHeight(backend)).toBe(0);
  });
});

test("content growth flows through the observer, throttled and deduped", async () => {
  const backend = installBackend();
  await renderApp();
  const before = heights(backend).length;

  // Simulate a grown panel with a scrolling content region: 300px border box
  // plus 100px of scrollback that only the sizer wrapper can reveal.
  const panel = document.querySelector(".panel") as HTMLElement;
  const content = document.querySelector(".content") as HTMLElement;
  panel.getBoundingClientRect = () => ({ height: 300 }) as DOMRect;
  Object.defineProperty(content, "scrollHeight", { value: 400, configurable: true });
  Object.defineProperty(content, "clientHeight", { value: 300, configurable: true });

  fireResizeObservers();
  await vi.waitFor(() => {
    expect(lastHeight(backend)).toBe(400);
  });

  // Same geometry again: within the ±1px tolerance — no new IPC.
  fireResizeObservers();
  await new Promise((resolve) => setTimeout(resolve, 180));
  expect(heights(backend).length).toBe(before + 1);

  // The tolerance boundary itself: a 1px move is DPI echo (still no IPC),
  // a 3px move is real growth (reports).
  Object.defineProperty(content, "scrollHeight", { value: 401, configurable: true });
  fireResizeObservers();
  await new Promise((resolve) => setTimeout(resolve, 180));
  expect(heights(backend).length).toBe(before + 1);

  Object.defineProperty(content, "scrollHeight", { value: 403, configurable: true });
  fireResizeObservers();
  await vi.waitFor(() => {
    expect(lastHeight(backend)).toBe(403);
  });
});
