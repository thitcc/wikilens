// Pure hotkey helpers for the settings recorder: KeyboardEvent → canonical
// accelerator string, combo validation (player-language copy lives here), and
// accelerator → <kbd> label parts. Pure functions, no DOM — unit-tested like
// menuPlacement.ts.
//
// The canonical accelerator form (modifiers in Ctrl+Alt+Shift+Super order,
// then the KeyboardEvent.code name) and the glyph table are paired with
// `hotkey::to_accelerator` / `hotkey::display_label` in
// src-tauri/src/hotkey.rs — keep the two sides in sync.

/** A pressed combo, extracted from a keydown event. */
export interface ComboParts {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  /** The main key's KeyboardEvent.code, e.g. "Backquote", "KeyC". */
  code: string;
}

/** What the recorder needs from a KeyboardEvent. */
export interface ComboEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/** Render fallbacks while `get_settings` hasn't resolved (or failed) —
 * mirror the Rust defaults' display labels. */
export const DEFAULT_SUMMON_LABEL = "Ctrl+`";
export const DEFAULT_CAPTURE_LABEL = "Ctrl+Shift+C";

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/** Whether a keydown is a bare modifier (still composing the combo). */
export function isModifierCode(code: string): boolean {
  return MODIFIER_CODES.has(code);
}

/** The main-key + modifier state of a keydown, or `null` while only
 * modifiers are down (keep recording). Validation is `validateCombo`'s job —
 * unknown main keys pass through so the recorder can show its hint. */
export function comboFromEvent(e: ComboEvent): ComboParts | null {
  if (isModifierCode(e.code)) {
    return null;
  }
  return {
    ctrl: e.ctrlKey,
    alt: e.altKey,
    shift: e.shiftKey,
    meta: e.metaKey,
    code: e.code,
  };
}

/** The held modifiers as label chips ("Ctrl", "Alt", "Shift", "Win") — the
 * recorder's live preview while the combo is being composed. */
export function pendingModLabels(e: Omit<ComboEvent, "code">): string[] {
  const labels: string[] = [];
  if (e.ctrlKey) labels.push("Ctrl");
  if (e.altKey) labels.push("Alt");
  if (e.shiftKey) labels.push("Shift");
  if (e.metaKey) labels.push("Win");
  return labels;
}

// Punctuation-row glyphs — paired with `key_glyph` in src-tauri/src/hotkey.rs.
const PUNCTUATION_GLYPHS: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

const LETTER = /^Key[A-Z]$/;
const DIGIT = /^Digit[0-9]$/;
const F_KEY = /^F([1-9]|1[0-2])$/;

/** Whether a code is a main key the recorder accepts at all. */
function isAllowedCode(code: string): boolean {
  return (
    LETTER.test(code) ||
    DIGIT.test(code) ||
    F_KEY.test(code) ||
    code in PUNCTUATION_GLYPHS
  );
}

export type ComboVerdict = { ok: true } | { ok: false; reason: string };

/** Whether a combo may become a global shortcut. The reasons are complete
 * player-facing copy, shown as the armed row's hint. */
export function validateCombo(parts: ComboParts): ComboVerdict {
  if (!isAllowedCode(parts.code)) {
    return { ok: false, reason: "That key can't be a shortcut here." };
  }
  if (parts.meta) {
    return {
      ok: false,
      reason: "Windows reserves most Win shortcuts — use Ctrl or Alt instead.",
    };
  }
  // F-keys are fine bare or with any (non-Win) modifiers.
  if (F_KEY.test(parts.code)) {
    return { ok: true };
  }
  if (!parts.ctrl && !parts.alt) {
    if (parts.shift) {
      // The old Shift+C default swallowed capital C system-wide — never
      // let a Shift-only combo recreate that trap.
      return {
        ok: false,
        reason:
          "Shift plus a key would block typing it everywhere — add Ctrl or Alt.",
      };
    }
    return { ok: false, reason: "Add Ctrl or Alt to make this a shortcut." };
  }
  return { ok: true };
}

/** The canonical accelerator string for a combo — byte-identical to what
 * Rust's `hotkey::to_accelerator` produces for the same combo, so equality
 * checks are plain string compares. */
export function toAccelerator(parts: ComboParts): string {
  const out: string[] = [];
  if (parts.ctrl) out.push("Ctrl");
  if (parts.alt) out.push("Alt");
  if (parts.shift) out.push("Shift");
  if (parts.meta) out.push("Super");
  out.push(parts.code);
  return out.join("+");
}

const MOD_ALIASES: Record<string, string> = {
  ctrl: "ctrl",
  control: "ctrl",
  cmdorctrl: "ctrl",
  commandorcontrol: "ctrl",
  alt: "alt",
  option: "alt",
  shift: "shift",
  super: "super",
  cmd: "super",
  command: "super",
  meta: "super",
};

const MOD_LABELS: Record<string, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
  shift: "Shift",
  super: "Win",
};

/** A code token's display glyph: "KeyC" → "C", "Digit1" → "1", "Backquote"
 * → "`", "F8" → "F8"; unmapped tokens fall back verbatim. */
function keyGlyph(token: string): string {
  if (LETTER.test(token)) return token.slice(3);
  if (DIGIT.test(token)) return token.slice(5);
  return PUNCTUATION_GLYPHS[token] ?? token;
}

/** An accelerator string as <kbd> chip labels: "Ctrl+Backquote" →
 * ["Ctrl", "`"]. Tolerant of Rust `Display` casing ("control+Backquote"). */
export function labelParts(accelerator: string): string[] {
  const tokens = accelerator.split("+").filter((t) => t.length > 0);
  const parts: string[] = [];
  tokens.forEach((token, i) => {
    const mod = MOD_ALIASES[token.toLowerCase()];
    // Only leading tokens are modifiers — the final token is the main key
    // even when it collides with a modifier name.
    if (mod !== undefined && i < tokens.length - 1) {
      parts.push(MOD_LABELS[mod]);
    } else {
      parts.push(keyGlyph(token));
    }
  });
  return parts;
}

/** Whether two accelerator strings mean the same combo, regardless of casing
 * and modifier order ("control+Backquote" ≡ "Ctrl+Backquote"). */
export function sameCombo(a: string, b: string): boolean {
  return canonicalKey(a) === canonicalKey(b);
}

function canonicalKey(accelerator: string): string {
  const tokens = accelerator.split("+").filter((t) => t.length > 0);
  const mods = new Set<string>();
  let key = "";
  tokens.forEach((token, i) => {
    const mod = MOD_ALIASES[token.toLowerCase()];
    if (mod !== undefined && i < tokens.length - 1) {
      mods.add(mod);
    } else {
      key = token.toLowerCase();
    }
  });
  return [...mods].sort().join("+") + "::" + key;
}
