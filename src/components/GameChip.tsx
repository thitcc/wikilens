import type { RefObject } from "react";

interface GameChipProps {
  gameName: string;
  open: boolean;
  disabled?: boolean;
  onToggle: () => void;
  /** Shared with GameMenu (and AddGameMenu) so outside-click close can
   * exclude the chip — otherwise a chip click would close-then-reopen. */
  buttonRef: RefObject<HTMLButtonElement | null>;
}

/** Quiet header chip showing the current game; opens the game menu. The
 * game reads as identity, not a form field (exploration 4c) — the caret and
 * hover fill are the affordance, mirroring the footer model chip. */
export function GameChip({
  gameName,
  open,
  disabled,
  onToggle,
  buttonRef,
}: GameChipProps) {
  return (
    <button
      ref={buttonRef}
      type="button"
      className={"quiet-chip" + (open ? " is-open" : "")}
      disabled={disabled}
      aria-haspopup="dialog"
      aria-expanded={open}
      aria-label={`Game: ${gameName}`}
      onClick={onToggle}
    >
      <span className="chip-name">{gameName}</span>{" "}
      <span className="caret">▾</span>
    </button>
  );
}
