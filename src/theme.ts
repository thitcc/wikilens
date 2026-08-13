// The persisted theme pick — pure helpers shared by App and the test suite
// (the modelPick.ts pattern). A theme is a UI pick, not behavior config, so it
// lives in localStorage like the game/provider/model picks; unlike those there
// is no backend list to reconcile against, so the reader itself owns the
// default instead of returning null for a downstream snap.

export const THEME_STORAGE_KEY = "wikilens.theme";

/** The closed theme set. "default" is the bare `:root` appearance (no
 * data-theme attribute); every other id is an overrides-only
 * `:root[data-theme="<id>"]` block in styles.css (DESIGN.md §8). */
export const THEMES = ["default", "micrographics"] as const;

export type ThemeId = (typeof THEMES)[number];

/** The stored theme pick, or "default" when unset or corrupted. Stored as the
 * bare id string (no JSON — nothing else rides along). */
export function storedTheme(): ThemeId {
  try {
    const raw = localStorage.getItem(THEME_STORAGE_KEY);
    if (raw && (THEMES as readonly string[]).includes(raw)) {
      return raw as ThemeId;
    }
  } catch {
    // Unreadable storage — fall through to the default appearance.
  }
  return "default";
}
