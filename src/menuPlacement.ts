// Model-menu placement — pure math shared by ModelMenu and the test suite.
// Measured on open and again on window `resize` while open: opening a menu
// pins the window at the 70% cap (set_overlay_height's sentinel), but that
// OS resize lands async — menuViewportHeight() estimates the cap for a
// correct first paint, and the resize re-measure is the authoritative
// correction. The panel growing under an open menu while an answer streams
// still doesn't re-measure (the window is pinned at the cap while open, so
// no resize fires) — the next open corrects.
//
// The constants are paired with their CSS/window.rs twins by comment, the
// same convention window.rs uses for the float geometry. They can't be read
// from the stylesheet at runtime: getComputedStyle returns "" for custom
// properties in jsdom, which would make the helper untestable.

/** Design height of the menu card, both directions. JS-owned — no CSS twin;
 * applied as an inline style so the CSS caps never fight it. */
export const MENU_FIXED_HEIGHT = 460;
/** Floor for degenerate measurements — a squashed menu beats an invisible
 * one, and the outside-click/Esc contracts still work at any size. */
export const MENU_MIN_HEIGHT = 120;

const DROP_GAP = 6; // .menu--down `top: calc(100% + var(--space-6))`
const SHADOW_APRON = 44; // --shadow-room-bottom / window.rs APRON_BOTTOM
const APRON_TOP = 20; // --shadow-room-top / window.rs APRON_TOP
const MENU_CLEARANCE = 52; // --menu-clearance (upward anchor above the footer)
const PANEL_PAD = 14; // --space-14 (top inset of an upward menu in the panel)

export interface MenuPlacement {
  direction: "down" | "up";
  height: number;
}

/** The viewport a menu will actually get: App pins the window at the 70% cap
 * whenever a menu is open, but the resize lands async — estimate the cap so
 * the first paint doesn't measure the still-hugged window. PAIRED CONSTANTS:
 * 0.70 = window.rs PANEL_HEIGHT_FRAC, 12 = PANEL_GAP. `screen.height` is
 * logical CSS px on Windows; 0 (jsdom) degrades to `innerHeight`, preserving
 * the zero-rect test equilibrium. */
export function menuViewportHeight(
  innerHeight: number,
  screenHeight: number,
): number {
  return Math.max(
    innerHeight,
    Math.round(screenHeight * 0.7) + APRON_TOP + SHADOW_APRON,
  );
}

/** Where the model menu opens and how tall it is. Prefers dropping below the
 * panel's bottom edge into the free window space (the window always holds
 * the 70% cap while the panel hugs its content); when a tall panel — a long
 * streamed answer — leaves more room above than below, it flips to the
 * upward `--menu-clearance` anchoring instead. Picking the larger side is
 * monotonic in panel height (the two rooms sum to a per-monitor constant),
 * so there is exactly one flip point — plus a safety flip: `.menu--down`
 * has no CSS cap (`max-height: none`), so a floored height must never go
 * below; when even the floor can't fit under the panel (sub-~370px logical
 * windows), up wins regardless, where the base `.menu` panel-relative
 * `max-height` clamps the inline height inside the viewport. */
export function modelMenuPlacement(input: {
  /** `.panel` rect top — the 20px `--shadow-room-top` for top anchors;
   * larger when a bottom/center anchor floats the panel down the window. */
  panelTop: number;
  /** `.panel` rect height (content-hugging). */
  panelHeight: number;
  /** The cap-expanded viewport — pass `menuViewportHeight(...)`, not a bare
   * `window.innerHeight` (the window may still be hugging the panel). */
  viewportHeight: number;
}): MenuPlacement {
  const panelBottom = input.panelTop + input.panelHeight;
  const roomBelow = input.viewportHeight - SHADOW_APRON - panelBottom - DROP_GAP;
  // Room above the upward anchor (--menu-clearance over the panel bottom) up
  // to the window's top apron. For a top-anchored panel (panelTop ==
  // APRON_TOP) this reduces to the old panel-interior formula; a
  // bottom-anchored panel adds the free window space above it, matching the
  // viewport cap :root[data-anchor-v="bottom"] .menu gets in styles.css.
  const roomAbove =
    input.panelTop - APRON_TOP + input.panelHeight - MENU_CLEARANCE - PANEL_PAD;
  const direction =
    roomBelow >= roomAbove && roomBelow >= MENU_MIN_HEIGHT ? "down" : "up";
  const room = direction === "down" ? roomBelow : roomAbove;
  return {
    direction,
    height: Math.max(Math.min(MENU_FIXED_HEIGHT, room), MENU_MIN_HEIGHT),
  };
}
