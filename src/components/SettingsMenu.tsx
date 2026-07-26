import { Fragment, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import {
  listKeyStatus,
  onOverlayHidden,
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
import { Badge } from "./Badge";

interface SettingsMenuProps {
  settings: SettingsInfo;
  /** A shortcut was saved (already persisted and re-registered Rust-side). */
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
 * The Settings panel: Model source (Default vs Custom API — the footer chip
 * and ask path follow it), API keys (paste/remove per provider;
 * a pasted key crosses IPC once and is never displayed back — the only
 * action on a set key is Remove), and Shortcuts (the summon and capture key
 * recorders). Arming a recorder row suspends the OS registrations (pressing
 * the current combo mid-recording must not toggle the overlay) and swallows
 * every keydown at capture phase — Enter must not reach the prompt, and Esc
 * gets a third layer: cancel recording, then close the menu, then hide the
 * overlay. Same interaction contract as AddGameMenu otherwise: capture-phase
 * Esc, outside-pointerdown close excluding the trigger, direct `.panel`
 * child.
 */
export function SettingsMenu({
  settings,
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
  /** Per-provider key input text — cleared the moment a save succeeds. */
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  /** The one in-flight panel action (outside the recorder's `saving`). */
  const [action, setAction] = useState<
    "idle" | "mode" | "save-key" | "remove-key"
  >("idle");
  const [modeError, setModeError] = useState<string | null>(null);
  const [keysError, setKeysError] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  // Whether THIS menu suspended the registrations — resume exactly once per
  // suspend, whatever exit path runs (save, cancel, hide, unmount).
  const suspendedRef = useRef(false);

  /** One in-flight action panel-wide — every section's controls wait. */
  const busyAll = saving || action !== "idle";
  /** The stored choice; never-chosen displays as Custom (today's behavior). */
  const currentMode: Mode = settings.mode ?? "custom";

  // Key statuses are fetched per open (the menu mounts fresh each time).
  useEffect(() => {
    let active = true;
    listKeyStatus()
      .then((list) => {
        if (active) setStatuses(list);
      })
      .catch((e) => {
        if (active) setKeysError(String(e));
      });
    return () => {
      active = false;
    };
  }, []);

  async function pickMode(mode: Mode) {
    // A first click on Custom with no stored mode DOES persist the explicit
    // choice (settings.mode is null then, not "custom").
    if (busyAll || settings.mode === mode) return;
    setAction("mode");
    setModeError(null);
    try {
      onSaved(await setMode(mode));
    } catch (e) {
      setModeError(String(e));
    } finally {
      setAction("idle");
    }
  }

  async function handleSaveKey(providerId: string) {
    const draft = (drafts[providerId] ?? "").trim();
    if (busyAll || draft === "") return;
    setAction("save-key");
    setKeysError(null);
    try {
      setStatuses(await setApiKey(providerId, draft));
      // Success unmounts the input; drop the key text from state too.
      setDrafts((d) => ({ ...d, [providerId]: "" }));
      onKeysChanged?.();
    } catch (e) {
      // The draft stays for a retry.
      setKeysError(String(e));
    } finally {
      setAction("idle");
    }
  }

  async function handleRemoveKey(providerId: string) {
    if (busyAll) return;
    setAction("remove-key");
    setKeysError(null);
    try {
      setStatuses(await removeApiKey(providerId));
      onKeysChanged?.();
    } catch (e) {
      setKeysError(String(e));
    } finally {
      setAction("idle");
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

  function modeRow(mode: Mode, label: string, disabledExtra = false) {
    const selected = currentMode === mode;
    return (
      <button
        type="button"
        className={"model-row" + (selected ? " selected" : "")}
        disabled={disabledExtra || busyAll}
        aria-current={selected ? "true" : undefined}
        onClick={() => void pickMode(mode)}
      >
        <span className="row-name">{label}</span>
        <span className="row-side">
          <span className="check" aria-hidden="true">
            ✓
          </span>
        </span>
      </button>
    );
  }

  function keyRow(status: KeyStatus) {
    if (status.hasKey) {
      return (
        <div className="hotkey-row" key={status.id}>
          <span className="hotkey-role">{status.name}</span>
          <Badge title="A key is stored on this machine — it's never shown again">
            Key set
          </Badge>
          <span className="hotkey-actions">
            <button
              type="button"
              className="hotkey-btn"
              disabled={busyAll}
              aria-label={`Remove the ${status.name} API key`}
              onClick={() => void handleRemoveKey(status.id)}
            >
              Remove
            </button>
          </span>
        </div>
      );
    }
    return (
      <div className="menu-url-row" key={status.id}>
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={drafts[status.id] ?? ""}
          placeholder={`${status.name} API key…`}
          aria-label={`${status.name} API key`}
          onChange={(e) => {
            // Read before the updater runs — currentTarget is only valid
            // during dispatch.
            const value = e.currentTarget.value;
            setDrafts((d) => ({ ...d, [status.id]: value }));
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void handleSaveKey(status.id);
            }
          }}
        />
        <button
          type="button"
          disabled={busyAll || (drafts[status.id] ?? "").trim() === ""}
          aria-label={`Save the ${status.name} API key`}
          onClick={() => void handleSaveKey(status.id)}
        >
          Save
        </button>
      </div>
    );
  }

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      role="dialog"
      aria-label="Settings"
    >
      <div className="menu-list">
        <div className="menu-heading">Model source</div>
        {modeRow("default", "Default", !settings.defaultMode.configured)}
        {!settings.defaultMode.configured && (
          <div className="menu-note">Default isn't set up in this install.</div>
        )}
        {modeRow("custom", "Custom API")}
        {modeError && <div className="menu-error">{modeError}</div>}

        <div className="menu-heading">API keys</div>
        {statuses?.map(keyRow)}
        {keysError && <div className="menu-error">{keysError}</div>}

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
