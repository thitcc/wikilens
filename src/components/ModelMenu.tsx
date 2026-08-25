import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { listModels } from "../api";
import { isNavKey, nextHighlight } from "../menuNav";
import {
  MENU_FIXED_HEIGHT,
  menuViewportHeight,
  modelMenuPlacement,
  type MenuPlacement,
} from "../menuPlacement";
import { centerRowInList, keepRowInView } from "../menuScroll";
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
 * starts collapsed — which also defers its fetch until first expand. One
 * exception, applied at mount: the group holding the *current selection*
 * opens expanded, so the picked row is visible, centered, and highlighted
 * instead of hidden behind a collapsed header. */
const INITIALLY_COLLAPSED = new Set(["openrouter"]);

/**
 * Combined provider/model menu (design exploration 3a): filter input on top,
 * one collapsible group per provider, accent check on the selected row.
 * Mounted fresh on every open, so collapse/filter state intentionally resets.
 *
 * Opens as a fixed-height dropdown below the panel's bottom edge (into the
 * free window space under the content-hugging panel), flipping to the upward
 * `--menu-clearance` anchoring when a tall panel leaves more room above —
 * measured on open and on window `resize` while open via
 * `modelMenuPlacement` (opening pins the window at the 70% cap; the resize
 * lands async). The fixed frame is a feature: filtering, group collapse,
 * and list loading never resize the card.
 *
 * Rendered as a direct child of `.panel` — never inside `.content`, whose
 * `overflow-y: auto` would clip the absolutely-positioned menu.
 *
 * Keyboard model shared with GameMenu (see its doc comment): arrows move a
 * visible highlight through the model rows only — group headers stay Tab
 * territory — and Enter picks the highlighted row.
 */
export function ModelMenu({
  providers,
  selected,
  onSelect,
  onClose,
  chipRef,
}: ModelMenuProps) {
  const [filter, setFilter] = useState("");
  // The selection's own group never starts collapsed (see INITIALLY_COLLAPSED)
  // — fresh per open, since the menu remounts every open.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => {
    const initial = new Set(INITIALLY_COLLAPSED);
    initial.delete(selected.providerId);
    return initial;
  });
  const [lists, setLists] = useState<Record<string, ModelList>>({});
  const [loading, setLoading] = useState<Set<string>>(() => new Set());
  /** Per-group fetch failure copy. Registry ids degrade to their curated
   * fallback Rust-side and rarely land here; the Local group has no fallback
   * and rejects with "is it running?" copy that must reach the player — a
   * silent empty group would be a dead end. */
  const [errors, setErrors] = useState<Record<string, string>>({});
  // The keyboard highlight, as a `${providerId}:${modelId}` row key. null =
  // "auto": the first match while filtering, the selected row otherwise. A
  // key survives rows appearing above it (async lists) and degrades to
  // no-highlight when its row vanishes (group collapsed).
  const [highlightKey, setHighlightKey] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const selectedRowRef = useRef<HTMLButtonElement | null>(null);
  const rowRefs = useRef(new Map<string, HTMLButtonElement>());
  const didCenterRef = useRef(false);
  const requestedRef = useRef<Set<string>>(new Set());
  const [placement, setPlacement] = useState<MenuPlacement>({
    direction: "down",
    height: MENU_FIXED_HEIGHT,
  });

  // Measure on mount and again on window `resize` while open: App pins the
  // window at the 70% cap when a menu opens (set_overlay_height sentinel),
  // but that OS resize lands async — AFTER this first run.
  // menuViewportHeight() pre-empts it with the estimated cap; the resize
  // listener is the authoritative correction (the bail-out makes repeats
  // free). useLayoutEffect: a corrected placement lands before first paint.
  // In jsdom the rect is zeros, innerHeight is 768 and screen.height is 0,
  // which resolves to exactly the initial state.
  useLayoutEffect(() => {
    const measure = () => {
      const panel = menuRef.current?.parentElement; // .panel — the positioning context
      if (!panel) return;
      const rect = panel.getBoundingClientRect();
      const next = modelMenuPlacement({
        panelTop: rect.top,
        panelHeight: rect.height,
        viewportHeight: menuViewportHeight(
          window.innerHeight,
          window.screen?.height ?? 0,
        ),
      });
      // Keep the previous object when nothing changed: the common open
      // resolves to exactly the initial down/460, and React's Object.is
      // bail-out then skips the second render-commit.
      setPlacement((prev) =>
        prev.direction === next.direction && prev.height === next.height
          ? prev
          : next,
      );
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, []);

  function ensureList(providerId: string) {
    if (requestedRef.current.has(providerId)) return;
    requestedRef.current.add(providerId);
    setLoading((prev) => new Set(prev).add(providerId));
    listModels(providerId)
      .then((list) => {
        setLists((prev) => ({ ...prev, [providerId]: list }));
      })
      .catch((e) => {
        // Keep the message: for the Local group it is the whole diagnosis
        // ("Couldn't reach your local AI server at … — is it running?").
        // The empty fallback list keeps the group renderable either way.
        setErrors((prev) => ({ ...prev, [providerId]: String(e) }));
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

  // The arrow-navigable rows in display order — the same traversal the old
  // Enter-picks-the-first-match walked, generalized: collapsed groups are
  // skipped, so their rows (and the group headers, which stay Tab territory)
  // never enter the arrow order.
  const rows: Array<{ key: string; providerId: string; model: ModelInfo }> = [];
  for (const provider of providers) {
    if (collapsed.has(provider.id)) continue;
    for (const model of lists[provider.id]?.models ?? []) {
      if (matches(model)) {
        rows.push({ key: `${provider.id}:${model.id}`, providerId: provider.id, model });
      }
    }
  }

  // Derived, never stored: a stale index can't point at the wrong row when
  // lists arrive or a group collapses. Auto (null) anchors on the first match
  // while filtering, else on the selected row — falling back to the top row
  // (visible in an unscrolled list) when the selection hides inside a
  // collapsed group or its list hasn't arrived yet.
  const selectedKey = `${selected.providerId}:${selected.modelId}`;
  const highlightIndex =
    highlightKey !== null
      ? rows.findIndex((r) => r.key === highlightKey)
      : rows.length === 0
        ? -1
        : query
          ? 0
          : Math.max(
              rows.findIndex((r) => r.key === selectedKey),
              0,
            );
  const highlightedKey = highlightIndex >= 0 ? rows[highlightIndex].key : null;

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
          onChange={(e) => {
            setFilter(e.currentTarget.value);
            // Re-anchor on the first match — and scroll to the top so the
            // auto-highlight is actually visible (a highlighted-but-scrolled-
            // away row would recreate the invisible-pick bug).
            setHighlightKey(null);
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
                setHighlightKey(target.key);
                keepRowInView(
                  listRef.current,
                  rowRefs.current.get(target.key) ?? null,
                );
              }
              return;
            }
            if (e.key === "Enter") {
              e.preventDefault();
              const target = rows[highlightIndex];
              if (target) onSelect(target.providerId, target.model);
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
                {!isCollapsed &&
                  list?.source === "fallback" &&
                  !errors[provider.id] && (
                    <span className="group-note">offline list</span>
                  )}
              </button>
              {!isCollapsed && loading.has(provider.id) && (
                <div className="menu-note">Loading…</div>
              )}
              {!isCollapsed && errors[provider.id] && (
                <div className="menu-error">{errors[provider.id]}</div>
              )}
              {!isCollapsed &&
                visible.map((model) => {
                  const isSelected =
                    provider.id === selected.providerId &&
                    model.id === selected.modelId;
                  const rowKey = `${provider.id}:${model.id}`;
                  return (
                    <button
                      key={model.id}
                      type="button"
                      className={
                        "model-row" +
                        (isSelected ? " selected" : "") +
                        (rowKey === highlightedKey ? " is-highlighted" : "")
                      }
                      ref={(el) => {
                        if (el) rowRefs.current.set(rowKey, el);
                        else rowRefs.current.delete(rowKey);
                        // Keep feeding the center-on-open ref — dropping this
                        // silently kills the scroll-to-selected (jsdom can't
                        // catch it).
                        if (isSelected) selectedRowRef.current = el;
                      }}
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
