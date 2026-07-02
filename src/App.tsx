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
import type { AskStatus, GameInfo, ProviderInfo, Source } from "./types";
import { GamePicker } from "./components/GamePicker";
import { ProviderPicker } from "./components/ProviderPicker";
import { PromptInput } from "./components/PromptInput";
import { AnswerView } from "./components/AnswerView";
import { SourceList } from "./components/SourceList";
import "./styles.css";

const GAME_STORAGE_KEY = "wikilens.selectedGame";
const PROVIDER_STORAGE_KEY = "wikilens.selectedProvider";

const STATUS_LABEL: Record<AskStatus, string> = {
  searching: "Searching the wiki…",
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
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [sources, setSources] = useState<Source[]>([]);
  const [status, setStatus] = useState<AskStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const inputRef = useRef<HTMLTextAreaElement | null>(null);

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

  // Subscribe to backend events for the lifetime of the app.
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];
    const register = (unlisten: () => void) => {
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    };

    void onOverlayShown(() => {
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
  }

  function handleProviderChange(providerId: string) {
    setSelectedProvider(providerId);
    localStorage.setItem(PROVIDER_STORAGE_KEY, providerId);
  }

  async function handleSubmit() {
    const trimmed = question.trim();
    if (busy || !trimmed || !selectedGame || !selectedProvider) return;

    setBusy(true);
    setError(null);
    setAnswer("");
    setSources([]);
    setStatus("searching");

    try {
      const result = await ask(selectedGame, selectedProvider, trimmed);
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
        <div className="picker-group">
          <ProviderPicker
            providers={providers}
            value={selectedProvider}
            onChange={handleProviderChange}
            disabled={busy}
          />
          <GamePicker
            games={games}
            value={selectedGame}
            onChange={handleGameChange}
            disabled={busy}
          />
        </div>
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
    </div>
  );
}

export default App;
