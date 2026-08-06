interface GameSuggestionProps {
  /** Display name of the detected game. */
  gameName: string;
  /** Apply the suggestion. Routed through App's ordinary game-change path, so
   * accepting is indistinguishable from picking the game by hand. */
  onAccept: () => void;
}

/** Header chip offering the game WikiLens found running under the overlay.
 *
 * It only ever *offers*. The detection is an inference from a process name,
 * and a wrong one must cost a glance — not an answer sourced from the wrong
 * game's wiki. Nothing changes until this is clicked; ignoring it is free and
 * needs no dismissal, because the chip is derived state and disappears the
 * moment the suggestion matches the selection.
 *
 * The arrow points right, at the game chip this would fill — "put Grounded
 * there". Accent ink marks it as the one thing in the header the player didn't
 * ask for. */
export function GameSuggestion({ gameName, onAccept }: GameSuggestionProps) {
  return (
    <button
      type="button"
      className="quiet-chip game-suggestion"
      aria-label={`Switch to ${gameName}`}
      title={`${gameName} is running — switch to its wiki`}
      onClick={onAccept}
    >
      <span className="chip-name">{gameName}</span>{" "}
      <span className="suggestion-arrow" aria-hidden="true">
        →
      </span>
    </button>
  );
}
