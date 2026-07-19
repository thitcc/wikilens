import { Fragment, useEffect, useRef, useState } from "react";
import {
  ASK_CANCELLED,
  ask,
  beginCapture,
  cancelAsk,
  clearCapture,
  debugAvailable,
  getSettings,
  hideOverlay,
  listGames,
  listModels,
  listProviders,
  onAskDelta,
  onAskStatus,
  onCaptureAttached,
  onCaptureError,
  onCaptureHotkey,
  onOverlayHidden,
  onOverlayShown,
  showOverlay,
  toggleDebugWindow,
} from "./api";
import type {
  AskStatus,
  AttachmentInfo,
  GameInfo,
  ModelInfo,
  ProviderInfo,
  SettingsInfo,
  Source,
  StoredModelPick,
} from "./types";
import {
  DEFAULT_CAPTURE_LABEL,
  DEFAULT_SUMMON_LABEL,
  labelParts,
} from "./hotkeys";
import {
  MODEL_STORAGE_PREFIX,
  PROVIDER_STORAGE_KEY,
  activeModelVision,
  sameModelPick,
  storedModel,
} from "./modelPick";
import { AddGameMenu } from "./components/AddGameMenu";
import { GameChip } from "./components/GameChip";
import { SettingsMenu } from "./components/SettingsMenu";
import { GameMenu } from "./components/GameMenu";
import { ModelChip } from "./components/ModelChip";
import { ModelMenu } from "./components/ModelMenu";
import { PromptInput } from "./components/PromptInput";
import { AnswerView } from "./components/AnswerView";
import { SourceList } from "./components/SourceList";
import "./styles.css";

const GAME_STORAGE_KEY = "wikilens.selectedGame";
const RECENT_GAMES_KEY = "wikilens.recentGames";

/** How many recent game ids to remember (the menu shows the top 3). */
const RECENT_GAMES_STORED = 5;

/** Recently selected game ids, most recent first. Ids of removed games are
 * kept here harmlessly — the menu drops any id not in the current list. */
function storedRecentGames(): string[] {
  try {
    const parsed: unknown = JSON.parse(
      localStorage.getItem(RECENT_GAMES_KEY) ?? "[]",
    );
    if (Array.isArray(parsed)) {
      return parsed.filter((id): id is string => typeof id === "string");
    }
  } catch {
    // Corrupted entry — start fresh.
  }
  return [];
}

/** Why an image can't go to the current model — shared by the attachment
 * strip's hint and the capture hotkey's error surface, so the two never
 * drift. */
const CAPTURE_NEEDS_VISION =
  "This model can't read images — remove it or pick one with the Image badge.";

const STATUS_LABEL: Record<AskStatus, string> = {
  searching: "Searching the wiki…",
  understanding: "Understanding your question…",
  retrying: "Broadening the search…",
  reading: "Reading pages…",
  answering: "Answering…",
};

/** The one-shot answer-ready announcement (2026-07-19 critique P1: phases
 * were announced but the answer's arrival was silent — "Answering…" then
 * silence, forever). Spoken through the persistent role=status region. */
function answerReadyLabel(sourceCount: number): string {
  if (sourceCount === 0) return "Answer ready";
  if (sourceCount === 1) return "Answer ready — 1 source";
  return `Answer ready — ${sourceCount} sources`;
}

/** How long one ask phase may run before the slow-wiki hint shows. Above the
 * healthy worst cases (search ≲3s, 4-page fetch ≲8s), below the backend's 12s
 * per-request fetch timeout. Exported for the fake-timer tests. */
export const SLOW_WIKI_HINT_MS = 10_000;

/** Every phase except the LLM stream waits on the wiki. ("searching" also
 * joins the LLM rewrite, but only the wiki search there can run this long.) */
function isWikiBoundStatus(s: AskStatus): boolean {
  return s !== "answering";
}

/** A show this soon after a hide skips the select-all: an accidental hide
 * followed by a re-summon must not arm a keystroke that replaces the whole
 * draft. Born as the capital-C trap fix (the old Shift+C default made typing
 * `C` fire the toggle — resolved by the Ctrl+` default); kept because any
 * accidental hide arms the same loss. Exported for the fake-timer tests. */
export const SELECT_SUPPRESS_MS = 2_000;

function App() {
  const [games, setGames] = useState<GameInfo[]>([]);
  const [selectedGame, setSelectedGame] = useState<string>(
    () => localStorage.getItem(GAME_STORAGE_KEY) ?? "",
  );
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>(
    () => localStorage.getItem(PROVIDER_STORAGE_KEY) ?? "",
  );
  const [modelPick, setModelPick] = useState<StoredModelPick | null>(() =>
    storedModel(localStorage.getItem(PROVIDER_STORAGE_KEY) ?? ""),
  );
  const [recentGames, setRecentGames] = useState<string[]>(storedRecentGames);
  // At most one popover at a time — their capture-phase Esc handlers would
  // otherwise stack, and one Esc would close both.
  const [openMenu, setOpenMenu] = useState<
    "game" | "addGame" | "model" | "settings" | null
  >(null);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [sources, setSources] = useState<Source[]>([]);
  const [status, setStatus] = useState<AskStatus | null>(null);
  const [slowHint, setSlowHint] = useState(false);
  // The one-shot "Answer ready — N sources" text, set on resolve only (never
  // cancel or error) and cleared on the next submit — no timer: the text is
  // visually hidden, staying accurate for as long as the answer is on screen,
  // and clearing on submit guarantees a same-N repeat ask re-announces.
  const [announcement, setAnnouncement] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [attachment, setAttachment] = useState<AttachmentInfo | null>(null);
  const [debugChip, setDebugChip] = useState(false);
  // The configured shortcuts (null until get_settings resolves — the copy
  // below falls back to the shipped defaults, and the gear stays disabled).
  const [settings, setSettings] = useState<SettingsInfo | null>(null);

  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const gameChipRef = useRef<HTMLButtonElement | null>(null);
  const settingsChipRef = useRef<HTMLButtonElement | null>(null);
  // Latest capture handler, so the mount-only hotkey listener always sees
  // current state (e.g. `busy`) instead of a stale mount-time closure.
  const requestCaptureRef = useRef<() => void>(() => {});
  // When the overlay last hid (overlay://hidden), for the select-all
  // suppression below. 0 = never, so the first show always selects.
  const lastHiddenAtRef = useRef(0);

  // Load the supported games once; default the selection to the first game.
  useEffect(() => {
    let active = true;
    listGames()
      .then((list) => {
        if (!active) return;
        setGames(list);
        setSelectedGame((current) => {
          if (current && list.some((g) => g.id === current)) return current;
          return list[0]?.id ?? "";
        });
      })
      .catch((e) => setError(String(e)));
    return () => {
      active = false;
    };
  }, []);

  // Load the LLM providers once; default the selection to the first provider.
  useEffect(() => {
    let active = true;
    listProviders()
      .then((list) => {
        if (!active) return;
        setProviders(list);
        setSelectedProvider((current) => {
          if (current && list.some((p) => p.id === current)) return current;
          return list[0]?.id ?? "";
        });
      })
      .catch((e) => setError(String(e)));
    return () => {
      active = false;
    };
  }, []);

  // Load the configured shortcuts once. A failure is non-fatal: the copy
  // falls back to the shipped defaults and the gear stays disabled.
  useEffect(() => {
    let active = true;
    getSettings()
      .then((info) => {
        if (active) setSettings(info);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // The footer Debug chip exists only when the debug window does
  // (WIKILENS_DEBUG at startup). A failed probe just means no chip — never
  // an error box.
  useEffect(() => {
    let active = true;
    debugAvailable()
      .then((available) => {
        if (active) setDebugChip(available);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // Each provider remembers its own last-picked model; re-read it whenever
  // the provider changes (including the initial load/validation above). Keep
  // the previous object on a content-equal re-read — a fresh object would
  // re-arm the self-heal effect below and fire a second list_models.
  useEffect(() => {
    const next = storedModel(selectedProvider);
    setModelPick((prev) => (sameModelPick(prev, next) ? prev : next));
  }, [selectedProvider]);

  // Self-heal a pre-badges pick (stored without `vision`): one model-list load
  // (session cache / instant curated fallback — works offline) patches the flag
  // and rewrites localStorage, so capture gating resolves correctly for
  // returning users instead of wrongly treating them as text-only.
  useEffect(() => {
    if (!selectedProvider || !modelPick || typeof modelPick.vision === "boolean") {
      return;
    }
    let active = true;
    listModels(selectedProvider)
      .then((list) => {
        if (!active) return;
        const found = list.models.find((m) => m.id === modelPick.id);
        if (!found) return; // live-only / env-override id: fall back to the default
        const patched: StoredModelPick = {
          id: modelPick.id,
          label: modelPick.label,
          vision: found.vision,
        };
        localStorage.setItem(
          MODEL_STORAGE_PREFIX + selectedProvider,
          JSON.stringify(patched),
        );
        setModelPick(patched);
      })
      .catch(() => {
        // Non-fatal: gating falls back to the provider default until next time.
      });
    return () => {
      active = false;
    };
  }, [selectedProvider, modelPick]);

  // Subscribe to backend events for the lifetime of the app.
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];
    const register = (unlisten: () => void) => {
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    };

    void onOverlayShown(() => {
      setOpenMenu(null);
      inputRef.current?.focus();
      // Select the old question so the first keystroke starts the new one —
      // unless the panel was hidden moments ago, where "the old question" is
      // really a live draft the player is mid-typing (see SELECT_SUPPRESS_MS).
      if (Date.now() - lastHiddenAtRef.current > SELECT_SUPPRESS_MS) {
        inputRef.current?.select();
      }
    }).then(register);
    void onOverlayHidden(() => {
      lastHiddenAtRef.current = Date.now();
    }).then(register);
    void onAskStatus((s) => setStatus(s)).then(register);
    void onAskDelta((chunk) => setAnswer((prev) => prev + chunk)).then(register);
    void onCaptureAttached((info) => {
      setAttachment(info);
      setError(null);
    }).then(register);
    void onCaptureError((message) => setError(message)).then(register);
    // The global Ctrl+Shift+C hotkey funnels into the same request as the
    // footer button; the ref keeps this mount-only listener current.
    void onCaptureHotkey(() => requestCaptureRef.current()).then(register);

    return () => {
      disposed = true;
      cleanups.forEach((fn) => fn());
    };
  }, []);

  // Esc hides the overlay (focus returns to the game).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void hideOverlay();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Arm a per-phase slow-wiki timer; any status/busy change resets it. A
  // backend re-emit of the current status is a state bail-out (no effect
  // re-run), so it can't stretch the clock the optimistic submit started.
  useEffect(() => {
    setSlowHint(false);
    if (!busy || !status || !isWikiBoundStatus(status)) return;
    const id = window.setTimeout(() => setSlowHint(true), SLOW_WIKI_HINT_MS);
    return () => window.clearTimeout(id);
  }, [status, busy]);

  function handleGameChange(gameId: string) {
    setSelectedGame(gameId);
    localStorage.setItem(GAME_STORAGE_KEY, gameId);
    if (gameId) {
      const next = [
        gameId,
        ...recentGames.filter((id) => id !== gameId),
      ].slice(0, RECENT_GAMES_STORED);
      setRecentGames(next);
      localStorage.setItem(RECENT_GAMES_KEY, JSON.stringify(next));
    }
  }

  function closeMenu() {
    setOpenMenu(null);
    // Return focus to the prompt: both menus' inputs steal it on open, and
    // focus inside an unmounting subtree otherwise drops to <body>, deadening
    // the keyboard until the user clicks the textarea.
    inputRef.current?.focus();
  }

  function handleGameAdded(game: GameInfo) {
    handleGameChange(game.id);
    closeMenu();
    // Re-fetch so ordering matches the backend; on an IPC hiccup, fall back
    // to appending locally (the add itself already succeeded Rust-side).
    listGames()
      .then(setGames)
      .catch(() => setGames((prev) => [...prev, game]));
  }

  function handleGameRemoved(id: string) {
    // Menu stays open — the user may want to remove more than one. Drop the
    // row synchronously (no re-click window), then reconcile with the backend.
    const remaining = games.filter((g) => g.id !== id);
    setGames(remaining);
    if (selectedGame === id) handleGameChange(remaining[0]?.id ?? "");
    listGames()
      .then(setGames)
      .catch(() => {
        // The local filter above already applied; nothing more to do.
      });
  }

  function handleModelSelect(providerId: string, model: ModelInfo) {
    // Storage first: the provider-change effect re-reads the stored pick.
    localStorage.setItem(PROVIDER_STORAGE_KEY, providerId);
    localStorage.setItem(MODEL_STORAGE_PREFIX + providerId, JSON.stringify(model));
    setSelectedProvider(providerId);
    setModelPick(model);
    closeMenu();
  }

  const provider = providers.find((p) => p.id === selectedProvider);
  // Whether the active model can read images — gates capture and image submit.
  const vision = activeModelVision(provider, modelPick);

  // Start a capture (footer button and hotkey both land here). Rust hides the
  // panel, shows the crosshair overlay, and later fires capture://attached.
  // The `!vision` branch covers the hotkey path (the button is already
  // disabled): a global hotkey must never fail silently, so surface the
  // existing copy — and show the panel first, since the hotkey also fires
  // while it's hidden. Busy stays a silent no-op (the ask lock is visible).
  function handleCaptureRequest() {
    if (busy) return;
    if (!vision) {
      void showOverlay().catch(() => {});
      setError(CAPTURE_NEEDS_VISION);
      return;
    }
    setOpenMenu(null);
    void beginCapture().catch((e) => setError(String(e)));
  }
  // Point the mount-only hotkey listener at the current closure each render.
  requestCaptureRef.current = handleCaptureRequest;

  function handleRemoveAttachment() {
    setAttachment(null);
    // The frontend already dropped it; a failed Rust clear is harmless (the
    // next successful ask or capture overwrites the slot anyway).
    void clearCapture().catch(() => {});
  }

  // The status row's Stop action: fire the Rust-side abort and let the pending
  // `ask` promise settle (with ASK_CANCELLED) — never reset state here, or a
  // Stop racing a real completion would clobber the answer. The refocus covers
  // keyboard activation: resolving unmounts the focused button, which would
  // otherwise drop focus to <body> (the closeMenu trap).
  function handleStop() {
    void cancelAsk().catch(() => {});
    inputRef.current?.focus();
  }

  async function handleSubmit() {
    const trimmed = question.trim();
    if (busy || !trimmed || !selectedGame || !selectedProvider) return;
    // Never dispatch an image to a model that can't read it — the attachment
    // hint already explains why; this makes Enter a no-op instead of burning a
    // request we can predict will fail.
    if (attachment && !vision) return;

    setBusy(true);
    setOpenMenu(null);
    setError(null);
    setAnswer("");
    setSources([]);
    setAnnouncement(null);
    setStatus("searching");

    try {
      // The explicit pick when there is one, else the provider default; a
      // blank value would fall back Rust-side, this is just the same rule
      // applied eagerly so the chip and the request always agree.
      const model = modelPick?.id ?? provider?.defaultModel ?? "";
      const result = await ask(
        selectedGame,
        selectedProvider,
        model,
        trimmed,
        attachment?.id,
      );
      setAnswer(result.answer);
      setSources(result.sources);
      setAnnouncement(answerReadyLabel(result.sources.length));
      // Clears-on-success, mirroring the Rust slot: the model answered, so the
      // screenshot is spent. A failed ask keeps it (this line isn't reached).
      setAttachment(null);
    } catch (e) {
      // A cancelled ask resets quietly: no error box, and whatever partial
      // answer already streamed stays on screen. (A delta racing the abort may
      // still append after this settles — harmless, it lands on the kept text.)
      if (String(e) !== ASK_CANCELLED) setError(String(e));
    } finally {
      setBusy(false);
      setStatus(null);
    }
  }

  const showPlaceholder = !error && !busy && !answer;
  // The phase to display, null outside a healthy busy ask. A narrowed value
  // (not a boolean) so STATUS_LABEL[activeStatus] type-checks in the JSX.
  const activeStatus = !error && busy ? status : null;
  // Configured shortcut copy, falling back to the shipped defaults until
  // get_settings resolves (or if it failed).
  const summonKeys = labelParts(
    settings?.hotkeys.summon.accelerator ?? DEFAULT_SUMMON_LABEL,
  );
  const captureLabel = settings?.hotkeys.capture.label ?? DEFAULT_CAPTURE_LABEL;

  return (
    <div className="panel">
      <header className="panel-header">
        {/* The gear rides with the brand (the game chip owns the right edge).
            Disabled until get_settings resolves — the popover needs data. */}
        <span className="brand-cluster">
          <span className="brand">WikiLens</span>
          <button
            type="button"
            ref={settingsChipRef}
            className={
              "quiet-chip settings-chip" +
              (openMenu === "settings" ? " is-open" : "")
            }
            disabled={busy || !settings}
            aria-haspopup="dialog"
            aria-expanded={openMenu === "settings"}
            aria-label="Shortcuts"
            title="Shortcuts"
            // Same closeMenu() rule as the game chip (focus contract).
            onClick={() =>
              openMenu === "settings" ? closeMenu() : setOpenMenu("settings")
            }
          >
            {/* Filled cog silhouette (Bootstrap Icons gear-fill, MIT) — teeth
                attached to the ring + a center hole keep it reading as a
                gear, not a sun, at this size. */}
            <svg
              className="settings-gear"
              aria-hidden="true"
              width="12"
              height="12"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M9.405 1.05c-.413-1.4-2.397-1.4-2.81 0l-.1.34a1.464 1.464 0 0 1-2.105.872l-.31-.17c-1.283-.698-2.686.705-1.987 1.987l.169.311c.446.82.023 1.841-.872 2.105l-.34.1c-1.4.413-1.4 2.397 0 2.81l.34.1a1.464 1.464 0 0 1 .872 2.105l-.17.31c-.698 1.283.705 2.686 1.987 1.987l.311-.169a1.464 1.464 0 0 1 2.105.872l.1.34c.413 1.4 2.397 1.4 2.81 0l.1-.34a1.464 1.464 0 0 1 2.105-.872l.31.17c1.283.698 2.686-.705 1.987-1.987l-.169-.311a1.464 1.464 0 0 1 .872-2.105l.34-.1c1.4-.413 1.4-2.397 0-2.81l-.34-.1a1.464 1.464 0 0 1-.872-2.105l.17-.31c.698-1.283-.705-2.686-1.987-1.987l-.311.169a1.464 1.464 0 0 1-2.105-.872zM8 10.93a2.929 2.929 0 1 1 0-5.86 2.929 2.929 0 0 1 0 5.858z" />
            </svg>
          </button>
        </span>
        <GameChip
          gameName={
            games.length === 0
              ? "Loading games…"
              : (games.find((g) => g.id === selectedGame)?.name ??
                "Pick a game")
          }
          open={openMenu === "game"}
          disabled={busy}
          // Closing must go through closeMenu(): the click focuses the chip,
          // and without the prompt refocus, typing lands on the chip and
          // Enter reopens the menu.
          onToggle={() =>
            openMenu === "game" ? closeMenu() : setOpenMenu("game")
          }
          buttonRef={gameChipRef}
        />
      </header>

      <PromptInput
        value={question}
        onChange={setQuestion}
        onSubmit={handleSubmit}
        busy={busy}
        inputRef={inputRef}
      />

      {/* Attached screenshot — a direct .panel child, never inside .content
          (its overflow would clip the strip). Dims + explains when the active
          model can't read images. */}
      {attachment && (
        <div className="attachment">
          <div
            className={
              "attachment-row" + (!vision ? " attachment-row--blocked" : "")
            }
          >
            <img
              className="attachment-thumb"
              src={attachment.thumbUri}
              alt="Screenshot to attach"
            />
            <span className="attachment-meta">
              {attachment.width}×{attachment.height}
            </span>
            <button
              type="button"
              className="attachment-remove"
              aria-label="Remove screenshot"
              disabled={busy}
              onClick={handleRemoveAttachment}
            >
              ✕
            </button>
          </div>
          {!vision && (
            <div className="attachment-hint">{CAPTURE_NEEDS_VISION}</div>
          )}
        </div>
      )}

      <div className="content">
        {error && (
          <div className="error" role="alert">
            {error}
          </div>
        )}
        {/* Always mounted: a live region that unmounts in the resolve commit
            can never announce, so the row persists — visible as the status
            line while busy, visually hidden while carrying the one-shot
            answer-ready announcement (or nothing). */}
        <div className={activeStatus ? "status" : "visually-hidden"}>
          {/* The live region wraps the label + hint only — with Stop inside,
              every phase change would re-announce a "Stop" button. */}
          <div role="status">
            {activeStatus ? (
              <>
                {STATUS_LABEL[activeStatus]}
                {slowHint && isWikiBoundStatus(activeStatus) && (
                  <div className="status-hint">
                    The wiki is responding slowly — this isn't WikiLens.
                  </div>
                )}
              </>
            ) : (
              announcement
            )}
          </div>
          {activeStatus && (
            <button
              type="button"
              className="status-stop"
              // Keep the pointer press from stealing focus off the prompt —
              // the (a) invariant; handleStop refocuses for keyboard users.
              onMouseDown={(e) => e.preventDefault()}
              onClick={handleStop}
            >
              Stop
            </button>
          )}
        </div>
        {/* Not gated on error: a mid-stream failure keeps the partial that
            already streamed (submit cleared `answer`, so it's this ask's own
            text). Sources are cleared on submit and set only on success, so
            the list self-hides under an error. */}
        {answer && <AnswerView markdown={answer} />}
        <SourceList sources={sources} />
        {showPlaceholder && (
          <div className="placeholder">
            Press{" "}
            {summonKeys.map((part, i) => (
              <Fragment key={i}>
                {i > 0 && "+"}
                <kbd>{part}</kbd>
              </Fragment>
            ))}{" "}
            anytime to open this panel. Ask a question and WikiLens answers
            straight from the game's wiki.
          </div>
        )}
      </div>

      <footer className="panel-footer">
        {provider && (
          <ModelChip
            providerName={provider.name}
            modelLabel={modelPick?.label ?? provider.defaultModelLabel}
            open={openMenu === "model"}
            disabled={busy}
            // Same closeMenu() rule as the game chip (focus contract).
            onToggle={() =>
              openMenu === "model" ? closeMenu() : setOpenMenu("model")
            }
            buttonRef={chipRef}
          />
        )}
        <div className="capture-cluster">
          {debugChip && (
            <button
              type="button"
              className="quiet-chip debug-chip"
              // Never disabled while busy: mid-ask is exactly when the debug
              // window is worth opening, and the command only touches window
              // visibility.
              onClick={() => void toggleDebugWindow().catch(() => {})}
              title="Show or hide the debug panel"
            >
              <span className="chip-name">Debug</span>
            </button>
          )}
          {!vision && <span className="chip-hint">text-only model</span>}
          <button
            type="button"
            className="quiet-chip capture-chip"
            onClick={handleCaptureRequest}
            disabled={busy || !vision}
            title={
              vision
                ? `Capture a screenshot (${captureLabel})`
                : "This model can't read images"
            }
          >
            <span className="chip-name">Capture</span>
          </button>
        </div>
      </footer>

      {/* Menus are direct children of .panel — .content's overflow would clip them. */}
      {openMenu === "model" && provider && (
        <ModelMenu
          providers={providers}
          selected={{
            providerId: selectedProvider,
            modelId: modelPick?.id ?? provider.defaultModel,
          }}
          onSelect={handleModelSelect}
          onClose={closeMenu}
          chipRef={chipRef}
        />
      )}
      {openMenu === "game" && (
        <GameMenu
          games={games}
          selectedId={selectedGame}
          recentIds={recentGames}
          onSelect={(id) => {
            handleGameChange(id);
            closeMenu();
          }}
          // Direct menu swap — no closeMenu(), whose prompt refocus would
          // fight the add-game menu's own autofocus.
          onAddGame={() => setOpenMenu("addGame")}
          onClose={closeMenu}
          chipRef={gameChipRef}
        />
      )}
      {openMenu === "addGame" && (
        <AddGameMenu
          games={games}
          onClose={closeMenu}
          onAdded={handleGameAdded}
          onRemoved={handleGameRemoved}
          triggerRef={gameChipRef}
        />
      )}
      {openMenu === "settings" && settings && (
        <SettingsMenu
          settings={settings}
          onSaved={setSettings}
          onClose={closeMenu}
          triggerRef={settingsChipRef}
        />
      )}
    </div>
  );
}

export default App;
