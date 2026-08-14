import { describe, expect, it } from "vitest";
import { cycle } from "./stepper";

const THREE = ["a", "b", "c"];

describe("cycle", () => {
  it("steps to the neighbor inside the list", () => {
    expect(cycle(THREE, "a", 1)).toBe("b");
    expect(cycle(THREE, "b", -1)).toBe("a");
  });

  it("wraps forward off the last option", () => {
    expect(cycle(THREE, "c", 1)).toBe("a");
  });

  it("wraps backward off the first option", () => {
    expect(cycle(THREE, "a", -1)).toBe("c");
  });

  it("returns the lone option unchanged", () => {
    expect(cycle(["custom"], "custom", 1)).toBe("custom");
    expect(cycle(["custom"], "custom", -1)).toBe("custom");
  });

  it("returns the current id when there are no options", () => {
    expect(cycle([], "custom", 1)).toBe("custom");
  });

  it("snaps an unknown current to the first option", () => {
    expect(cycle(THREE, "banana", 1)).toBe("a");
    expect(cycle(THREE, "banana", -1)).toBe("a");
  });
});
