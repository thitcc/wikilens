import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { isNavKey, nextHighlight } from "../menuNav";
import { keepRowInView } from "../menuScroll";
import { relativeTime } from "../historyTime";
import type { HistoryEntry } from "../types";

interface HistoryMenuProps {
  /** Past answered asks, newest first (`list_history` order). */
  entries: HistoryEntry[];
  onPick: (entry: HistoryEntry) => void;
  /** The pinned footer action. Rejects with a user-readable message when the
   * store can't be written — surfaced inline, the menu stays open. */
  onClear: () => Promise<void>;
  onClose: () => void;
  /** The header History chip; outside-click close ignores it. */
  chipRef: RefObject<HTMLButtonElement | null>;
}

/**
 * Owned answer-history menu: past questions newest-first, filterable by
 * question text or game name, with "Clear history" pinned at the bottom. Picking a row restores that answer
 * without re-asking (App owns the restore). Rows stack the question over a
 * muted "game · when" meta line — list rows, not chat bubbles, per
 * PRODUCT.md's anti-reference.
 *
 * Same interaction contract as GameMenu: capture-phase Esc (first Esc closes
 * the menu, second hides the overlay), outside-pointerdown close excluding
 * the trigger chip, remounted fresh on every open, rendered as a direct
 * child of `.panel` — `.content`'s overflow would clip it.
 *
 * Keyboard model (shared with GameMenu/ModelMenu): focus stays on the filter
 * input; Up/Down move a visible highlight (Home/End jump, no wrap), Enter
 * picks the highlighted row, any other key keeps filtering. History has no
 * persistent selection, so the highlight anchors on the newest entry and the
 * list opens at the top — no center-on-open, no check glyph.
 */
export function HistoryMenu({
  entries,
  onPick,
  onClear,
  onClose,
  chipRef,
}: HistoryMenuProps) {
  const [filter, setFilter] = useState("");
  // The keyboard highlight as an entry id. null = "auto": the first (newest
  // or best-match) row. An id survives the row set changing under it and
  // degrades to no-highlight when its row vanishes.
  const [highlightId, setHighlightId] = useState<string | null>(null);
  const [clearError, setClearError] = useState<string | null>(null);
  // "Now" is frozen per open (the menu remounts each open) — a live clock
  // would re-render rows mid-read for no player-visible gain.
  const [nowMs] = useState(() => Date.now());
  const menuRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const rowRefs = useRef(new Map<string, HTMLButtonElement>());

  // Esc closes the menu only. Capture phase, because App's Esc-hides-overlay
  // listener is bubble-phase on this same window (see GameMenu).
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

  // One box filters both axes: the question text and the game's name — so
  // "abiotic" surfaces every Abiotic Factor ask alongside any question that
  // happens to mention it.
  const query = filter.trim().toLowerCase();
  const rows = query
    ? entries.filter(
        (e) =>
          e.question.toLowerCase().includes(query) ||
          e.gameName.toLowerCase().includes(query),
      )
    : entries;

  // Derived, never stored: a stale index can't point at the wrong row when
  // the row set changes. Auto (null) anchors on the first row — the newest
  // entry, or the first match while filtering — so a bare Enter restores a
  // visibly highlighted row, never an invisible one.
  const highlightIndex =
    highlightId !== null
      ? rows.findIndex((r) => r.id === highlightId)
      : rows.length === 0
        ? -1
        : 0;
  const highlightedId = highlightIndex >= 0 ? rows[highlightIndex].id : null;

  function handleClear() {
    setClearError(null);
    // On success App empties the list and closes the menu; on rejection the
    // menu stays open with the message inline.
    void onClear().catch((e) => setClearError(String(e)));
  }

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      role="dialog"
      aria-label="Answer history"
    >
      <div className="menu-search">
        <input
          type="text"
          value={filter}
          placeholder="Filter by question or game…"
          aria-label="Filter by question or game"
          autoFocus
          onChange={(e) => {
            setFilter(e.currentTarget.value);
            // Re-anchor on the first match — and scroll to the top so the
            // auto-highlight is actually visible (see GameMenu).
            setHighlightId(null);
            if (listRef.current) listRef.current.scrollTop = 0;
          }}
          onKeyDown={(e) => {
            // Esc stays with the capture-phase window listener above — the
            // menu/overlay layering must not gain a third handler here.
            if (isNavKey(e.key)) {
              e.preventDefault(); // the caret would jump to the input's ends
              const next = nextHighlight(highlightIndex, rows.length, e.key);
              if (next >= 0) {
                const target = rows[next];
                setHighlightId(target.id);
                keepRowInView(
                  listRef.current,
                  rowRefs.current.get(target.id) ?? null,
                );
              }
              return;
            }
            if (e.key === "Enter") {
              e.preventDefault();
              const target = rows[highlightIndex];
              if (target) onPick(target);
            }
          }}
        />
      </div>
      <div className="menu-list" ref={listRef}>
        {rows.map((entry) => (
          <button
            key={entry.id}
            ref={(el) => {
              if (el) rowRefs.current.set(entry.id, el);
              else rowRefs.current.delete(entry.id);
            }}
            type="button"
            className={
              "model-row history-row" +
              (entry.id === highlightedId ? " is-highlighted" : "")
            }
            onClick={() => onPick(entry)}
          >
            <span className="row-main">
              <span className="row-name">{entry.question}</span>
              <span className="row-meta">
                {entry.gameName} · {relativeTime(entry.createdMs, nowMs)}
              </span>
            </span>
          </button>
        ))}
        {rows.length === 0 && <div className="menu-note">No matches</div>}
      </div>
      <div className="menu-footer">
        {clearError && (
          <div className="menu-error" role="alert">
            {clearError}
          </div>
        )}
        <button type="button" className="menu-action" onClick={handleClear}>
          Clear history
        </button>
      </div>
    </div>
  );
}
