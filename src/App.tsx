import { useEffect, useRef, useState } from "react";
import {
  ask,
  beginCapture,
  clearCapture,
  hideOverlay,
  listGames,
  listModels,
  listProviders,
  onAskDelta,
  onAskStatus,
  onCaptureAttached,
  onCaptureError,
  onCaptureHotkey,
  onOverlayShown,
} from "./api";
import type {
  AskStatus,
  AttachmentInfo,
  GameInfo,
  ModelInfo,
  ProviderInfo,
  Source,
  StoredModelPick,
} from "./types";
import {
  MODEL_STORAGE_PREFIX,
  PROVIDER_STORAGE_KEY,
  activeModelVision,
  sameModelPick,
  storedModel,
} from "./modelPick";
import { AddGameMenu } from "./components/AddGameMenu";
import { GameChip } from "./components/GameChip";
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

const STATUS_LABEL: Record<AskStatus, string> = {
  searching: "Searching the wiki…",
  understanding: "Understanding your question…",
  retrying: "Broadening the search…",
  reading: "Reading pages…",
  answering: "Answering…",
};

/** How long one ask phase may run before the slow-wiki hint shows. Above the
 * healthy worst cases (search ≲3s, 4-page fetch ≲8s), below the backend's 12s
 * per-request fetch timeout. Exported for the fake-timer tests. */
export const SLOW_WIKI_HINT_MS = 10_000;

/** Every phase except the LLM stream waits on the wiki. ("searching" also
 * joins the LLM rewrite, but only the wiki search there can run this long.) */
function isWikiBoundStatus(s: AskStatus): boolean {
  return s !== "answering";
}

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
    "game" | "addGame" | "model" | null
  >(null);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [sources, setSources] = useState<Source[]>([]);
  const [status, setStatus] = useState<AskStatus | null>(null);
  const [slowHint, setSlowHint] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [attachment, setAttachment] = useState<AttachmentInfo | null>(null);

  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const gameChipRef = useRef<HTMLButtonElement | null>(null);
  // Latest capture handler, so the mount-only hotkey listener always sees
  // current state (e.g. `busy`) instead of a stale mount-time closure.
  const requestCaptureRef = useRef<() => void>(() => {});

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
      inputRef.current?.select();
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
  // The `!vision` guard covers the hotkey path; the button is already disabled.
  function handleCaptureRequest() {
    if (busy || !vision) return;
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
      // Clears-on-success, mirroring the Rust slot: the model answered, so the
      // screenshot is spent. A failed ask keeps it (this line isn't reached).
      setAttachment(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setStatus(null);
    }
  }

  const showPlaceholder = !error && !busy && !answer;

  return (
    <div className="panel">
      <header className="panel-header">
        <span className="brand">WikiLens</span>
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
        disabled={busy}
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
            <div className="attachment-hint">
              This model can't read images — remove it or pick one with the Image
              badge.
            </div>
          )}
        </div>
      )}

      <div className="content">
        {error && <div className="error">{error}</div>}
        {!error && busy && status && (
          <div className="status">
            {STATUS_LABEL[status]}
            {slowHint && isWikiBoundStatus(status) && (
              <div className="status-hint">
                The wiki is responding slowly — this isn't WikiLens.
              </div>
            )}
          </div>
        )}
        {!error && answer && <AnswerView markdown={answer} />}
        {!error && <SourceList sources={sources} />}
        {showPlaceholder && (
          <div className="placeholder">
            Press <kbd>Shift</kbd>+<kbd>C</kbd> anytime to open this panel. Ask a
            question and WikiLens answers straight from the game's wiki.
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
          {!vision && <span className="chip-hint">text-only model</span>}
          <button
            type="button"
            className="quiet-chip capture-chip"
            onClick={handleCaptureRequest}
            disabled={busy || !vision}
            title={
              vision
                ? "Capture a screenshot (Ctrl+Shift+C)"
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
    </div>
  );
}

export default App;
