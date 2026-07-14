// The persisted model pick and its vision resolution — pure helpers shared by
// App and the test suite (extracted from App.tsx so they're testable without
// mounting the app).

import type { ProviderInfo, StoredModelPick } from "./types";

export const PROVIDER_STORAGE_KEY = "wikilens.selectedProvider";
export const MODEL_STORAGE_PREFIX = "wikilens.selectedModel.";

/** The user's explicit model pick for a provider, or null when they've never
 * picked one. Stored as JSON `{id, label, vision?}` so the chip can label
 * itself without any list fetch (offline included). `vision` is carried
 * through when present; entries saved before the badges feature lack it, and
 * healing that is the guardrails plan's concern. */
export function storedModel(providerId: string): StoredModelPick | null {
  if (!providerId) return null;
  try {
    const raw = localStorage.getItem(MODEL_STORAGE_PREFIX + providerId);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      typeof (parsed as StoredModelPick).id === "string" &&
      typeof (parsed as StoredModelPick).label === "string"
    ) {
      const pick = parsed as StoredModelPick;
      return {
        id: pick.id,
        label: pick.label,
        ...(typeof pick.vision === "boolean" ? { vision: pick.vision } : {}),
      };
    }
  } catch {
    // Corrupted entry — fall through to the provider default.
  }
  return null;
}

/** Whether the active model can read images, resolved with no fetch: DeepSeek
 * is definitively text-only (short-circuit — re-check when it ships a vision
 * model), then the stored pick's own flag, then the provider's default, then
 * false. */
export function activeModelVision(
  provider: ProviderInfo | undefined,
  pick: StoredModelPick | null,
): boolean {
  if (provider?.id === "deepseek") return false;
  if (typeof pick?.vision === "boolean") return pick.vision;
  return provider?.defaultModelVision ?? false;
}

/** Content equality for picks (id, label, and the `vision` flag including its
 * absence). Lets state setters keep the previous object when a re-read
 * produced an identical pick, so downstream effects keyed on the pick don't
 * re-fire. */
export function sameModelPick(
  a: StoredModelPick | null,
  b: StoredModelPick | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.id === b.id && a.label === b.label && a.vision === b.vision;
}
