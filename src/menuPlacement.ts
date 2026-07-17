// Model-menu placement — pure math shared by ModelMenu and the test suite.
// Measured once per open (menus remount every open and `overlay://shown`
// closes them, so geometry is never stale across shows); the panel growing
// under an open menu while an answer streams is accepted — the next open
// corrects.
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
const SHADOW_APRON = 44; // --shadow-room-bottom / window.rs SHADOW_ROOM_BOTTOM
const MENU_CLEARANCE = 52; // --menu-clearance (upward anchor above the footer)
const PANEL_PAD = 14; // --space-14 (top inset of an upward menu in the panel)

export interface MenuPlacement {
  direction: "down" | "up";
  height: number;
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
  /** `.panel` rect top — the 12px `--panel-gap` in practice. */
  panelTop: number;
  /** `.panel` rect height (content-hugging). */
  panelHeight: number;
  /** `window.innerHeight` — the window always holds the 70% cap. */
  viewportHeight: number;
}): MenuPlacement {
  const panelBottom = input.panelTop + input.panelHeight;
  const roomBelow = input.viewportHeight - SHADOW_APRON - panelBottom - DROP_GAP;
  const roomAbove = input.panelHeight - MENU_CLEARANCE - PANEL_PAD;
  const direction =
    roomBelow >= roomAbove && roomBelow >= MENU_MIN_HEIGHT ? "down" : "up";
  const room = direction === "down" ? roomBelow : roomAbove;
  return {
    direction,
    height: Math.max(Math.min(MENU_FIXED_HEIGHT, room), MENU_MIN_HEIGHT),
  };
}
