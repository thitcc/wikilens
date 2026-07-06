import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { GameInfo } from "../types";

interface GameMenuProps {
  games: GameInfo[];
  selectedId: string;
  /** Recent game ids, most recent first; ids that no longer exist are
   * filtered here. */
  recentIds: string[];
  onSelect: (gameId: string) => void;
  /** The pinned footer action — swaps to the add-game menu. */
  onAddGame: () => void;
  onClose: () => void;
  /** The header game chip; outside-click close ignores it (see GameChip). */
  chipRef: RefObject<HTMLButtonElement | null>;
}

/** How many recents the menu pins (App stores a few more than shown). */
const RECENT_SHOWN = 3;

/** Two-letter scan anchor: initials of the first two words, else the first
 * two letters ("Stardew Valley" → SV, "Terraria" → TE). Deterministic and
 * collision-tolerant — it's an anchor, not an identifier. */
function monogram(name: string): string {
  const words = name.split(/\s+/).filter((w) => w.length > 0);
  const raw =
    words.length >= 2 ? `${words[0][0]}${words[1][0]}` : name.trim().slice(0, 2);
  return raw.toUpperCase();
}

/**
 * Owned game menu (exploration 4c) — replaces the native `<select>` popup,
 * which WebView2 drew outside the page as an unstylable white sheet. Recent
 * games pinned on top, the full list below (built-ins + user-added merged,
 * name-sorted), monogram tiles as scan anchors, and "Add a game…" pinned at
 * the bottom while the list scrolls.
 *
 * Same interaction contract as ModelMenu: capture-phase Esc (first Esc
 * closes the menu, second hides the overlay), outside-pointerdown close
 * excluding the trigger, remounted fresh on every open, and rendered as a
 * direct child of `.panel` — `.content`'s overflow would clip it.
 */
export function GameMenu({
  games,
  selectedId,
  recentIds,
  onSelect,
  onAddGame,
  onClose,
  chipRef,
}: GameMenuProps) {
  const [filter, setFilter] = useState("");
  const menuRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const selectedRowRef = useRef<HTMLButtonElement | null>(null);

  // Esc closes the menu only. Capture phase, because App's Esc-hides-overlay
  // listener is bubble-phase on this same window (see ModelMenu).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);

  // Click/tap anywhere outside closes; the chip is excluded because its own
  // onClick toggles.
  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (chipRef.current?.contains(target)) return;
      onClose();
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [onClose, chipRef]);

  // Center the selected row on open, like the native popup did. Runs once —
  // the menu remounts on every open, and the filter always starts empty.
  useEffect(() => {
    const list = listRef.current;
    const row = selectedRowRef.current;
    if (list && row && list.scrollHeight > list.clientHeight) {
      // Both offsetTops are .menu-relative (.menu-list is unpositioned, so
      // the rows' offsetParent is the absolutely-positioned .menu); the
      // difference converts the row into list coordinates — without it the
      // scroll overshoots by the search-bar height.
      const rowTopInList = row.offsetTop - list.offsetTop;
      list.scrollTop = Math.max(
        0,
        rowTopInList - list.clientHeight / 2 + row.offsetHeight / 2,
      );
    }
  }, []);

  const query = filter.trim().toLowerCase();

  const sorted = [...games].sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
  );
  const recent = recentIds
    .map((id) => games.find((g) => g.id === id))
    .filter((g): g is GameInfo => g !== undefined)
    .slice(0, RECENT_SHOWN);
  // Filtering flattens the sections: matches come from the sorted full list
  // (which already contains every recent game), so rows dedupe by
  // construction and the headings drop.
  const filtered = query
    ? sorted.filter((g) => g.name.toLowerCase().includes(query))
    : null;

  const firstVisible = filtered ? filtered[0] : (recent[0] ?? sorted[0]);
  const selectedInRecent = recent.some((g) => g.id === selectedId);

  function row(game: GameInfo, keyPrefix: string, withRef: boolean) {
    const isSelected = game.id === selectedId;
    return (
      // Keys are section-prefixed: a game can render in both Recent and
      // All games, and bare ids would collide.
      <button
        key={`${keyPrefix}-${game.id}`}
        ref={withRef && isSelected ? selectedRowRef : undefined}
        type="button"
        className={"model-row" + (isSelected ? " selected" : "")}
        onClick={() => onSelect(game.id)}
      >
        <span className="row-main">
          <span className="tile" aria-hidden="true">
            {monogram(game.name)}
          </span>
          <span className="row-name">{game.name}</span>
        </span>
        <span className="check" aria-hidden="true">
          ✓
        </span>
      </button>
    );
  }

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      role="dialog"
      aria-label="Choose a game"
    >
      <div className="menu-search">
        <input
          type="text"
          value={filter}
          placeholder="Filter games…"
          aria-label="Filter games"
          autoFocus
          onChange={(e) => setFilter(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              if (firstVisible) onSelect(firstVisible.id);
            }
          }}
        />
      </div>
      <div className="menu-list" ref={listRef}>
        {filtered ? (
          <>
            {filtered.map((g) => row(g, "match", true))}
            {filtered.length === 0 && (
              <div className="menu-note">No matches</div>
            )}
          </>
        ) : (
          <>
            {recent.length > 0 && (
              <>
                <div className="menu-heading">Recent</div>
                {recent.map((g) => row(g, "recent", true))}
              </>
            )}
            <div className="menu-heading">All games</div>
            {/* The scroll-to-selected ref goes to the first rendered copy:
                the Recent row when the selected game is pinned there. */}
            {sorted.map((g) => row(g, "all", !selectedInRecent))}
          </>
        )}
      </div>
      <div className="menu-footer">
        <button type="button" className="menu-action" onClick={onAddGame}>
          <span className="row-main">
            <span className="tile tile-add" aria-hidden="true">
              +
            </span>
            <span className="row-name">Add a game…</span>
          </span>
        </button>
      </div>
    </div>
  );
}
