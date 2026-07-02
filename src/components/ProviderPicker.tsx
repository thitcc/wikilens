import type { ProviderInfo } from "../types";

interface ProviderPickerProps {
  providers: ProviderInfo[];
  value: string;
  onChange: (providerId: string) => void;
  disabled?: boolean;
}

/** Dropdown of LLM providers. Selection is persisted by the parent. */
export function ProviderPicker({ providers, value, onChange, disabled }: ProviderPickerProps) {
  return (
    <select
      className="provider-picker"
      value={value}
      disabled={disabled}
      aria-label="LLM provider"
      onChange={(e) => onChange(e.currentTarget.value)}
    >
      {providers.length === 0 && <option value="">Loading…</option>}
      {providers.map((provider) => (
        <option key={provider.id} value={provider.id}>
          {provider.name}
        </option>
      ))}
    </select>
  );
}
