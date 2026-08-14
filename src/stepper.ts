/**
 * Pure stepper-cycle math, DOM-free like `menuNav.ts`. Unlike `nextHighlight`,
 * the stepper WRAPS at both ends — a deliberate deviation: the clamp exists so
 * a wrap can't teleport a scroll viewport across a long list, and a two-item
 * value cycle has no viewport to teleport
 * (vault/2026-08-13_stepper-popover-third-altitude.md).
 */
export function cycle(
  options: readonly string[],
  current: string,
  dir: 1 | -1,
): string {
  if (options.length === 0) return current;
  const i = options.indexOf(current);
  // An unknown current snaps to the first option rather than guessing a
  // neighbor — the caller's state is already off the list.
  if (i === -1) return options[0];
  return options[(i + dir + options.length) % options.length];
}
