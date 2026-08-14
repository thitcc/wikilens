import { Fragment, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import {
  getAppVersion,
  listKeyStatus,
  onOverlayHidden,
  openExternal,
  removeApiKey,
  resumeHotkeys,
  setApiKey,
  setHotkey,
  setMode,
  setPanelPosition,
  setPositionLocked,
  suspendHotkeys,
} from "../api";
import type {
  HotkeyInfo,
  HotkeyRole,
  KeyStatus,
  Mode,
  PositionChoice,
  SettingsInfo,
} from "../types";
import {
  comboFromEvent,
  labelParts,
  pendingModLabels,
  sameCombo,
  toAccelerator,
  validateCombo,
} from "../hotkeys";
import { keyHelp } from "../providerHelp";
import type { ThemeId } from "../theme";
import { Badge } from "./Badge";
import { Stepper } from "./Stepper";

interface SettingsMenuProps {
  settings: SettingsInfo;
  /** The active appearance (DESIGN.md §8) — App owns the state and the
   * localStorage write; this panel only renders the pick and reports clicks. */
  theme: ThemeId;
  onThemeChange: (next: ThemeId) => void;
  /** A shortcut or the mode was saved (already persisted Rust-side). */
  onSaved: (next: SettingsInfo) => void;
  /** A key was saved or removed — App re-fetches the keyed-provider list.
   * Never fired by the open-time status fetch (nothing changed then). */
  onKeysChanged?: () => void;
  onClose: () => void;
  /** The header gear; outside-click close ignores it (see GameChip). */
  triggerRef: RefObject<HTMLButtonElement | null>;
}

const ROLE_NAMES: Record<HotkeyRole, string> = {
  summon: "Summon",
  capture: "Capture",
};

/** The error tag for the panel-wide key-status fetch. The other `sourceError`
 * ids are a mode ("default"/"custom") or a provider id, so this one must be
 * neither — its slot sits under the heading, not on a row. */
const KEYS_ROW = "keys";

/** The option popovers' ids, for the value buttons' `aria-controls`.
 * Module constants are safe: App's `openMenu` union guarantees one mounted
 * SettingsMenu. */
const ANSWERS_POPOVER_ID = "settings-answers-options";
const THEME_POPOVER_ID = "settings-theme-options";
const POSITION_POPOVER_ID = "settings-position-options";

/** The padlock's error tag — not an option id (the Position slot renders
 * both), same reasoning as KEYS_ROW. */
const POSITION_LOCK_ROW = "position-lock";

/** The Position stepper's wire values, in cycle order. */
const POSITION_OPTIONS: { id: PositionChoice; name: string; label: string; note?: string | null }[] = [
  { id: "top-right", name: "Top Right", label: "Dock the panel to the top right" },
  { id: "top-left", name: "Top Left", label: "Dock the panel to the top left" },
  {
    id: "bottom-right",
    name: "Bottom Right",
    label: "Dock the panel to the bottom right",
  },
  {
    id: "bottom-left",
    name: "Bottom Left",
    label: "Dock the panel to the bottom left",
  },
  { id: "center", name: "Center", label: "Center the panel on the screen" },
  { id: "manual", name: "Manual", label: "Keep the panel where you drag it" },
];

/** The padlock states — hand-drawn to the trash icon's 16-grid, stroke ink
 * so the open/closed shackle reads at 12px. */
const LOCK_OPEN_ICON = (
  <svg
    aria-hidden="true"
    width="12"
    height="12"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.4"
    strokeLinecap="round"
  >
    <rect x="3.5" y="7" width="9" height="6" rx="1.2" />
    <path d="M5.5 7V4.8a2.6 2.6 0 0 1 5-1" />
  </svg>
);
const LOCK_CLOSED_ICON = (
  <svg
    aria-hidden="true"
    width="12"
    height="12"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.4"
    strokeLinecap="round"
  >
    <rect x="3.5" y="7" width="9" height="6" rx="1.2" />
    <path d="M5.5 7V5.5a2.5 2.5 0 0 1 5 0V7" />
  </svg>
);

/** Bootstrap Icons "trash" (MIT), sized like the header gear. Static, so it
 * lives at module scope rather than being rebuilt on every render. */
const TRASH_ICON = (
  <svg
    aria-hidden="true"
    width="12"
    height="12"
    viewBox="0 0 16 16"
    fill="currentColor"
  >
    <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5m2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5m3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0z" />
    <path d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4zM2.5 3h11V2h-11z" />
  </svg>
);

/** A combo's <kbd> chips with "+" separators, shared by both row states. */
function comboKeys(parts: string[]) {
  return parts.map((part, i) => (
    <Fragment key={i}>
      {i > 0 && "+"}
      <kbd>{part}</kbd>
    </Fragment>
  ));
}

/**
 * The Settings panel: where answers come from, and the two key recorders.
 *
 * Two jobs, not one list. The "Answers" stepper picks a MODE — the built-in
 * model (when this install has one) or Custom API, your own provider — by
 * cycling the arrows or picking from the value's option popover. The provider
 * lines below it are not choices: they only set and clear keys, because which
 * provider answers is picked from the footer chip on the main panel. An
 * unkeyed line opens in place into a single key field (one at a time). A
 * stored key's only action is the trash: the keyed line is static — a neutral
 * "Set" pill marks it, and re-keying means trash, then the now-unkeyed line.
 * Ink strength reinforces the pill (keyed at full ink, unkeyed receding). A
 * pasted key crosses IPC once and is never displayed back.
 *
 * Two rules follow from the split, and both are load-bearing. A key never
 * moves the value: saving one stores it and nothing else
 * (vault/2026-07-29_keys-are-not-a-mode-choice.md). And the key lines exist
 * exactly while Custom API answers — their visibility is derived from the
 * mode, so the list can never sit open under a value that isn't answering.
 *
 * Arming a recorder suspends the OS registrations (pressing the current combo
 * mid-recording must not toggle the overlay) and swallows every keydown at
 * capture phase — Enter must not reach the prompt. Esc is layered four deep,
 * exactly: cancel recording, then close an open option popover, then close
 * the menu, then hide the overlay — and deliberately no layer for the key
 * form. Same interaction contract as AddGameMenu otherwise: capture-phase
 * Esc, outside-pointerdown close excluding the trigger, direct `.panel`
 * child.
 */
export function SettingsMenu({
  settings,
  theme,
  onThemeChange,
  onSaved,
  onKeysChanged,
  onClose,
  triggerRef,
}: SettingsMenuProps) {
  const [recording, setRecording] = useState<HotkeyRole | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Why the last recorded combo was refused — shown under the armed row. */
  const [hint, setHint] = useState<string | null>(null);
  const [pendingMods, setPendingMods] = useState<string[]>([]);
  /** Key presence per provider; `null` while the open-time fetch runs. */
  const [statuses, setStatuses] = useState<KeyStatus[] | null>(null);
  /** The provider whose key form is open — at most one, ever. */
  const [openKey, setOpenKey] = useState<string | null>(null);
  /** The one open field's text. Collapsing drops it: shorter key residency. */
  const [draft, setDraft] = useState("");
  /** The one in-flight panel action (outside the recorder's `saving`), and the
   * row running it — the pending label belongs to that row alone, or two keyed
   * providers both read "Removing…" for one click. */
  const [action, setAction] = useState<
    "idle" | "source" | "save-key" | "remove-key"
  >("idle");
  const [actionRow, setActionRow] = useState<string | null>(null);
  /** A failure, tagged with the row that produced it so it renders under it. */
  const [sourceError, setSourceError] = useState<{
    rowId: string;
    message: string;
  } | null>(null);
  /** Which stepper's option popover is open — at most one: the popover is a
   * scoped third altitude (vault/2026-08-13_stepper-popover-third-altitude.md)
   * and a second one would stack over it. */
  const [openStepper, setOpenStepper] = useState<
    "answers" | "theme" | "position" | null
  >(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  /** The scrolling list — the steppers close their popover when it scrolls
   * (placement is measured once per open). */
  const listRef = useRef<HTMLDivElement | null>(null);
  // Whether THIS menu suspended the registrations — resume exactly once per
  // suspend, whatever exit path runs (save, cancel, hide, unmount).
  const suspendedRef = useRef(false);
  /** Removal unmounts the focused trash; this row's now-unkeyed button
   * focuses itself on mount instead of letting focus fall to the dialog
   * (the busyAll recovery effect then sees focus is placed and stays out). */
  const refocusRowRef = useRef<string | null>(null);

  /** One in-flight action panel-wide — every section's controls wait. */
  const busyAll = saving || action !== "idle";
  /** The stored choice; never-chosen behaves as Custom (today's behavior). */
  const currentMode: Mode = settings.mode ?? "custom";
  /** The built-in row exists when it is configured — or when it is the stored
   * mode on an install that lost its config, because a state the player is
   * actually in must stay visible and one click from a fix. */
  const showBuiltIn =
    settings.defaultMode.configured || currentMode === "default";
  // Key statuses are fetched per open (the menu mounts fresh each time).
  useEffect(() => {
    let active = true;
    listKeyStatus()
      .then((list) => {
        if (active) setStatuses(list);
      })
      .catch((e) => {
        // Tagged for the slot under the heading, not a row: on a fresh install
        // the built-in row doesn't exist, and burying this behind the caret
        // would hide it in exactly the case where it matters most.
        if (active) setSourceError({ rowId: KEYS_ROW, message: String(e) });
      });
    return () => {
      active = false;
    };
  }, []);

  // A failed lookup renders no row rather than an error — a missing version
  // number is not player-actionable.
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    getAppVersion()
      .then((v) => {
        if (active) setVersion(v);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // Tab must start inside the labelled dialog, not walk the whole panel to
  // reach it (the card is the last `.panel` child).
  useEffect(() => {
    menuRef.current?.focus();
  }, []);

  // Every settled action either disables or unmounts the control that ran it,
  // and Chromium blurs both — which drops focus to <body> and deadens the
  // keyboard (App.tsx guards the same trap on close and on Stop). Re-entering
  // the dialog is the cheap fix, and it must run AFTER the re-render that
  // re-enables the control, so it keys on busyAll settling rather than a
  // handler's `finally`.
  useEffect(() => {
    if (busyAll) return;
    if (document.activeElement === document.body) menuRef.current?.focus();
  }, [busyAll]);

  /** Store the open field's key. That is the whole job — a key is not a choice
   * (vault/2026-07-29_keys-are-not-a-mode-choice.md), so the mode stays where
   * the player put it and the check does not move. `onKeysChanged` is all App
   * needs: its provider re-fetch is what makes a newly-keyed provider reachable
   * from the footer chip. */
  async function handleSaveKey(providerId: string) {
    const key = draft.trim();
    if (busyAll || key === "") return;
    setAction("save-key");
    setActionRow(providerId);
    setSourceError(null);
    try {
      // The fresh statuses land first, so the Custom row's "Needs a key" note
      // clears in the same commit the key does.
      setStatuses(await setApiKey(providerId, key));
      // Success collapses the form; drop the key text from state too.
      setDraft("");
      setOpenKey(null);
      onKeysChanged?.();
    } catch (e) {
      // The draft stays for a retry.
      setSourceError({ rowId: providerId, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  async function handleRemoveKey(providerId: string) {
    if (busyAll) return;
    setAction("remove-key");
    setActionRow(providerId);
    setSourceError(null);
    try {
      setStatuses(await removeApiKey(providerId));
      // The trash that had focus unmounts with this commit; hand focus to
      // the row's fresh "Add a key" button (removal never opens a field).
      refocusRowRef.current = providerId;
      // Never touches the mode or the pick — App's own fallback moves the
      // check if the removed provider was the source.
      onKeysChanged?.();
    } catch (e) {
      setSourceError({ rowId: providerId, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  function resume() {
    if (!suspendedRef.current) return;
    suspendedRef.current = false;
    void resumeHotkeys().catch((e) => setError(String(e)));
  }

  function arm(role: HotkeyRole) {
    setError(null);
    setHint(null);
    setPendingMods([]);
    // An open popover under an armed recorder would fight it for the arrows.
    setOpenStepper(null);
    setRecording(role);
    if (!suspendedRef.current) {
      suspendedRef.current = true;
      // Don't block arming on the IPC round-trip: a failed suspend just
      // means the current combo may still fire — the overlay://hidden
      // disarm below contains that.
      void suspendHotkeys().catch(() => {});
    }
  }

  function disarm() {
    setRecording(null);
    setPendingMods([]);
    resume();
  }

  async function save(role: HotkeyRole, accelerator: string) {
    setRecording(null);
    setPendingMods([]);
    setSaving(true);
    try {
      onSaved(await setHotkey(role, accelerator));
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
      resume();
    }
  }

  async function handleReset(role: HotkeyRole) {
    if (saving) return;
    setError(null);
    setHint(null);
    // No recording, no suspension: Rust swaps the live registration itself.
    await save(role, settings.hotkeys[role].defaultAccelerator);
  }

  // One capture-phase keydown listener for both jobs. Idle: Esc closes an
  // open option popover first, then the menu (App's Esc-hides-overlay
  // listener is bubble-phase on this same window — see ModelMenu). Armed:
  // swallow everything and run the recorder. Esc is four layers exactly —
  // recording, popover, menu, overlay — and deliberately none for the key
  // form.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (recording === null) {
        if (e.key === "Escape") {
          e.stopPropagation();
          if (openStepper !== null) {
            setOpenStepper(null);
            return;
          }
          onClose();
        }
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      if (saving) return;
      if (e.key === "Escape") {
        disarm();
        return;
      }
      const parts = comboFromEvent(e);
      if (parts === null) {
        setPendingMods(pendingModLabels(e));
        return;
      }
      const verdict = validateCombo(parts);
      if (!verdict.ok) {
        setHint(verdict.reason);
        return;
      }
      const accelerator = toAccelerator(parts);
      const other: HotkeyRole = recording === "summon" ? "capture" : "summon";
      if (sameCombo(accelerator, settings.hotkeys[other].accelerator)) {
        setHint(
          `That's already your ${ROLE_NAMES[other]} shortcut — pick a different combo.`,
        );
        return;
      }
      void save(recording, accelerator);
    };
    // Released modifiers leave the live preview (keyup carries the updated
    // modifier state).
    const onKeyUp = (e: KeyboardEvent) => {
      if (recording === null) return;
      e.preventDefault();
      e.stopPropagation();
      setPendingMods(pendingModLabels(e));
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recording, saving, settings, onClose, openStepper]);

  // Click/tap anywhere outside closes; the trigger is excluded because its
  // own onClick toggles. Closing unmounts — the unmount effect resumes.
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

  // The overlay hiding while armed would leave the hotkeys suspended with no
  // UI to resume them (the panel is gone) — disarm and restore immediately.
  const disarmRef = useRef(disarm);
  disarmRef.current = disarm;
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | null = null;
    void onOverlayHidden(() => {
      disarmRef.current();
      // A re-show would float an open popover over stale geometry.
      setOpenStepper(null);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else cleanup = unlisten;
    });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);

  // Last-resort resume on unmount (outside-click close, App-level close).
  useEffect(() => {
    return () => {
      if (suspendedRef.current) {
        suspendedRef.current = false;
        void resumeHotkeys().catch(() => {});
      }
    };
  }, []);

  function row(role: HotkeyRole, info: HotkeyInfo) {
    const armed = recording === role;
    return (
      <div className={"hotkey-row" + (armed ? " is-armed" : "")}>
        <span className="hotkey-role">{ROLE_NAMES[role]}</span>
        {armed ? (
          <span className="hotkey-live" aria-live="polite">
            {pendingMods.length > 0 ? (
              comboKeys(pendingMods)
            ) : (
              <span className="hotkey-wait">
                Press the new shortcut… Esc cancels
              </span>
            )}
          </span>
        ) : (
          <span className="hotkey-combo">
            {comboKeys(labelParts(info.accelerator))}
          </span>
        )}
        {!armed && (
          <span className="hotkey-actions">
            {!info.isDefault && (
              <button
                type="button"
                className="hotkey-btn"
                disabled={busyAll}
                aria-label={`Reset the ${ROLE_NAMES[role]} shortcut to ${info.defaultLabel}`}
                onClick={() => void handleReset(role)}
              >
                Reset
              </button>
            )}
            <button
              type="button"
              className="hotkey-btn"
              disabled={busyAll}
              aria-label={`Change the ${ROLE_NAMES[role]} shortcut`}
              onClick={() => arm(role)}
            >
              Change
            </button>
          </span>
        )}
      </div>
    );
  }

  /** The one open key field, mounted as a plain sibling of its row — no fill,
   * no border, no shadow, so it claims no second altitude inside the card. */
  function keyForm(status: KeyStatus) {
    const help = keyHelp(status.id);
    return (
      <div className="key-form">
        <div className="key-help">
          {help.url ? (
            <>
              Create one at{" "}
              <button
                type="button"
                className="source-link"
                onClick={() => void openExternal(help.url as string)}
              >
                {help.host}
              </button>{" "}
              — it&apos;s stored on this PC and never shown again.
            </>
          ) : (
            <>
              Create one in your {status.name} account — it&apos;s stored on
              this PC and never shown again.
            </>
          )}
        </div>
        <div className="key-field">
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            autoFocus
            value={draft}
            placeholder="Paste your key…"
            aria-label={`${status.name} API key`}
            onChange={(e) => setDraft(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleSaveKey(status.id);
              }
            }}
          />
          <button
            type="button"
            className="menu-action"
            disabled={busyAll || draft.trim() === ""}
            aria-label={`Save the ${status.name} API key`}
            onClick={() => void handleSaveKey(status.id)}
          >
            {action === "save-key" && actionRow === status.id
              ? "Saving…"
              : "Save"}
          </button>
        </div>
      </div>
    );
  }

  // The list picks a MODE (built-in vs your own provider), and the provider
  // lines below only set/clear a key — the provider itself is picked from the
  // footer chip on the main panel.

  /** No provider has a key. Gated on the fetch having landed: derived from a
   * bare `some()` it reads false while `statuses` is null, so both notes below
   * flashed on every open and pinned forever when the fetch failed. */
  const needsKey = statuses !== null && !statuses.some((s) => s.hasKey);
  /** Nothing on this install can answer at all — no configured built-in model
   * and no key. Keys on `configured` rather than `showBuiltIn`, so a stale
   * default (row visible, nothing behind it) still reads as dead. */
  const nothingCanAnswer = needsKey && !settings.defaultMode.configured;

  async function pickMode(next: Mode) {
    if (busyAll || currentMode === next) return;
    setAction("source");
    setActionRow(next);
    setSourceError(null);
    // The key lines' visibility is derived from the mode — leaving Custom
    // unmounts them, so the open form and its typed key must not outlive the
    // list (the key-residency rule toggleKeyForm honors, one level out).
    setOpenKey(null);
    setDraft("");
    try {
      onSaved(await setMode(next));
    } catch (e) {
      setSourceError({ rowId: next, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  function toggleKeyForm(id: string) {
    setSourceError(null);
    setDraft("");
    setOpenKey((open) => (open === id ? null : id));
  }

  /** The stepper's current value: the anchor, or "manual" while dragged. */
  const currentPosition: PositionChoice =
    settings.position.mode === "manual" ? "manual" : settings.position.anchor;

  async function pickPosition(next: PositionChoice) {
    if (busyAll || currentPosition === next) return;
    setAction("source");
    setActionRow(next);
    setSourceError(null);
    try {
      onSaved(await setPanelPosition(next));
    } catch (e) {
      setSourceError({ rowId: next, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  /** Flip the padlock. Locked = the header never drags, in any mode; the
   * stepper keeps working — the lock pins the gesture, not the setting. */
  async function toggleLock() {
    if (busyAll) return;
    setAction("source");
    setActionRow(POSITION_LOCK_ROW);
    setSourceError(null);
    try {
      onSaved(await setPositionLocked(!settings.position.locked));
    } catch (e) {
      setSourceError({ rowId: POSITION_LOCK_ROW, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  /** The Answers enum. One option on an unconfigured install that never
   * chose Default — the arrows disable and the popover lists the lone row.
   * The mode pick is an IPC round trip with an error slot; the theme pick
   * below is pure frontend state with nothing to fail — one Stepper, and
   * each wiring keeps its own contract. */
  const modeOptions = [
    ...(showBuiltIn
      ? [
          {
            id: "default",
            name: "Built In",
            label: "Answer with the built-in model",
            note: settings.defaultMode.configured ? null : "Not set up here",
          },
        ]
      : []),
    {
      id: "custom",
      name: "Custom API",
      label: "Answer with your own provider",
      note: needsKey ? "Needs a key" : null,
    },
  ];

  /** The appearance pick (DESIGN.md §8). Deliberately not busyAll-gated:
   * swapping chrome mid-action is safe, and the live swap is the theme's own
   * preview. */
  const themeOptions = [
    { id: "default", name: "Default", label: "Use the default theme" },
    {
      id: "micrographics",
      name: "Micrographics",
      label: "Use the Micrographics theme",
    },
  ];

  /* A key line: an unkeyed name opens the field; a keyed line is static —
     the Set pill marks it and the trash is its only control. Nothing here is
     a choice — only the Answers stepper above picks the mode. */
  const keyLine = (status: KeyStatus) => (
    <Fragment key={status.id}>
      <div className={"key-line" + (status.hasKey ? " is-keyed" : "")}>
        {status.hasKey ? (
          // Not a button on purpose: a stored key can't be replaced in
          // place, so a click target would promise an action that doesn't
          // exist (and its aria-expanded could never flip). AT reads the
          // name + the pill text; the trash carries the actionable label.
          // The pill rides the right rail (the model menu's badge position):
          // inline after the name it box-centers against Segoe's line box
          // and reads ~2px high next to the letters.
          <div className="model-row model-row--static">
            <span className="row-main">
              <span className="row-name">{status.name}</span>
            </span>
            <span className="row-side">
              <Badge title="Key stored on this PC">Set</Badge>
            </span>
          </div>
        ) : (
          <button
            type="button"
            className={"model-row" + (openKey === status.id ? " is-open" : "")}
            disabled={busyAll}
            aria-expanded={openKey === status.id}
            aria-label={`Add a key for ${status.name}`}
            ref={(el) => {
              // After a removal, this button is the line's fresh identity —
              // catch focus here (see refocusRowRef).
              if (el && refocusRowRef.current === status.id) {
                refocusRowRef.current = null;
                el.focus();
              }
            }}
            onClick={() => toggleKeyForm(status.id)}
          >
            <span className="row-main">
              <span className="row-name">{status.name}</span>
            </span>
          </button>
        )}
        {status.hasKey && (
          <button
            type="button"
            className="remove-btn remove-btn--icon"
            disabled={busyAll}
            aria-label={`Remove the ${status.name} API key`}
            title="Remove key"
            onClick={() => void handleRemoveKey(status.id)}
          >
            {action === "remove-key" && actionRow === status.id
              ? "…"
              : TRASH_ICON}
          </button>
        )}
      </div>
      {openKey === status.id && keyForm(status)}
      {sourceError?.rowId === status.id && (
        <div className="menu-error">{sourceError.message}</div>
      )}
    </Fragment>
  );

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      tabIndex={-1}
      role="dialog"
      aria-label="Settings"
    >
      <div className="menu-list menu-list--settings" ref={listRef}>
        <div className="menu-heading menu-heading--versioned">
          Answers
          {version !== null && <span className="menu-version">v{version}</span>}
        </div>
        {/* Panel-scope, so it sits above the stepper rather than on it: a
            failed key-status read isn't any one option's failure, and on a
            Default-mode install the key lines it describes aren't rendered. */}
        {sourceError?.rowId === KEYS_ROW && (
          <div className="menu-error">{sourceError.message}</div>
        )}
        {nothingCanAnswer && (
          <div className="menu-note">
            {currentMode === "custom"
              ? "Nothing can answer yet — add a key below."
              : "Nothing can answer yet — switch to Custom API and add a key."}
          </div>
        )}
        <Stepper
          options={modeOptions}
          currentId={currentMode}
          disabled={busyAll}
          onPick={(id) => void pickMode(id as Mode)}
          prevLabel="Switch to the previous answer source"
          nextLabel="Switch to the next answer source"
          valueLabel="Choose the answer source"
          popoverId={ANSWERS_POPOVER_ID}
          open={openStepper === "answers"}
          onOpenChange={(next) => setOpenStepper(next ? "answers" : null)}
          cardRef={menuRef}
          listRef={listRef}
        />
        {(sourceError?.rowId === "default" ||
          sourceError?.rowId === "custom") && (
          <div className="menu-error">{sourceError.message}</div>
        )}

        {currentMode === "custom" && (
          <div className="keys-nest">
            {statuses === null && <div className="menu-note">Loading…</div>}
            {statuses?.length === 0 && (
              <div className="menu-note">
                No providers to key on this install.
              </div>
            )}
            {statuses?.map(keyLine)}
          </div>
        )}

        <div className="menu-heading">Theme</div>
        <Stepper
          options={themeOptions}
          currentId={theme}
          onPick={(id) => onThemeChange(id as ThemeId)}
          prevLabel="Switch to the previous theme"
          nextLabel="Switch to the next theme"
          valueLabel="Choose the theme"
          popoverId={THEME_POPOVER_ID}
          open={openStepper === "theme"}
          onOpenChange={(next) => setOpenStepper(next ? "theme" : null)}
          cardRef={menuRef}
          listRef={listRef}
        />

        {/* The padlock rides the heading's right rail (the version-stamp
            seat; sheet picks A1/B1, 2026-08-14): accent closed lock while
            engaged, muted open lock while free. It pins the drag gesture
            only — the stepper stays live either way. */}
        <div className="menu-heading menu-heading--lock">
          Position
          <button
            type="button"
            className={
              "lock-btn" + (settings.position.locked ? " is-locked" : "")
            }
            disabled={busyAll}
            aria-pressed={settings.position.locked}
            aria-label={
              settings.position.locked
                ? "Unlock the panel position"
                : "Lock the panel position"
            }
            title={
              settings.position.locked
                ? "Position locked — dragging is off"
                : "Lock the position against accidental drags"
            }
            onClick={() => void toggleLock()}
          >
            {settings.position.locked ? LOCK_CLOSED_ICON : LOCK_OPEN_ICON}
          </button>
        </div>
        <Stepper
          options={POSITION_OPTIONS}
          currentId={currentPosition}
          disabled={busyAll}
          onPick={(id) => void pickPosition(id as PositionChoice)}
          prevLabel="Switch to the previous position"
          nextLabel="Switch to the next position"
          valueLabel="Choose the panel position"
          popoverId={POSITION_POPOVER_ID}
          open={openStepper === "position"}
          onOpenChange={(next) => setOpenStepper(next ? "position" : null)}
          cardRef={menuRef}
          listRef={listRef}
        />
        {sourceError !== null &&
          (sourceError.rowId === POSITION_LOCK_ROW ||
            POSITION_OPTIONS.some((o) => o.id === sourceError.rowId)) && (
            <div className="menu-error">{sourceError.message}</div>
          )}

        <div className="menu-heading">Shortcuts</div>
        {row("summon", settings.hotkeys.summon)}
        {row("capture", settings.hotkeys.capture)}
        {hint && recording !== null && <div className="hotkey-hint">{hint}</div>}
        {error && <div className="menu-error">{error}</div>}
      </div>

    </div>
  );
}
