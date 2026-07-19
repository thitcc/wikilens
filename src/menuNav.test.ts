import { describe, expect, test } from "vitest";
import { isNavKey, nextHighlight } from "./menuNav";

describe("nextHighlight", () => {
  test("ArrowDown steps toward the end and clamps there (no wrap)", () => {
    expect(nextHighlight(0, 3, "ArrowDown")).toBe(1);
    expect(nextHighlight(2, 3, "ArrowDown")).toBe(2);
  });

  test("ArrowUp steps toward the start and clamps there (no wrap)", () => {
    expect(nextHighlight(2, 3, "ArrowUp")).toBe(1);
    expect(nextHighlight(0, 3, "ArrowUp")).toBe(0);
  });

  test("entering from no-highlight lands nearest the direction of travel", () => {
    expect(nextHighlight(-1, 3, "ArrowDown")).toBe(0);
    expect(nextHighlight(-1, 3, "ArrowUp")).toBe(2);
  });

  test("Home and End jump to the ends regardless of position", () => {
    expect(nextHighlight(1, 5, "Home")).toBe(0);
    expect(nextHighlight(1, 5, "End")).toBe(4);
    expect(nextHighlight(-1, 5, "Home")).toBe(0);
    expect(nextHighlight(-1, 5, "End")).toBe(4);
  });

  test("an empty list yields no highlight for every key", () => {
    for (const key of ["ArrowDown", "ArrowUp", "Home", "End"] as const) {
      expect(nextHighlight(-1, 0, key)).toBe(-1);
      expect(nextHighlight(3, 0, key)).toBe(-1);
    }
  });

  test("a single row is every key's destination", () => {
    for (const key of ["ArrowDown", "ArrowUp", "Home", "End"] as const) {
      expect(nextHighlight(0, 1, key)).toBe(0);
      expect(nextHighlight(-1, 1, key)).toBe(0);
    }
  });
});

describe("isNavKey", () => {
  test("accepts exactly the four movement keys", () => {
    expect(isNavKey("ArrowDown")).toBe(true);
    expect(isNavKey("ArrowUp")).toBe(true);
    expect(isNavKey("Home")).toBe(true);
    expect(isNavKey("End")).toBe(true);
  });

  test("rejects keys the filter input must keep", () => {
    expect(isNavKey("ArrowLeft")).toBe(false);
    expect(isNavKey("ArrowRight")).toBe(false);
    expect(isNavKey("Enter")).toBe(false);
    expect(isNavKey("Escape")).toBe(false);
    expect(isNavKey("a")).toBe(false);
  });
});
