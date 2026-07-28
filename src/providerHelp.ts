/**
 * Where a player mints an API key, per provider.
 *
 * Deliberately frontend-side. Putting the URL on the Rust `Provider` registry
 * would widen `KeyStatus`, and
 * `config_guardrails::key_status_serializes_exactly_the_known_fields` exists to
 * force review before that payload grows — a hyperlink is the wrong thing to
 * spend that pin on. The named cost of the split: a provider added by one
 * `providers.rs` entry gets a working row and a generic sentence here until
 * someone adds it below.
 */
const CONSOLES: Record<string, string> = {
  anthropic: "https://console.anthropic.com/settings/keys",
  deepseek: "https://platform.deepseek.com/api_keys",
  openrouter: "https://openrouter.ai/keys",
};

export interface KeyHelp {
  /** The URL to open, or `null` for an unmapped provider. */
  url: string | null;
  /** The bare host, shown as the link text ("console.anthropic.com"). */
  host: string | null;
}

/** The console link for a provider, or `{null, null}` when it has no mapping. */
export function keyHelp(providerId: string): KeyHelp {
  const url = CONSOLES[providerId];
  if (url === undefined) return { url: null, host: null };
  try {
    return { url, host: new URL(url).host };
  } catch {
    return { url: null, host: null };
  }
}
