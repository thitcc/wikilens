import type { RefObject } from "react";

interface ModelChipProps {
  providerName: string;
  modelLabel: string;
  open: boolean;
  disabled?: boolean;
  onToggle: () => void;
  /** Shared with ModelMenu so its outside-click close can exclude the chip
   * (otherwise a chip click would close-then-reopen the menu). */
  buttonRef: RefObject<HTMLButtonElement | null>;
}

/** Quiet footer chip showing the current provider + model; opens the menu.
 * Disabled while an ask is streaming, so the answer on screen always matches
 * the model on the chip. */
export function ModelChip({
  providerName,
  modelLabel,
  open,
  disabled,
  onToggle,
  buttonRef,
}: ModelChipProps) {
  return (
    <button
      ref={buttonRef}
      type="button"
      className={"quiet-chip" + (open ? " is-open" : "")}
      disabled={disabled}
      aria-haspopup="dialog"
      aria-expanded={open}
      aria-label={`Model: ${providerName} ${modelLabel}`}
      onClick={onToggle}
    >
      {providerName} <span className="dot">·</span> {modelLabel}{" "}
      <span className="caret">▾</span>
    </button>
  );
}
