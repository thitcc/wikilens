// Relative-time labels for the history menu — pure and DOM-free like
// modelPick.ts/menuNav.ts so the banding rules are unit-testable. `nowMs`
// is an explicit parameter (never Date.now() internally): tests pass fixed
// clocks, and the component decides how often "now" refreshes.

const MINUTE_MS = 60_000;
const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;
const WEEK_MS = 7 * DAY_MS;

/** English month labels, hardcoded: the app's copy is English throughout, and
 * `toLocaleDateString` would make the label (and the tests) OS-locale
 * dependent. */
const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/** When a history entry landed, relative to `nowMs`: "just now" under a
 * minute, then "Nm ago" / "Nh ago" / "Nd ago", then a plain "Jul 30" past a
 * week (an exact weekday adds nothing at that distance). A `thenMs` in the
 * future (clock skew) reads as "just now" rather than a negative count. */
export function relativeTime(thenMs: number, nowMs: number): string {
  const elapsed = nowMs - thenMs;
  if (elapsed < MINUTE_MS) return "just now";
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)}m ago`;
  if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)}h ago`;
  if (elapsed < WEEK_MS) return `${Math.floor(elapsed / DAY_MS)}d ago`;
  const then = new Date(thenMs);
  return `${MONTHS[then.getMonth()]} ${then.getDate()}`;
}
