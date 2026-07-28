import { Fragment, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import {
  listKeyStatus,
  onOverlayHidden,
  openExternal,
  removeApiKey,
  resumeHotkeys,
  setApiKey,
  setHotkey,
  setMode,
  suspendHotkeys,
} from "../api";
import type {
  HotkeyInfo,
  HotkeyRole,
  KeyStatus,
  Mode,
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

interface SettingsMenuProps {
  settings: SettingsInfo;
  /** The provider the footer chip is on — the source check follows it. */
  selectedProviderId: string;
  /** A shortcut or the mode was saved (already persisted Rust-side). */
  onSaved: (next: SettingsInfo) => void;
  /** A key was saved or removed — App re-fetches the keyed-provider list.
   * Never fired by the open-time status fetch (nothing changed then). */
  onKeysChanged?: () => void;
  /** A provider became the answer source — picked, or its key just landed. */
  onPickProvider?: (id: string) => void;
  onClose: () => void;
  /** The header gear; outside-click close ignores it (see GameChip). */
  triggerRef: RefObject<HTMLButtonElement | null>;
}

const ROLE_NAMES: Record<HotkeyRole, string> = {
  summon: "Summon",
  capture: "Capture",
};

/** The built-in source's row id — never a provider id (those come from Rust). */
const BUILTIN = "builtin";

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
 * The Settings panel: one answer-source list (the built-in model when this
 * install has one, then every provider) and the two key recorders.
 *
 * The list is exclusive — exactly one row wears the check, and it is the row
 * answers are coming from right now. Picking means one thing: make this my
 * source. A keyed row commits immediately; an unkeyed provider has nothing to
 * commit yet, so its row opens in place into a single key field (one at a
 * time), and saving the key completes the choice. A pasted key crosses IPC
 * once and is never displayed back — the only action on a stored key is
 * Remove.
 *
 * Arming a recorder suspends the OS registrations (pressing the current combo
 * mid-recording must not toggle the overlay) and swallows every keydown at
 * capture phase — Enter must not reach the prompt, and Esc gets a third layer:
 * cancel recording, then close the menu, then hide the overlay. Same
 * interaction contract as AddGameMenu otherwise: capture-phase Esc,
 * outside-pointerdown close excluding the trigger, direct `.panel` child.
 */
export function SettingsMenu({
  settings,
  selectedProviderId,
  onSaved,
  onKeysChanged,
  onPickProvider,
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
  const menuRef = useRef<HTMLDivElement | null>(null);
  // Whether THIS menu suspended the registrations — resume exactly once per
  // suspend, whatever exit path runs (save, cancel, hide, unmount).
  const suspendedRef = useRef(false);

  /** One in-flight action panel-wide — every section's controls wait. */
  const busyAll = saving || action !== "idle";
  /** The stored choice; never-chosen behaves as Custom (today's behavior). */
  const currentMode: Mode = settings.mode ?? "custom";
  /** The built-in row exists when it is configured — or when it is the stored
   * mode on an install that lost its config, because a state the player is
   * actually in must stay visible and one click from a fix. */
  const showBuiltIn =
    settings.defaultMode.configured || currentMode === "default";
  const keyed = statuses?.filter((s) => s.hasKey) ?? [];
  /** Which row wears the check. Derived, never stored, so it cannot disagree
   * with App's own first-keyed-provider fallback. */
  const sourceId =
    currentMode === "default"
      ? BUILTIN
      : (keyed.find((s) => s.id === selectedProviderId)?.id ??
        keyed[0]?.id ??
        null);

  // Key statuses are fetched per open (the menu mounts fresh each time).
  useEffect(() => {
    let active = true;
    listKeyStatus()
      .then((list) => {
        if (active) setStatuses(list);
      })
      .catch((e) => {
        if (active) setSourceError({ rowId: BUILTIN, message: String(e) });
      });
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

  /** Make a row the answer source. An unkeyed provider has nothing to commit
   * yet — it opens its key field instead, and fires no IPC. */
  async function pickSource(rowId: string, hasKey: boolean) {
    if (busyAll) return;
    if (rowId !== BUILTIN && !hasKey) {
      // Toggle the form; collapsing drops the draft.
      setSourceError(null);
      setDraft("");
      setOpenKey((open) => (open === rowId ? null : rowId));
      return;
    }
    if (rowId === sourceId) return;
    setAction("source");
    setActionRow(rowId);
    setSourceError(null);
    setOpenKey(null);
    try {
      if (rowId === BUILTIN) {
        onSaved(await setMode("default"));
      } else {
        if (settings.mode !== "custom") onSaved(await setMode("custom"));
        onPickProvider?.(rowId);
      }
    } catch (e) {
      setSourceError({ rowId, message: String(e) });
    } finally {
      setAction("idle");
      setActionRow(null);
    }
  }

  /** Save the open field's key, which also completes the source choice. */
  async function handleSaveKey(providerId: string) {
    const key = draft.trim();
    if (busyAll || key === "") return;
    setAction("save-key");
    setActionRow(providerId);
    setSourceError(null);
    try {
      setStatuses(await setApiKey(providerId, key));
      // Success collapses the form; drop the key text from state too.
      setDraft("");
      setOpenKey(null);
      // Order matters: the pick must be stored before onKeysChanged bumps
      // App's provider re-fetch, and the mode must be custom for it to run.
      onPickProvider?.(providerId);
      if (settings.mode !== "custom") onSaved(await setMode("custom"));
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

  // One capture-phase keydown listener for both jobs. Idle: Esc closes the
  // menu only (App's Esc-hides-overlay listener is bubble-phase on this same
  // window — see ModelMenu). Armed: swallow everything and run the recorder.
  // Deliberately no fourth layer for the key form — Esc keeps three exactly.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (recording === null) {
        if (e.key === "Escape") {
          e.stopPropagation();
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
  }, [recording, saving, settings, onClose]);

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
    void onOverlayHidden(() => disarmRef.current()).then((unlisten) => {
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

  /** One row of the source list. `note` is the right cell's single signal —
   * the accent check speaks for the live source, so it carries none. */
  function sourceRow(opts: {
    rowId: string;
    name: string;
    label: string;
    note: string | null;
    opens: boolean;
    onRemove?: () => void;
  }) {
    const selected = sourceId === opts.rowId;
    const open = openKey === opts.rowId;
    return (
      <div className="source-row">
        <button
          type="button"
          className={
            "model-row" +
            (selected ? " selected" : "") +
            (open ? " is-open" : "")
          }
          disabled={busyAll}
          aria-current={selected ? "true" : undefined}
          aria-expanded={opts.opens ? open : undefined}
          aria-label={opts.label}
          onClick={() => void pickSource(opts.rowId, !opts.opens)}
        >
          <span className="row-main">
            <span
              className={
                "group-caret" +
                (opts.opens ? (open ? "" : " is-collapsed") : " is-blank")
              }
              aria-hidden="true"
            >
              ▾
            </span>
            <span className="row-name">{opts.name}</span>
          </span>
          <span className="row-side">
            {opts.note && <span className="row-note">{opts.note}</span>}
            <span className="check" aria-hidden="true">
              ✓
            </span>
          </span>
        </button>
        {opts.onRemove && (
          <button
            type="button"
            className="remove-btn"
            disabled={busyAll}
            aria-label={`Remove the ${opts.name} API key`}
            onClick={opts.onRemove}
          >
            {action === "remove-key" && actionRow === opts.rowId
              ? "Removing…"
              : "Remove key"}
          </button>
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

  const nothingLive = sourceId === null;

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      tabIndex={-1}
      role="dialog"
      aria-label="Settings"
    >
      <div className="menu-list">
        <div className="menu-heading">Answers come from</div>
        <div className="menu-note">
          {!nothingLive
            ? "WikiLens finds the wiki pages. The one you pick here writes the answer."
            : showBuiltIn
              ? // The built-in row needs no key, so don't ask for one.
                "Nothing can answer yet — pick one below."
              : "Nothing can answer yet — pick one below and add its key."}
        </div>

        {showBuiltIn && (
          <>
            {sourceRow({
              rowId: BUILTIN,
              name: "Built into WikiLens",
              label: "Answer with the built-in model",
              note: settings.defaultMode.configured ? null : "Not set up here",
              opens: false,
            })}
            {sourceError?.rowId === BUILTIN && (
              <div className="menu-error">{sourceError.message}</div>
            )}
          </>
        )}

        {statuses === null && <div className="menu-note">Loading…</div>}
        {statuses?.map((status) => (
          <Fragment key={status.id}>
            {sourceRow({
              rowId: status.id,
              name: status.name,
              label: status.hasKey
                ? `Answer with ${status.name}`
                : `Add a key for ${status.name}`,
              note: !status.hasKey
                ? "Needs a key"
                : sourceId === status.id
                  ? null
                  : "Key added",
              opens: !status.hasKey,
              onRemove: status.hasKey
                ? () => void handleRemoveKey(status.id)
                : undefined,
            })}
            {openKey === status.id && keyForm(status)}
            {sourceError?.rowId === status.id && (
              <div className="menu-error">{sourceError.message}</div>
            )}
          </Fragment>
        ))}

        <div className="menu-heading">Shortcuts</div>
        {row("summon", settings.hotkeys.summon)}
        {row("capture", settings.hotkeys.capture)}
        {hint && recording !== null && <div className="hotkey-hint">{hint}</div>}
        {error && <div className="menu-error">{error}</div>}
        <div className="menu-note">
          Shortcuts work in-game, even while this panel is hidden.
        </div>
      </div>
    </div>
  );
}
