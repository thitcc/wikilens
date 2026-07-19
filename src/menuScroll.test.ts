import { describe, expect, test } from "vitest";
import { centerRowInList, keepRowInView } from "./menuScroll";

/** jsdom does no layout, so fabricate the geometry the helpers read. Both
 * offsetTops are `.menu`-relative (the real offsetParent), which is what the
 * `row.offsetTop - list.offsetTop` conversion in the helpers exercises. */
function fakeList(opts: {
  offsetTop: number;
  clientHeight: number;
  scrollHeight: number;
  scrollTop?: number;
}): HTMLElement {
  const list = document.createElement("div");
  Object.defineProperty(list, "offsetTop", { value: opts.offsetTop });
  Object.defineProperty(list, "clientHeight", { value: opts.clientHeight });
  Object.defineProperty(list, "scrollHeight", { value: opts.scrollHeight });
  list.scrollTop = opts.scrollTop ?? 0;
  return list;
}

function fakeRow(opts: { offsetTop: number; offsetHeight: number }): HTMLElement {
  const row = document.createElement("button");
  Object.defineProperty(row, "offsetTop", { value: opts.offsetTop });
  Object.defineProperty(row, "offsetHeight", { value: opts.offsetHeight });
  return row;
}

describe("centerRowInList", () => {
  test("centers the row, converting menu coordinates to list coordinates", () => {
    const list = fakeList({ offsetTop: 48, clientHeight: 200, scrollHeight: 1000 });
    const row = fakeRow({ offsetTop: 548, offsetHeight: 30 });
    centerRowInList(list, row);
    // rowTopInList = 548 - 48 = 500; 500 - 100 + 15 = 415.
    expect(list.scrollTop).toBe(415);
  });

  test("clamps at the top for a row near the start", () => {
    const list = fakeList({ offsetTop: 48, clientHeight: 200, scrollHeight: 1000 });
    const row = fakeRow({ offsetTop: 60, offsetHeight: 30 });
    centerRowInList(list, row);
    expect(list.scrollTop).toBe(0);
  });

  test("no-ops when the list does not scroll", () => {
    const list = fakeList({
      offsetTop: 48,
      clientHeight: 200,
      scrollHeight: 200,
      scrollTop: 0,
    });
    const row = fakeRow({ offsetTop: 148, offsetHeight: 30 });
    centerRowInList(list, row);
    expect(list.scrollTop).toBe(0);
  });

  test("no-ops on null list or row", () => {
    const list = fakeList({ offsetTop: 0, clientHeight: 100, scrollHeight: 500 });
    expect(() => centerRowInList(null, fakeRow({ offsetTop: 0, offsetHeight: 30 }))).not.toThrow();
    expect(() => centerRowInList(list, null)).not.toThrow();
    expect(list.scrollTop).toBe(0);
  });
});

describe("keepRowInView", () => {
  test("row above the viewport aligns to the top edge", () => {
    const list = fakeList({
      offsetTop: 48,
      clientHeight: 200,
      scrollHeight: 1000,
      scrollTop: 300,
    });
    // rowTop in list coords = 148 - 48 = 100 < scrollTop 300.
    const row = fakeRow({ offsetTop: 148, offsetHeight: 30 });
    keepRowInView(list, row);
    expect(list.scrollTop).toBe(100);
  });

  test("row below the viewport aligns to the bottom edge", () => {
    const list = fakeList({
      offsetTop: 48,
      clientHeight: 200,
      scrollHeight: 1000,
      scrollTop: 0,
    });
    // rowBottom = (548 - 48) + 30 = 530 > 0 + 200 → scrollTop = 530 - 200.
    const row = fakeRow({ offsetTop: 548, offsetHeight: 30 });
    keepRowInView(list, row);
    expect(list.scrollTop).toBe(330);
  });

  test("row already in view scrolls nothing (minimal-scroll pin)", () => {
    const list = fakeList({
      offsetTop: 48,
      clientHeight: 200,
      scrollHeight: 1000,
      scrollTop: 80,
    });
    // rowTop 100, rowBottom 130 — inside [80, 280].
    const row = fakeRow({ offsetTop: 148, offsetHeight: 30 });
    keepRowInView(list, row);
    expect(list.scrollTop).toBe(80);
  });

  test("no-ops when the list does not scroll", () => {
    const list = fakeList({
      offsetTop: 48,
      clientHeight: 200,
      scrollHeight: 200,
      scrollTop: 0,
    });
    const row = fakeRow({ offsetTop: 548, offsetHeight: 30 });
    keepRowInView(list, row);
    expect(list.scrollTop).toBe(0);
  });

  test("no-ops on null list or row", () => {
    const list = fakeList({ offsetTop: 0, clientHeight: 100, scrollHeight: 500 });
    expect(() => keepRowInView(null, fakeRow({ offsetTop: 0, offsetHeight: 30 }))).not.toThrow();
    expect(() => keepRowInView(list, null)).not.toThrow();
    expect(list.scrollTop).toBe(0);
  });
});
