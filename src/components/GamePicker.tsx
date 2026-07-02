import type { GameInfo } from "../types";

interface GamePickerProps {
  games: GameInfo[];
  value: string;
  onChange: (gameId: string) => void;
  disabled?: boolean;
}

/** Dropdown of supported games. Selection is persisted by the parent. */
export function GamePicker({ games, value, onChange, disabled }: GamePickerProps) {
  return (
    <select
      className="game-picker"
      value={value}
      disabled={disabled}
      aria-label="Game"
      onChange={(e) => onChange(e.currentTarget.value)}
    >
      {games.length === 0 && <option value="">Loading games…</option>}
      {games.map((game) => (
        <option key={game.id} value={game.id}>
          {game.name}
        </option>
      ))}
    </select>
  );
}
