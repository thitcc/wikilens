import { describe, expect, test } from "vitest";
import {
  MODEL_STORAGE_PREFIX,
  activeModelVision,
  sameModelPick,
  storedModel,
} from "./modelPick";
import { PROVIDERS } from "./test/backend";

const anthropic = PROVIDERS[0];
const deepseek = PROVIDERS[1];

describe("storedModel", () => {
  test("corrupted or wrong-shape storage falls back to null", () => {
    expect(storedModel("")).toBeNull();
    expect(storedModel("anthropic")).toBeNull(); // nothing stored

    localStorage.setItem(MODEL_STORAGE_PREFIX + "anthropic", "{not json");
    expect(storedModel("anthropic")).toBeNull();

    localStorage.setItem(
      MODEL_STORAGE_PREFIX + "anthropic",
      JSON.stringify({ id: 5, label: "nope" }),
    );
    expect(storedModel("anthropic")).toBeNull();

    localStorage.setItem(
      MODEL_STORAGE_PREFIX + "anthropic",
      JSON.stringify(["id", "label"]),
    );
    expect(storedModel("anthropic")).toBeNull();
  });

  test("vision is carried through only when it is a boolean", () => {
    localStorage.setItem(
      MODEL_STORAGE_PREFIX + "anthropic",
      JSON.stringify({ id: "m", label: "M" }),
    );
    expect(storedModel("anthropic")).toEqual({ id: "m", label: "M" });
    expect("vision" in storedModel("anthropic")!).toBe(false);

    localStorage.setItem(
      MODEL_STORAGE_PREFIX + "anthropic",
      JSON.stringify({ id: "m", label: "M", vision: "yes" }),
    );
    expect("vision" in storedModel("anthropic")!).toBe(false);

    localStorage.setItem(
      MODEL_STORAGE_PREFIX + "anthropic",
      JSON.stringify({ id: "m", label: "M", vision: false }),
    );
    expect(storedModel("anthropic")).toEqual({
      id: "m",
      label: "M",
      vision: false,
    });
  });
});

describe("activeModelVision", () => {
  test("resolves deepseek → pick flag → provider default → false, in order", () => {
    // DeepSeek short-circuits to text-only even if a stored flag claims vision.
    expect(activeModelVision(deepseek, { id: "m", label: "M", vision: true })).toBe(
      false,
    );
    // The pick's own boolean beats the provider default.
    expect(
      activeModelVision(anthropic, { id: "m", label: "M", vision: false }),
    ).toBe(false);
    expect(
      activeModelVision(anthropic, { id: "m", label: "M", vision: true }),
    ).toBe(true);
    // No flag on the pick → provider default.
    expect(activeModelVision(anthropic, { id: "m", label: "M" })).toBe(true);
    expect(activeModelVision(anthropic, null)).toBe(true);
    // No provider at all → false.
    expect(activeModelVision(undefined, null)).toBe(false);
  });
});

describe("sameModelPick", () => {
  test("compares by content including the vision flag and its absence", () => {
    expect(sameModelPick(null, null)).toBe(true);
    expect(sameModelPick(null, { id: "m", label: "M" })).toBe(false);
    expect(sameModelPick({ id: "m", label: "M" }, null)).toBe(false);
    expect(
      sameModelPick({ id: "m", label: "M" }, { id: "m", label: "M" }),
    ).toBe(true);
    expect(
      sameModelPick(
        { id: "m", label: "M", vision: true },
        { id: "m", label: "M", vision: true },
      ),
    ).toBe(true);
    // An absent flag is not the same pick as an explicit one.
    expect(
      sameModelPick({ id: "m", label: "M" }, { id: "m", label: "M", vision: false }),
    ).toBe(false);
    expect(
      sameModelPick({ id: "m", label: "M" }, { id: "x", label: "M" }),
    ).toBe(false);
  });
});
