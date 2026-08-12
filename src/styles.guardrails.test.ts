// Theme guardrails — the CSS analogue of src-tauri's config_guardrails.rs:
// parse the shipped stylesheet and pin the theme-invariant token set that
// otherwise exists only as prose (DESIGN.md §7 "express a new theme as token
// overrides only" / §8's invariant list, and the theme plan's step 1). The
// invariants guard the paired-constant twins: --menu-clearance* mirrors
// menuPlacement.ts, --panel-gap / --shadow-room-* mirror window.rs, and the
// type metrics feed the rendered heights those constants were derived from.
//
// Mechanism: assertions run over the raw stylesheet text read from disk —
// jsdom returns "" for custom properties (documented in menuPlacement.ts), so
// computed styles can't check this, and Vitest's CSS handling intercepts
// `.css` imports before Vite's `?raw` query, yielding "" (verified — that
// path is a trap). Rules are split naively on braces; that mis-parses @media
// wrappers but matches every inner and top-level rule, which is all this
// needs.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "vitest";

// Vitest runs with cwd at the repo root (vite.config.ts's home) — the
// frontend analogue of config_guardrails.rs's CARGO_MANIFEST_DIR anchor.
// (`new URL(..., import.meta.url)` is not usable here: under the jsdom
// environment import.meta.url is not a file: URL.)
const css = readFileSync(join(process.cwd(), "src", "styles.css"), "utf8");

/** Token NAMES a theme block may never redeclare. Values inside a theme
 * block may *use* them (`var(--space-6)` is fine); a declaration is the
 * violation. The rhythm scale is here because geometry aliases into it —
 * `--panel-gap: var(--space-12)` — so redefining a step would move the float
 * geometry transitively. */
const INVARIANT_TOKENS = [
  // Rhythm (aliased into geometry)
  "--space-2",
  "--space-4",
  "--space-6",
  "--space-8",
  "--space-10",
  "--space-12",
  "--space-14",
  "--space-20",
  "--pad-chip",
  "--pad-control",
  "--pad-box",
  // Float geometry (window.rs twins)
  "--panel-gap",
  "--shadow-room-left",
  "--shadow-room-bottom",
  // Menu geometry (menuPlacement.ts twin)
  "--menu-clearance",
  "--menu-clearance-top",
  // Type metrics (feed the rendered heights the clearances derive from)
  "--text-base",
  "--text-status",
  "--text-code",
  "--text-label",
  "--leading-compact",
  "--leading-body",
  "--leading-answer",
  // Material and states
  "--blur-panel",
  "--opacity-disabled",
  "--font-mono",
];

interface Rule {
  selector: string;
  body: string;
}

/** Every `selector { body }` pair in the sheet. Nested @media wrappers don't
 * match as a whole (their body has braces), but each rule inside them does. */
function rules(): Rule[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
    selector: (m[1] ?? "").trim(),
    body: m[2] ?? "",
  }));
}

/** A declaration of `name` (not a var() usage): start-of-block or after a
 * semicolon, then the exact name, then a colon. `--space-2` must not match
 * `--space-20:` — the colon anchor guarantees it. */
function declares(body: string, name: string): boolean {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(^|[{;\\s])${escaped}\\s*:`).test(body);
}

const themeRules = () =>
  rules().filter((r) => r.selector.includes("[data-theme"));

const microTokenBlock = () =>
  rules().find((r) => r.selector.endsWith('[data-theme="micrographics"]'));

const rootBlock = () => rules().find((r) => r.selector.endsWith(":root"));

test("the sheet still has the shapes this suite parses (vacuity guard)", () => {
  // A refactor that renames the blocks must fail HERE, loudly — not let the
  // invariant assertions below pass against nothing.
  const root = rootBlock();
  expect(root).toBeDefined();
  expect(
    [...(root?.body ?? "").matchAll(/--[\w-]+\s*:/g)].length,
  ).toBeGreaterThan(30);

  const micro = microTokenBlock();
  expect(micro).toBeDefined();
  const declarations = [...(micro?.body ?? "").matchAll(/--[\w-]+\s*:/g)];
  expect(declarations.length).toBeGreaterThan(10);
  // The sentinel override: the theme's loudest move must be present, or this
  // block isn't the micrographics token block at all.
  expect(/--radius-panel\s*:\s*0px/.test(micro?.body ?? "")).toBe(true);

  expect(themeRules().length).toBeGreaterThan(0);
});

test("every invariant token is still a real :root token", () => {
  // Guards the guard: a token renamed in :root would otherwise silently drop
  // out of coverage while its old name "passes" the absence check forever.
  const root = rootBlock();
  expect(root).toBeDefined();
  for (const name of INVARIANT_TOKENS) {
    expect(declares(root?.body ?? "", name), `${name} missing from :root`).toBe(
      true,
    );
  }
});

test("no theme block redeclares an invariant token", () => {
  const scoped = themeRules();
  expect(scoped.length).toBeGreaterThan(0);
  for (const rule of scoped) {
    for (const name of INVARIANT_TOKENS) {
      expect(
        declares(rule.body, name),
        `${name} redeclared under "${rule.selector}"`,
      ).toBe(false);
    }
  }
});
