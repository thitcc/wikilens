import { useEffect, useRef, useState } from "react";
import {
  ask,
  hideOverlay,
  listGames,
  listProviders,
  onAskDelta,
  onAskStatus,
  onOverlayShown,
} from "./api";
import type {
  AskStatus,
  GameInfo,
  ModelInfo,
  ProviderInfo,
  Source,
} from "./types";
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
const PROVIDER_STORAGE_KEY = "wikilens.selectedProvider";
const MODEL_STORAGE_PREFIX = "wikilens.selectedModel.";

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

/** The user's explicit model pick for a provider, or null when they've never
 * picked one. Stored as JSON `{id, label}` so the chip can label itself
 * without any list fetch (offline included). */
function storedModel(providerId: string): ModelInfo | null {
  if (!providerId) return null;
  try {
    const raw = localStorage.getItem(MODEL_STORAGE_PREFIX + providerId);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      typeof (parsed as ModelInfo).id === "string" &&
      typeof (parsed as ModelInfo).label === "string"
    ) {
      const model = parsed as ModelInfo;
      return { id: model.id, label: model.label };
    }
  } catch {
    // Corrupted entry — fall through to the provider default.
  }
  return null;
}

const STATUS_LABEL: Record<AskStatus, string> = {
  searching: "Searching the wiki…",
  retrying: "Broadening the search…",
  reading: "Reading pages…",
  answering: "Answering…",
};

function App() {
  const [games, setGames] = useState<GameInfo[]>([]);
  const [selectedGame, setSelectedGame] = useState<string>(
    () => localStorage.getItem(GAME_STORAGE_KEY) ?? "",
  );
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>(
    () => localStorage.getItem(PROVIDER_STORAGE_KEY) ?? "",
  );
  const [modelPick, setModelPick] = useState<ModelInfo | null>(() =>
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
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const gameChipRef = useRef<HTMLButtonElement | null>(null);

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
  // the provider changes (including the initial load/validation above).
  useEffect(() => {
    setModelPick(storedModel(selectedProvider));
  }, [selectedProvider]);

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

  async function handleSubmit() {
    const trimmed = question.trim();
    if (busy || !trimmed || !selectedGame || !selectedProvider) return;

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
      const result = await ask(selectedGame, selectedProvider, model, trimmed);
      setAnswer(result.answer);
      setSources(result.sources);
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

      <div className="content">
        {error && <div className="error">{error}</div>}
        {!error && busy && status && (
          <div className="status">{STATUS_LABEL[status]}</div>
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
