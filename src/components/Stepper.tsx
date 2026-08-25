import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, RefObject } from "react";
import { createPortal } from "react-dom";
import { cycle } from "../stepper";

export interface StepperOption {
  id: string;
  /** Visible name ("Custom API", "Local AI"). */
  name: string;
  /** The option row's aria-label — sentence case and descriptive, per the
   * instrument-layer rule: accessible names never take a visual register. */
  label: string;
  /** Right-rail signal ("Needs a key", "Not set up here"). */
  note?: string | null;
}

/** Popover placement, measured from the settings card (its containing block —
 * the card's `overflow: hidden` is the intended clamp). PAIRED CONSTANTS with
 * styles.css: gap = --space-4, inset = --space-6. */
const POPOVER_GAP = 4;
const POPOVER_INSET = 6;

interface Placement {
  top: number;
  left: number;
  minWidth: number;
  /** Which side of the row the popover landed on — drives A-06's travel
   * direction (drops when below the row, rises when flipped above). */
  dir: "down" | "up";
}

interface StepperProps {
  options: StepperOption[];
  currentId: string;
  /** Every commit path lands here — arrows and popover rows alike. The owner
   * wires its own semantics around it (mode = async IPC with an error slot,
   * theme = sync state); same-value picks are filtered out before this. */
  onPick: (id: string) => void;
  /** Gates the whole control (the mode stepper under `busyAll`); the arrows
   * additionally disable when there is nothing to cycle to. */
  disabled?: boolean;
  prevLabel: string;
  nextLabel: string;
  /** The center value's fixed accessible name. The current VALUE is conveyed
   * by the popover rows' aria-current, following the menus' convention. */
  valueLabel: string;
  popoverId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The settings card — the popover's portal target, so it escapes the
   * scrolling list's clip and floats over the card
   * (vault/2026-08-13_stepper-popover-third-altitude.md). */
  cardRef: RefObject<HTMLDivElement | null>;
  /** The scrolling list — a scroll while open closes the popover instead of
   * tracking it (placement is measured once per open). */
  listRef: RefObject<HTMLDivElement | null>;
}

/**
 * A game-style enum row: `◁  Value  ▷`. The arrows cycle the options with
 * wrap (`cycle` in stepper.ts); the center value opens a floating popover
 * listing them, the check on the current one. Esc is deliberately NOT handled
 * here — SettingsMenu's capture-phase listener owns the Esc layering and
 * closes the popover via `onOpenChange` before its own close.
 */
export function Stepper({
  options,
  currentId,
  onPick,
  disabled,
  prevLabel,
  nextLabel,
  valueLabel,
  popoverId,
  open,
  onOpenChange,
  cardRef,
  listRef,
}: StepperProps) {
  const rowRef = useRef<HTMLDivElement | null>(null);
  const valueRef = useRef<HTMLButtonElement | null>(null);
  const popRef = useRef<HTMLDivElement | null>(null);
  const [placement, setPlacement] = useState<Placement | null>(null);
  // Inline callbacks get fresh identities every render; the open-scoped
  // effects read through this ref so they subscribe once per open.
  const onOpenChangeRef = useRef(onOpenChange);
  onOpenChangeRef.current = onOpenChange;
  const wasOpen = useRef(false);

  // Measure per open, before paint (no hidden-frame flicker). No re-measure
  // while open: the scroll/resize effect below closes the popover instead.
  useLayoutEffect(() => {
    if (!open) {
      setPlacement(null);
      return;
    }
    const row = rowRef.current;
    const card = cardRef.current;
    const pop = popRef.current;
    const value = valueRef.current;
    if (!row || !card || !pop || !value) return;
    const rowRect = row.getBoundingClientRect();
    const cardRect = card.getBoundingClientRect();
    const valueRect = value.getBoundingClientRect();
    // jsdom reports zero rects — take the plain below-branch with no math so
    // behavior tests never depend on geometry (and no NaN reaches a style).
    if (cardRect.height === 0) {
      setPlacement({ top: 0, left: 0, minWidth: 0, dir: "down" });
      return;
    }
    // The popover never narrows below the value it came from; set before
    // reading its width so the clamp sees the final box.
    pop.style.minWidth = `${valueRect.width}px`;
    const popHeight = pop.offsetHeight;
    const popWidth = pop.offsetWidth;
    const below = rowRect.bottom - cardRect.top + POPOVER_GAP;
    let top: number;
    let dir: Placement["dir"];
    if (below + popHeight > cardRect.height - POPOVER_INSET) {
      dir = "up";
      top = Math.max(
        POPOVER_INSET,
        rowRect.top - cardRect.top - POPOVER_GAP - popHeight,
      );
    } else {
      dir = "down";
      top = below;
    }
    const left = Math.min(
      Math.max(POPOVER_INSET, valueRect.left - cardRect.left),
      Math.max(POPOVER_INSET, cardRect.width - POPOVER_INSET - popWidth),
    );
    setPlacement({ top, left, minWidth: valueRect.width, dir });
    // The refs are stable; options can't change while the popover is open
    // (every mutation path runs through a pick, which closes it first).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Focus lands on the current option so AT announces name + aria-current;
  // runs after the placement commit, so the popover is visible by then.
  useEffect(() => {
    if (!open) return;
    const target =
      popRef.current?.querySelector<HTMLButtonElement>(
        '[aria-current="true"]',
      ) ?? popRef.current?.querySelector<HTMLButtonElement>("button");
    target?.focus();
  }, [open]);

  // Closing unmounts the focused option row and Chromium drops focus to
  // <body> (the SettingsMenu trap, one level down) — hand it back to the
  // value button. An outside click keeps its own target: focus only moved to
  // body if it was inside the popover. A disabled value refuses the focus and
  // SettingsMenu's busyAll-settle recovery takes over.
  useEffect(() => {
    if (wasOpen.current && !open && document.activeElement === document.body) {
      valueRef.current?.focus();
    }
    wasOpen.current = open;
  }, [open]);

  // The placement is a one-shot measurement — a list scroll or window resize
  // under it would float the popover over stale geometry, so both close it.
  useEffect(() => {
    if (!open) return;
    const close = () => onOpenChangeRef.current(false);
    const list = listRef.current;
    list?.addEventListener("scroll", close);
    window.addEventListener("resize", close);
    return () => {
      list?.removeEventListener("scroll", close);
      window.removeEventListener("resize", close);
    };
  }, [open, listRef]);

  // Outside-pointerdown close; the value button is excluded because its own
  // onClick toggles (the menus' trigger rule). An arrow press is "outside":
  // it closes the popover and its click still cycles — the arrows never work
  // under an open popover's stale placement.
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (popRef.current?.contains(target)) return;
      if (valueRef.current?.contains(target)) return;
      onOpenChangeRef.current(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  if (options.length === 0) return null;

  const ids = options.map((o) => o.id);
  const current = options.find((o) => o.id === currentId) ?? options[0];
  const arrowsDisabled = disabled || options.length <= 1;

  function pick(dir: 1 | -1) {
    const next = cycle(ids, currentId, dir);
    if (next !== currentId) onPick(next);
  }

  function onValueKeyDown(e: ReactKeyboardEvent<HTMLButtonElement>) {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      pick(-1);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      pick(1);
    }
  }

  // Vertical focus-walk between the option rows, clamped like the menus'
  // highlight (`nextHighlight`) — only the horizontal CYCLE wraps.
  function onPopoverKeyDown(e: ReactKeyboardEvent<HTMLDivElement>) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const rows = Array.from(
      popRef.current?.querySelectorAll<HTMLButtonElement>(".model-row") ?? [],
    );
    const i = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      e.key === "ArrowDown" ? Math.min(rows.length - 1, i + 1) : Math.max(0, i - 1);
    rows[next]?.focus();
  }

  return (
    <div className="stepper-row" ref={rowRef}>
      <button
        type="button"
        className="stepper-arrow"
        disabled={arrowsDisabled}
        aria-label={prevLabel}
        onClick={() => pick(-1)}
      >
        <span aria-hidden="true">◁</span>
      </button>
      <button
        type="button"
        className={"stepper-value" + (open ? " is-open" : "")}
        ref={valueRef}
        disabled={disabled}
        aria-expanded={open}
        aria-controls={popoverId}
        aria-label={valueLabel}
        onClick={() => onOpenChange(!open)}
        onKeyDown={onValueKeyDown}
      >
        <span className="row-name">{current.name}</span>
        {current.note && <span className="row-note">{current.note}</span>}
      </button>
      <button
        type="button"
        className="stepper-arrow"
        disabled={arrowsDisabled}
        aria-label={nextLabel}
        onClick={() => pick(1)}
      >
        <span aria-hidden="true">▷</span>
      </button>
      {open &&
        cardRef.current !== null &&
        createPortal(
          <div
            className={
              "stepper-popover" +
              (placement?.dir === "up" ? " stepper-popover--up" : "")
            }
            id={popoverId}
            ref={popRef}
            style={
              placement
                ? {
                    top: placement.top,
                    left: placement.left,
                    minWidth: placement.minWidth,
                  }
                : { visibility: "hidden" }
            }
            onKeyDown={onPopoverKeyDown}
          >
            {options.map((o) => (
              <button
                key={o.id}
                type="button"
                className={"model-row" + (o.id === currentId ? " selected" : "")}
                aria-current={o.id === currentId ? "true" : undefined}
                aria-label={o.label}
                onClick={() => {
                  onOpenChange(false);
                  if (o.id !== currentId) onPick(o.id);
                }}
              >
                <span className="row-main">
                  <span className="row-name">{o.name}</span>
                </span>
                <span className="row-side">
                  {o.note && <span className="row-note">{o.note}</span>}
                  <span className="check" aria-hidden="true">
                    ✓
                  </span>
                </span>
              </button>
            ))}
          </div>,
          cardRef.current,
        )}
    </div>
  );
}
