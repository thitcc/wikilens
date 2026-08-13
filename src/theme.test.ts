// storedTheme()'s corruption tolerance (the modelPick.test.ts pattern):
// whatever localStorage holds, the reader answers with a member of the closed
// set — and unlike the game/model readers there is no backend list to
// reconcile against, so the default comes from the reader itself.

import { expect, test } from "vitest";
import { THEME_STORAGE_KEY, storedTheme } from "./theme";

test("unset storage reads as the default theme", () => {
  expect(storedTheme()).toBe("default");
});

test("both members of the closed set read back verbatim", () => {
  localStorage.setItem(THEME_STORAGE_KEY, "default");
  expect(storedTheme()).toBe("default");
  localStorage.setItem(THEME_STORAGE_KEY, "micrographics");
  expect(storedTheme()).toBe("micrographics");
});

test("garbage shapes all fall back to the default", () => {
  const garbage = [
    "",
    "neon",
    "MICROGRAPHICS",
    " micrographics",
    '{"theme":"micrographics"}',
    "{definitely not json",
  ];
  for (const value of garbage) {
    localStorage.setItem(THEME_STORAGE_KEY, value);
    expect(storedTheme(), `for ${JSON.stringify(value)}`).toBe("default");
  }
});
