import type { RefObject } from "react";
import { instrumentLabel } from "../instrument";
import type { ThemeId } from "../theme";

interface ModelChipProps {
  providerName: string;
  modelLabel: string;
  /** Drives the Micrographics label transform only — visible text gets
   * underscores per segment (the · separator stays), aria stays raw. */
  theme?: ThemeId;
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
  theme,
  open,
  disabled,
  onToggle,
  buttonRef,
}: ModelChipProps) {
  const micro = theme === "micrographics";
  const providerText = micro ? instrumentLabel(providerName) : providerName;
  const modelText = micro ? instrumentLabel(modelLabel) : modelLabel;
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
      {providerText} <span className="dot">·</span> {modelText}{" "}
      <span className="caret">▾</span>
    </button>
  );
}
