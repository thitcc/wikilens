import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { listModels } from "../api";
import {
  MENU_FIXED_HEIGHT,
  modelMenuPlacement,
  type MenuPlacement,
} from "../menuPlacement";
import { centerRowInList } from "../menuScroll";
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
 * Opens as a fixed-height dropdown below the panel's bottom edge (into the
 * free window space under the content-hugging panel), flipping to the upward
 * `--menu-clearance` anchoring when a tall panel leaves more room above —
 * measured once per open via `modelMenuPlacement`. The fixed frame is a
 * feature: filtering, group collapse, and list loading never resize the card.
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
  const listRef = useRef<HTMLDivElement | null>(null);
  const selectedRowRef = useRef<HTMLButtonElement | null>(null);
  const didCenterRef = useRef(false);
  const requestedRef = useRef<Set<string>>(new Set());
  const [placement, setPlacement] = useState<MenuPlacement>({
    direction: "down",
    height: MENU_FIXED_HEIGHT,
  });

  // Measure once per open (the menu remounts every open, and overlay://shown
  // closes menus, so geometry is never stale across shows). useLayoutEffect:
  // a corrected placement lands before first paint. The panel growing under
  // an open menu while an answer streams is accepted — the next open
  // corrects. In jsdom the rect is zeros and innerHeight is 768, which
  // resolves to exactly the initial state.
  useLayoutEffect(() => {
    const panel = menuRef.current?.parentElement; // .panel — the positioning context
    if (!panel) return;
    const rect = panel.getBoundingClientRect();
    const next = modelMenuPlacement({
      panelTop: rect.top,
      panelHeight: rect.height,
      viewportHeight: window.innerHeight,
    });
    // Keep the previous object when nothing changed: the common open
    // resolves to exactly the initial down/460, and React's Object.is
    // bail-out then skips the second render-commit.
    setPlacement((prev) =>
      prev.direction === next.direction && prev.height === next.height
        ? prev
        : next,
    );
  }, []);

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

  // Center the selected row once it exists, like GameMenu — but the lists
  // arrive async (listModels), so this keys on `lists` with a one-shot guard
  // instead of running once on mount. A selected model inside a collapsed
  // group (OpenRouter) keeps the shot until its first expand renders the row.
  useLayoutEffect(() => {
    if (didCenterRef.current) return;
    const row = selectedRowRef.current;
    if (!listRef.current || !row) return;
    didCenterRef.current = true;
    centerRowInList(listRef.current, row);
  }, [lists]);

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
    <div
      className={"menu" + (placement.direction === "down" ? " menu--down" : "")}
      style={{ height: placement.height }}
      ref={menuRef}
      role="dialog"
      aria-label="Choose a model"
    >
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
      <div
        className="menu-note menu-note--info"
        title="Any model can answer — Fast models also power the quick pre-search query rewrite."
      >
        ⓘ Fast models are recommended
      </div>
      <div className="menu-list" ref={listRef}>
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
                      ref={isSelected ? selectedRowRef : undefined}
                      // The assistive-tech counterpart of the check glyph.
                      aria-current={isSelected ? "true" : undefined}
                      onClick={() => onSelect(provider.id, model)}
                    >
                      <span className="row-name">{model.label}</span>
                      <span className="row-side">
                        {model.vision && (
                          <Badge title="Can read screenshots">Image</Badge>
                        )}
                        {/* Strict checks: absent = unknown = no badge. */}
                        {model.reasoning === false && (
                          <Badge title="Answers directly — best for WikiLens's pre-search query rewrite">
                            Fast
                          </Badge>
                        )}
                        {model.reasoning === true && (
                          <Badge title="Thinks before answering — slower, and WikiLens skips its pre-search query rewrite">
                            Reasoning
                          </Badge>
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
