import { Fragment, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { onOverlayHidden, resumeHotkeys, setHotkey, suspendHotkeys } from "../api";
import type { HotkeyInfo, HotkeyRole, SettingsInfo } from "../types";
import {
  comboFromEvent,
  labelParts,
  pendingModLabels,
  sameCombo,
  toAccelerator,
  validateCombo,
} from "../hotkeys";

interface SettingsMenuProps {
  settings: SettingsInfo;
  /** A shortcut was saved (already persisted and re-registered Rust-side). */
  onSaved: (next: SettingsInfo) => void;
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
 * "Shortcuts" popover: the summon and capture hotkeys, each with a key
 * recorder. Arming a row suspends the OS registrations (pressing the current
 * combo mid-recording must not toggle the overlay) and swallows every
 * keydown at capture phase — Enter must not reach the prompt, and Esc gets a
 * third layer: cancel recording, then close the menu, then hide the overlay.
 * Same interaction contract as AddGameMenu otherwise: capture-phase Esc,
 * outside-pointerdown close excluding the trigger, direct `.panel` child.
 */
export function SettingsMenu({
  settings,
  onSaved,
  onClose,
  triggerRef,
}: SettingsMenuProps) {
  const [recording, setRecording] = useState<HotkeyRole | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Why the last recorded combo was refused — shown under the armed row. */
  const [hint, setHint] = useState<string | null>(null);
  const [pendingMods, setPendingMods] = useState<string[]>([]);
  const menuRef = useRef<HTMLDivElement | null>(null);
  // Whether THIS menu suspended the registrations — resume exactly once per
  // suspend, whatever exit path runs (save, cancel, hide, unmount).
  const suspendedRef = useRef(false);

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
                disabled={saving}
                aria-label={`Reset the ${ROLE_NAMES[role]} shortcut to ${info.defaultLabel}`}
                onClick={() => void handleReset(role)}
              >
                Reset
              </button>
            )}
            <button
              type="button"
              className="hotkey-btn"
              disabled={saving}
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

  return (
    <div
      className="menu menu--top"
      ref={menuRef}
      role="dialog"
      aria-label="Shortcuts"
    >
      <div className="menu-list">
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
