import { describe, expect, test } from "vitest";
import {
  MENU_FIXED_HEIGHT,
  MENU_MIN_HEIGHT,
  menuViewportHeight,
  modelMenuPlacement,
} from "./menuPlacement";

// The cap estimate ModelMenu measures against: opening a menu pins the
// window at the 70% cap, but the OS resize lands async — the estimate keeps
// the first paint correct, and innerHeight wins once the resize has landed.
describe("menuViewportHeight", () => {
  test("a hugged window gets the estimated cap", () => {
    // 1080p: round(1080 × 0.70) + 20 + 44 = 820 — the window.rs twin.
    expect(menuViewportHeight(284, 1080)).toBe(820);
  });

  test("an already-expanded window keeps its real innerHeight", () => {
    expect(menuViewportHeight(820, 1080)).toBe(820);
  });

  test("jsdom's screen.height of 0 degrades to innerHeight", () => {
    // Pins the zero-rect test equilibrium: the DOM suites keep measuring
    // 768 and never see a placement change.
    expect(menuViewportHeight(768, 0)).toBe(768);
  });
});

// Scenario geometry: a 1080p monitor at scale 1 gives the overlay window
// 0.70 × 1080 + 64 = 820px (window.rs), with a top-anchored panel's top at
// the 20px --shadow-room-top. roomBelow = 750 − panelHeight and roomAbove =
// panelHeight − 66, so the flip point sits at panelHeight = 408.
describe("modelMenuPlacement", () => {
  test("idle panel drops below at the full fixed height", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 20, panelHeight: 220 }),
    ).toEqual({ direction: "down", height: MENU_FIXED_HEIGHT });
  });

  test("panel at the 70% cap flips upward, still at the fixed height", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 20, panelHeight: 756 }),
    ).toEqual({ direction: "up", height: MENU_FIXED_HEIGHT });
  });

  test("just past the crossover: up, capped by the room above", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 20, panelHeight: 420 }),
    ).toEqual({ direction: "up", height: 354 });
  });

  test("just before the crossover: down, capped by the room below", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 20, panelHeight: 400 }),
    ).toEqual({ direction: "down", height: 350 });
  });

  test("small monitor caps the drop below the fixed height", () => {
    // 768p monitor: window = 0.70 × 768 + 64 ≈ 602.
    expect(
      modelMenuPlacement({ viewportHeight: 602, panelTop: 20, panelHeight: 160 }),
    ).toEqual({ direction: "down", height: 372 });
  });

  test("floor never overflows downward: a tiny window falls back to up", () => {
    // roomBelow 60 ≥ roomAbove 54, but the 120px floor wouldn't fit below —
    // and .menu--down has no CSS cap, so a floored drop would spill past the
    // window bottom. Up is safe: the .menu max-height clamps the inline
    // height.
    expect(
      modelMenuPlacement({ viewportHeight: 250, panelTop: 20, panelHeight: 120 }),
    ).toEqual({ direction: "up", height: MENU_MIN_HEIGHT });
  });

  test("degenerate room engages the minimum-height floor", () => {
    expect(
      modelMenuPlacement({ viewportHeight: 100, panelTop: 20, panelHeight: 90 }),
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

  // Bottom-anchored panels (data-anchor-v="bottom"): the panel's bottom edge
  // is pinned at viewport − 44, so panelTop rides its height and the free
  // window space sits ABOVE — roomAbove extends past the panel top
  // (menuPlacement's panelTop − APRON_TOP term), matching the viewport cap
  // the CSS override grants.
  test("bottom-anchored idle panel flips up into the free space above", () => {
    // panelTop = 820 − 44 − 220 = 556: roomBelow is −6, roomAbove 690.
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 556, panelHeight: 220 }),
    ).toEqual({ direction: "up", height: MENU_FIXED_HEIGHT });
  });

  test("center-anchored panel still picks the larger side", () => {
    // A 220px panel centered in the 820 window: panelTop = 288 — more room
    // above (422) than below (262), so up, capped by the room above.
    expect(
      modelMenuPlacement({ viewportHeight: 820, panelTop: 288, panelHeight: 220 }),
    ).toEqual({ direction: "up", height: 422 });
  });
});
