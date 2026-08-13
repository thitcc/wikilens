// The instrument layer's pure helpers. The label contract under test is the
// half JS owns: characters only (spaces → underscores), never case — CSS's
// text-transform does the uppercasing, so a double transform is impossible
// and accessible names stay sentence case.

import { expect, test } from "vitest";
import {
  DOT_TOTAL,
  MICRO_PROMPT_PLACEHOLDER,
  instrumentLabel,
  statusDots,
} from "./instrument";
import type { AskStatus } from "./types";

test("instrumentLabel swaps spaces for underscores and nothing else", () => {
  expect(instrumentLabel("Stardew Valley")).toBe("Stardew_Valley");
  expect(instrumentLabel("Claude Sonnet 5")).toBe("Claude_Sonnet_5");
  // Case and punctuation pass through untouched — case is CSS's job.
  expect(instrumentLabel("The Elder Scrolls V: Skyrim")).toBe(
    "The_Elder_Scrolls_V:_Skyrim",
  );
  expect(instrumentLabel("Terraria")).toBe("Terraria");
});

test("the micro placeholder is already in the underscore voice", () => {
  expect(MICRO_PROMPT_PLACEHOLDER).not.toContain(" ");
});

test("dot fills stay strictly inside the row and never move backwards", () => {
  // `satisfies` makes this exhaustive: a future AskStatus member fails the
  // compile here, so the range loop can never silently skip a new phase.
  const ALL_PHASES = {
    searching: true,
    understanding: true,
    retrying: true,
    reading: true,
    answering: true,
  } satisfies Record<AskStatus, true>;
  const phases = Object.keys(ALL_PHASES) as AskStatus[];
  for (const phase of phases) {
    const dots = statusDots(phase);
    // Never 0 (an ask is running) and never full (the row unmounts on
    // resolve — a full row would claim a done that never renders).
    expect(dots, phase).toBeGreaterThanOrEqual(1);
    expect(dots, phase).toBeLessThan(DOT_TOTAL);
  }
  // Monotone along the linear path…
  expect(statusDots("searching")).toBeLessThan(statusDots("reading"));
  expect(statusDots("reading")).toBeLessThan(statusDots("answering"));
  // …and the non-linear phases may never pull the row backwards from the
  // phase they interleave with (they follow `searching`).
  expect(statusDots("understanding")).toBeGreaterThanOrEqual(
    statusDots("searching"),
  );
  expect(statusDots("retrying")).toBeGreaterThanOrEqual(
    statusDots("searching"),
  );
});
