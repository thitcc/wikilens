import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { addGame, removeGame, suggestWikis } from "../api";
import type { GameInfo, WikiCandidate } from "../types";

interface AddGameMenuProps {
  /** Current games; only the `custom` ones are listed (with remove). */
  games: GameInfo[];
  onClose: () => void;
  /** A game was added (already persisted Rust-side, list needs refreshing). */
  onAdded: (game: GameInfo) => void;
  /** A user game was removed (already persisted Rust-side). */
  onRemoved: (id: string) => void;
  /** The header "+" button; outside-click close ignores it (see ModelChip). */
  triggerRef: RefObject<HTMLButtonElement | null>;
}

/** Hostname of a candidate, so the user can tell wiki.gg from Fandom. */
function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/**
 * "Add a game" popover: type a name → verified wiki suggestions (probed
 * Rust-side) → click to add; or paste a wiki address directly. Also lists the
 * user's own games with a remove control. Same interaction contract as
 * ModelMenu: capture-phase Esc (first Esc closes the menu, second hides the
 * overlay), outside-pointerdown close excluding the trigger, and rendered as
 * a direct child of `.panel` — `.content`'s overflow would clip it.
 */
export function AddGameMenu({
  games,
  onClose,
  onAdded,
  onRemoved,
  triggerRef,
}: AddGameMenuProps) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [candidates, setCandidates] = useState<WikiCandidate[] | null>(null);
  const [busy, setBusy] = useState<"idle" | "finding" | "adding" | "removing">(
    "idle",
  );
  const [error, setError] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

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

  // Click/tap anywhere outside closes; the trigger is excluded because its
  // own onClick toggles.
  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      onClose();
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [onClose, triggerRef]);

  const customGames = games.filter((g) => g.custom);
  const canFind = name.trim() !== "" && busy === "idle";
  const canAddUrl = name.trim() !== "" && url.trim() !== "" && busy === "idle";

  async function handleFind() {
    if (!canFind) return;
    setBusy("finding");
    setError(null);
    setCandidates(null);
    try {
      setCandidates(await suggestWikis(name));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("idle");
    }
  }

  async function handleAdd(targetUrl: string) {
    if (busy !== "idle") return;
    setBusy("adding");
    setError(null);
    try {
      onAdded(await addGame(name, targetUrl));
      // Success unmounts the menu (onAdded closes it) — no reset needed.
    } catch (e) {
      setError(String(e));
      setBusy("idle");
    }
  }

  async function handleRemove(id: string) {
    if (busy !== "idle") return;
    setBusy("removing");
    setError(null);
    try {
      await removeGame(id);
      onRemoved(id);
    } catch (e) {
      setError(String(e));
    } finally {
      // The menu stays open after a remove, so this must reset (unlike add,
      // where success unmounts the menu).
      setBusy("idle");
    }
  }

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      role="dialog"
      aria-label="Add a game"
    >
      <div className="menu-search">
        <input
          type="text"
          value={name}
          placeholder="Game name…"
          aria-label="Game name"
          autoFocus
          onChange={(e) => {
            setName(e.currentTarget.value);
            // Suggestions were probed for the old name; keeping them would
            // pair the new name with the wrong wiki on click.
            setCandidates(null);
            setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void handleFind();
            }
          }}
        />
      </div>
      <div className="menu-list">
        <button
          type="button"
          className="menu-action"
          disabled={!canFind}
          onClick={() => void handleFind()}
        >
          {busy === "finding" ? "Looking for its wiki…" : "Find its wiki"}
        </button>

        {candidates && candidates.length > 0 && (
          <>
            <div className="menu-heading">Found — click to add</div>
            {candidates.map((c) => (
              <button
                key={c.apiUrl}
                type="button"
                className="model-row"
                disabled={busy !== "idle"}
                onClick={() => void handleAdd(c.apiUrl)}
              >
                <span>{c.name}</span>
                <span className="row-note">{hostOf(c.apiUrl)}</span>
              </button>
            ))}
          </>
        )}
        {candidates && candidates.length === 0 && (
          <div className="menu-note">
            No wiki found on wiki.gg or Fandom — paste its address below.
          </div>
        )}

        <div className="menu-heading">Or paste a wiki address</div>
        <div className="menu-url-row">
          <input
            type="text"
            value={url}
            placeholder="https://…"
            aria-label="Wiki address"
            onChange={(e) => setUrl(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && canAddUrl) {
                e.preventDefault();
                void handleAdd(url);
              }
            }}
          />
          <button
            type="button"
            disabled={!canAddUrl}
            onClick={() => void handleAdd(url)}
          >
            Add
          </button>
        </div>

        {busy === "adding" && <div className="menu-note">Checking the wiki…</div>}
        {error && <div className="menu-error">{error}</div>}

        {customGames.length > 0 && (
          <>
            <div className="menu-heading">Your games</div>
            {customGames.map((g) => (
              <div key={g.id} className="custom-game-row">
                <span>{g.name}</span>
                <button
                  type="button"
                  className="remove-btn"
                  aria-label={`Remove ${g.name}`}
                  disabled={busy !== "idle"}
                  onClick={() => void handleRemove(g.id)}
                >
                  ✕
                </button>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
