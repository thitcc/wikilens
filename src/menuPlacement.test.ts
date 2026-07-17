import { describe, expect, test } from "vitest";
import {
  MENU_FIXED_HEIGHT,
  MENU_MIN_HEIGHT,
  modelMenuPlacement,
} from "./menuPlacement";

// Scenario geometry: a 1080p monitor at scale 1 gives the overlay window
// 0.70 × 1080 + 56 = 812px (window.rs), with the panel top at the 12px
// --panel-gap. roomBelow = 750 − panelHeight and roomAbove = panelHeight − 66,
// so the flip point sits at panelHeight = 408.
describe("modelMenuPlacement", () => {
  test("idle panel drops below at the full fixed height", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 812, panelTop: 12, panelHeight: 220 }),
    ).toEqual({ direction: "down", height: MENU_FIXED_HEIGHT });
  });

  test("panel at the 70% cap flips upward, still at the fixed height", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 812, panelTop: 12, panelHeight: 756 }),
    ).toEqual({ direction: "up", height: MENU_FIXED_HEIGHT });
  });

  test("just past the crossover: up, capped by the room above", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 812, panelTop: 12, panelHeight: 420 }),
    ).toEqual({ direction: "up", height: 354 });
  });

  test("just before the crossover: down, capped by the room below", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 812, panelTop: 12, panelHeight: 400 }),
    ).toEqual({ direction: "down", height: 350 });
  });

  test("small monitor caps the drop below the fixed height", () => {
    // 768p monitor: window = 0.70 × 768 + 56 ≈ 594.
    expect(
      modelMenuPlacement({ viewportHeight: 594, panelTop: 12, panelHeight: 160 }),
    ).toEqual({ direction: "down", height: 372 });
  });

  test("degenerate room engages the minimum-height floor", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 100, panelTop: 12, panelHeight: 90 }),
    ).toEqual({ direction: "up", height: MENU_MIN_HEIGHT });
  });

  test("jsdom's zero rects resolve to the down/fixed default", () => {
    // getBoundingClientRect is all zeros in jsdom and innerHeight is 768 —
    // this matches ModelMenu's initial state, so the DOM tests never see a
    // placement change.
    expect(
      modelMenuPlacement({ viewportHeight: 768, panelTop: 0, panelHeight: 0 }),
    ).toEqual({ direction: "down", height: MENU_FIXED_HEIGHT });
  });
});
