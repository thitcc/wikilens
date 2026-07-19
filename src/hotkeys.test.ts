import { describe, expect, test } from "vitest";
import {
  comboFromEvent,
  labelParts,
  pendingModLabels,
  sameCombo,
  toAccelerator,
  validateCombo,
  type ComboParts,
} from "./hotkeys";

function combo(over: Partial<ComboParts> & { code: string }): ComboParts {
  return { ctrl: false, alt: false, shift: false, meta: false, ...over };
}

describe("comboFromEvent", () => {
  test("bare modifier keydowns keep the recorder waiting", () => {
    for (const code of ["ControlLeft", "ShiftRight", "AltLeft", "MetaRight"]) {
      expect(
        comboFromEvent({
          code,
          ctrlKey: true,
          altKey: false,
          shiftKey: false,
          metaKey: false,
        }),
      ).toBeNull();
    }
  });

  test("a main key snapshots the held modifiers", () => {
    expect(
      comboFromEvent({
        code: "KeyP",
        ctrlKey: true,
        altKey: true,
        shiftKey: false,
        metaKey: false,
      }),
    ).toEqual(combo({ code: "KeyP", ctrl: true, alt: true }));
  });

  test("unknown main keys pass through for validateCombo to reject", () => {
    expect(
      comboFromEvent({
        code: "ArrowUp",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toEqual(combo({ code: "ArrowUp", ctrl: true }));
  });
});

describe("toAccelerator", () => {
  test("canonical Ctrl+Alt+Shift+Super order, then the code name", () => {
    expect(
      toAccelerator(
        combo({ code: "KeyP", shift: true, ctrl: true, alt: true }),
      ),
    ).toBe("Ctrl+Alt+Shift+KeyP");
    expect(toAccelerator(combo({ code: "Backquote", ctrl: true }))).toBe(
      "Ctrl+Backquote",
    );
    expect(toAccelerator(combo({ code: "F8" }))).toBe("F8");
  });
});

describe("validateCombo", () => {
  test("the shipped defaults are valid", () => {
    expect(
      validateCombo(combo({ code: "Backquote", ctrl: true })),
    ).toEqual({ ok: true });
    expect(
      validateCombo(combo({ code: "KeyC", ctrl: true, shift: true })),
    ).toEqual({ ok: true });
  });

  test("bare keys need Ctrl or Alt", () => {
    const verdict = validateCombo(combo({ code: "KeyC" }));
    expect(verdict.ok).toBe(false);
    if (!verdict.ok) {
      expect(verdict.reason).toBe("Add Ctrl or Alt to make this a shortcut.");
    }
  });

  test("Shift-only combos are refused with the trap rationale", () => {
    // The old Shift+C default swallowed capital C system-wide.
    const verdict = validateCombo(combo({ code: "KeyC", shift: true }));
    expect(verdict.ok).toBe(false);
    if (!verdict.ok) {
      expect(verdict.reason).toMatch(/block typing/);
    }
  });

  test("F-keys are fine bare", () => {
    expect(validateCombo(combo({ code: "F8" }))).toEqual({ ok: true });
  });

  test("Win combos are refused", () => {
    const verdict = validateCombo(combo({ code: "KeyK", meta: true }));
    expect(verdict.ok).toBe(false);
    if (!verdict.ok) {
      expect(verdict.reason).toMatch(/Win/);
    }
  });

  test("non-shortcut keys are refused outright", () => {
    for (const code of ["ArrowUp", "Escape", "Enter", "Space", "F13"]) {
      const verdict = validateCombo(combo({ code, ctrl: true }));
      expect(verdict.ok).toBe(false);
      if (!verdict.ok) {
        expect(verdict.reason).toBe("That key can't be a shortcut here.");
      }
    }
  });
});

describe("labelParts", () => {
  test("maps codes to player glyphs", () => {
    expect(labelParts("Ctrl+Backquote")).toEqual(["Ctrl", "`"]);
    expect(labelParts("Ctrl+Shift+KeyC")).toEqual(["Ctrl", "Shift", "C"]);
    expect(labelParts("Alt+Digit1")).toEqual(["Alt", "1"]);
    expect(labelParts("F8")).toEqual(["F8"]);
    expect(labelParts("Super+KeyK")).toEqual(["Win", "K"]);
  });

  test("tolerates Rust Display casing", () => {
    expect(labelParts("control+shift+KeyC")).toEqual(["Ctrl", "Shift", "C"]);
  });
});

describe("sameCombo", () => {
  test("casing and modifier order don't matter", () => {
    expect(sameCombo("control+Backquote", "Ctrl+Backquote")).toBe(true);
    expect(sameCombo("Shift+Ctrl+KeyC", "Ctrl+Shift+KeyC")).toBe(true);
  });

  test("different combos differ", () => {
    expect(sameCombo("Ctrl+Backquote", "Ctrl+Shift+Backquote")).toBe(false);
    expect(sameCombo("Ctrl+KeyC", "Ctrl+KeyD")).toBe(false);
  });
});

describe("pendingModLabels", () => {
  test("lists held modifiers in display order", () => {
    expect(
      pendingModLabels({
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
        metaKey: false,
      }),
    ).toEqual(["Ctrl", "Shift"]);
  });
});
