/** Center a menu row inside its scrolling `.menu-list` — shared by the game
 * and model menus so the coordinate-conversion invariant lives once. Both
 * offsetTops are `.menu`-relative (`.menu-list` is unpositioned, so the
 * rows' offsetParent is the absolutely-positioned `.menu`); the difference
 * converts the row into list coordinates — without it the scroll overshoots
 * by the search-bar height. No-op when the list doesn't scroll. */
export function centerRowInList(
  list: HTMLElement | null,
  row: HTMLElement | null,
): void {
  if (!list || !row || list.scrollHeight <= list.clientHeight) return;
  const rowTopInList = row.offsetTop - list.offsetTop;
  list.scrollTop = Math.max(
    0,
    rowTopInList - list.clientHeight / 2 + row.offsetHeight / 2,
  );
}
