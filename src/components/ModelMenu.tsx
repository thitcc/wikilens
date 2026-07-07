import { Fragment, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { listModels } from "../api";
import type { ModelInfo, ModelList, ProviderInfo } from "../types";
import { Badge } from "./Badge";

interface ModelMenuProps {
  providers: ProviderInfo[];
  selected: { providerId: string; modelId: string };
  onSelect: (providerId: string, model: ModelInfo) => void;
  onClose: () => void;
  /** The footer chip; outside-click close ignores it (see ModelChip). */
  chipRef: RefObject<HTMLButtonElement | null>;
}

/** OpenRouter's 300+ model catalog would dominate the menu, so its group
 * starts collapsed — which also defers its fetch until first expand. */
const INITIALLY_COLLAPSED = new Set(["openrouter"]);

/**
 * Combined provider/model menu (design exploration 3a): filter input on top,
 * one collapsible group per provider, accent check on the selected row.
 * Mounted fresh on every open, so collapse/filter state intentionally resets.
 *
 * Rendered as a direct child of `.panel` — never inside `.content`, whose
 * `overflow-y: auto` would clip the absolutely-positioned menu.
 */
export function ModelMenu({
  providers,
  selected,
  onSelect,
  onClose,
  chipRef,
}: ModelMenuProps) {
  const [filter, setFilter] = useState("");
  const [collapsed, setCollapsed] = useState<Set<string>>(
    () => new Set(INITIALLY_COLLAPSED),
  );
  const [lists, setLists] = useState<Record<string, ModelList>>({});
  const [loading, setLoading] = useState<Set<string>>(() => new Set());
  const menuRef = useRef<HTMLDivElement | null>(null);
  const requestedRef = useRef<Set<string>>(new Set());

  function ensureList(providerId: string) {
    if (requestedRef.current.has(providerId)) return;
    requestedRef.current.add(providerId);
    setLoading((prev) => new Set(prev).add(providerId));
    listModels(providerId)
      .then((list) => {
        setLists((prev) => ({ ...prev, [providerId]: list }));
      })
      .catch(() => {
        // The backend already degrades to its curated fallback on any fetch
        // problem, so a rejection here is an IPC-level failure; show an empty
        // offline group rather than an error state in a quick-pick menu.
        setLists((prev) => ({
          ...prev,
          [providerId]: { models: [], source: "fallback" },
        }));
      })
      .finally(() => {
        setLoading((prev) => {
          const next = new Set(prev);
          next.delete(providerId);
          return next;
        });
      });
  }

  // Lazy fetch: expanded groups load when the menu opens; collapsed ones
  // (OpenRouter) wait for their first expand.
  useEffect(() => {
    for (const provider of providers) {
      if (!collapsed.has(provider.id)) ensureList(provider.id);
    }
    // Runs once per mount: the menu remounts on every open, and later
    // expansions fetch via toggleGroup.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Esc closes the menu only. Capture phase, because App's Esc-hides-overlay
  // listener is bubble-phase on this same window — stopping propagation here
  // keeps the overlay open; the next Esc then reaches App and hides it.
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

  // Click/tap anywhere outside the menu closes it. The chip is excluded: its
  // own onClick toggles, and closing here first would make that a reopen.
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

  function toggleGroup(providerId: string) {
    // Side effect outside the updater (updaters must stay pure): expanding a
    // group is what triggers its lazy fetch.
    if (collapsed.has(providerId)) ensureList(providerId);
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(providerId)) next.delete(providerId);
      else next.add(providerId);
      return next;
    });
  }

  const query = filter.trim().toLowerCase();
  const matches = (model: ModelInfo) =>
    query === "" ||
    model.label.toLowerCase().includes(query) ||
    model.id.toLowerCase().includes(query);

  /** Visible rows in display order, for Enter-picks-the-first-match. */
  function firstVisible(): { providerId: string; model: ModelInfo } | null {
    for (const provider of providers) {
      if (collapsed.has(provider.id)) continue;
      const model = lists[provider.id]?.models.find(matches);
      if (model) return { providerId: provider.id, model };
    }
    return null;
  }

  return (
    <div className="menu" ref={menuRef} role="dialog" aria-label="Choose a model">
      <div className="menu-search">
        <input
          type="text"
          value={filter}
          placeholder="Filter models…"
          aria-label="Filter models"
          autoFocus
          onChange={(e) => setFilter(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              const first = firstVisible();
              if (first) onSelect(first.providerId, first.model);
            }
          }}
        />
      </div>
      <div className="menu-list">
        {providers.map((provider) => {
          const isCollapsed = collapsed.has(provider.id);
          const list = lists[provider.id];
          const visible = list?.models.filter(matches) ?? [];
          return (
            <Fragment key={provider.id}>
              <button
                type="button"
                className="group-header"
                aria-expanded={!isCollapsed}
                onClick={() => toggleGroup(provider.id)}
              >
                <span
                  className={"group-caret" + (isCollapsed ? " is-collapsed" : "")}
                >
                  ▾
                </span>
                {provider.name}
                {!isCollapsed && list?.source === "fallback" && (
                  <span className="group-note">offline list</span>
                )}
              </button>
              {!isCollapsed && loading.has(provider.id) && (
                <div className="menu-note">Loading…</div>
              )}
              {!isCollapsed &&
                visible.map((model) => {
                  const isSelected =
                    provider.id === selected.providerId &&
                    model.id === selected.modelId;
                  return (
                    <button
                      key={model.id}
                      type="button"
                      className={"model-row" + (isSelected ? " selected" : "")}
                      onClick={() => onSelect(provider.id, model)}
                    >
                      <span className="row-name">{model.label}</span>
                      <span className="row-side">
                        {model.vision && (
                          <Badge title="Can read screenshots">Image</Badge>
                        )}
                        <span className="check" aria-hidden="true">
                          ✓
                        </span>
                      </span>
                    </button>
                  );
                })}
              {!isCollapsed &&
                list &&
                visible.length === 0 &&
                !loading.has(provider.id) && (
                  <div className="menu-note">
                    {query ? "No matches" : "No models"}
                  </div>
                )}
            </Fragment>
          );
        })}
      </div>
    </div>
  );
}
