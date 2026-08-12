import type { RefObject } from "react";
import { instrumentLabel } from "../instrument";
import type { ThemeId } from "../theme";

interface GameChipProps {
  gameName: string;
  /** Drives the Micrographics label transform only — the visible text gets
   * underscores (case is CSS's job), the aria-label stays the raw name. */
  theme?: ThemeId;
  open: boolean;
  disabled?: boolean;
  onToggle: () => void;
  /** Shared with GameMenu (and AddGameMenu) so outside-click close can
   * exclude the chip — otherwise a chip click would close-then-reopen. */
  buttonRef: RefObject<HTMLButtonElement | null>;
}

/** Quiet header chip showing the current game; opens the game menu. The
 * game reads as identity, not a form field (exploration 4c) — the caret and
 * hover fill are the affordance, mirroring the footer model chip. The
 * `game-chip` class is the identity hook the Micrographics bracket marks
 * scope to (never bare .chip-name — GameSuggestion shares that class in the
 * same cluster). */
export function GameChip({
  gameName,
  theme,
  open,
  disabled,
  onToggle,
  buttonRef,
}: GameChipProps) {
  const label =
    theme === "micrographics" ? instrumentLabel(gameName) : gameName;
  return (
    <button
      ref={buttonRef}
      type="button"
      className={"quiet-chip game-chip" + (open ? " is-open" : "")}
      disabled={disabled}
      aria-haspopup="dialog"
      aria-expanded={open}
      aria-label={`Game: ${gameName}`}
      onClick={onToggle}
    >
      <span className="chip-name">{label}</span>{" "}
      <span className="caret">▾</span>
    </button>
  );
}
