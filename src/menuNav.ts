/** Pure keyboard-highlight movement for the owned menus (game + model), paired
 * with the components' derived visible-row arrays. Kept DOM-free like
 * `modelPick.ts`/`hotkeys.ts` so the movement rules are unit-testable. */

export type NavKey = "ArrowDown" | "ArrowUp" | "Home" | "End";

export function isNavKey(key: string): key is NavKey {
  return (
    key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End"
  );
}

/**
 * Next highlight index for a nav key. `current` is the effective index
 * (-1 = no highlight, e.g. an empty filtered list or a vanished row).
 *
 * Clamps at both ends instead of wrapping: a wrap teleports the scroll
 * viewport across OpenRouter's 300+ rows, and Home/End already cover the
 * jumps. Entering from -1, ArrowDown starts at the top and ArrowUp at the
 * bottom — the row nearest the direction of travel.
 */
export function nextHighlight(
  current: number,
  count: number,
  key: NavKey,
): number {
  if (count <= 0) return -1;
  switch (key) {
    case "ArrowDown":
      return current < 0 ? 0 : Math.min(current + 1, count - 1);
    case "ArrowUp":
      return current < 0 ? count - 1 : Math.max(current - 1, 0);
    case "Home":
      return 0;
    case "End":
      return count - 1;
  }
}
