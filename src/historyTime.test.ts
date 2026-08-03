import { expect, test } from "vitest";
import { relativeTime } from "./historyTime";

// A fixed "now" — every band derives from explicit offsets against it, so the
// suite never touches the real clock.
const NOW = 1_800_000_000_000;

const MINUTE = 60_000;
const HOUR = 3_600_000;
const DAY = 86_400_000;

test("under a minute is 'just now'", () => {
  expect(relativeTime(NOW, NOW)).toBe("just now");
  expect(relativeTime(NOW - (MINUTE - 1), NOW)).toBe("just now");
});

test("a future timestamp (clock skew) reads as 'just now', never negative", () => {
  expect(relativeTime(NOW + 5 * MINUTE, NOW)).toBe("just now");
});

test("minutes band: floor of elapsed minutes", () => {
  expect(relativeTime(NOW - MINUTE, NOW)).toBe("1m ago");
  expect(relativeTime(NOW - (HOUR - 1), NOW)).toBe("59m ago");
});

test("hours band: floor of elapsed hours", () => {
  expect(relativeTime(NOW - HOUR, NOW)).toBe("1h ago");
  expect(relativeTime(NOW - (DAY - 1), NOW)).toBe("23h ago");
});

test("days band up to a week: floor of elapsed days", () => {
  expect(relativeTime(NOW - DAY, NOW)).toBe("1d ago");
  expect(relativeTime(NOW - (7 * DAY - 1), NOW)).toBe("6d ago");
});

test("past a week: a plain month-day from the hardcoded English months", () => {
  // Built via the local-time Date constructor so the expected label can't
  // shift with the machine's timezone (relativeTime reads local fields too).
  const then = new Date(2026, 6, 15, 12).getTime();
  expect(relativeTime(then, then + 30 * DAY)).toBe("Jul 15");
  const december = new Date(2026, 11, 3, 12).getTime();
  expect(relativeTime(december, december + 8 * DAY)).toBe("Dec 3");
});

test("the week boundary itself tips into the month-day form", () => {
  const then = new Date(2026, 6, 15, 12).getTime();
  expect(relativeTime(then, then + 7 * DAY)).toBe("Jul 15");
});
