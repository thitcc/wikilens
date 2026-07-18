// The debug window's root: subscribes to the debug:// stream (plus the
// overlay's ask://status broadcast for liveness) and renders one card per
// ask, newest first. Listen-only — this page never invokes a command.

import { useEffect, useReducer } from "react";

import "./styles.css";
import AskCard from "./AskCard";
import { INITIAL_STATE, reduce } from "./askState";
import {
  onAskStatus,
  onDebugAskStarted,
  onDebugCandidates,
  onDebugFinished,
  onDebugPages,
  onDebugPhase,
  onDebugUsage,
} from "./events";

export default function DebugApp() {
  const [state, dispatch] = useReducer(reduce, INITIAL_STATE);

  // Subscribe for the lifetime of the page (App.tsx's register idiom: a
  // resolved unlisten is stored and called synchronously on unmount; one that
  // resolves after unmount disposes itself).
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];
    const register = (unlisten: () => void) => {
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    };

    void onDebugAskStarted((payload) =>
      dispatch({ type: "ask-started", payload }),
    ).then(register);
    void onDebugPhase((payload) => dispatch({ type: "phase", payload })).then(
      register,
    );
    void onDebugCandidates((payload) =>
      dispatch({ type: "candidates", payload }),
    ).then(register);
    void onDebugUsage((payload) => dispatch({ type: "usage", payload })).then(
      register,
    );
    void onDebugPages((payload) => dispatch({ type: "pages", payload })).then(
      register,
    );
    void onDebugFinished((payload) =>
      dispatch({ type: "finished", payload }),
    ).then(register);
    void onAskStatus((payload) => dispatch({ type: "status", payload })).then(
      register,
    );

    return () => {
      disposed = true;
      for (const cleanup of cleanups) cleanup();
    };
  }, []);

  return (
    <main className="debug-panel">
      {/* "deep": the whole header subtree drags the undecorated window
          (clickable elements would still block). Needs the start-dragging
          permission in capabilities/debug.json — silently inert without it. */}
      <header className="debug-header" data-tauri-drag-region="deep">
        <span className="debug-brand">WikiLens debug</span>
        <span className="debug-count">
          {state.asks.length} ask{state.asks.length === 1 ? "" : "s"}
        </span>
      </header>
      <div className="debug-scroll">
        {state.asks.length === 0 ? (
          <p className="debug-empty">
            Waiting for the first ask… (asks appear here live while
            WIKILENS_DEBUG is on)
          </p>
        ) : (
          state.asks.map((ask) => <AskCard key={ask.askId} ask={ask} />)
        )}
      </div>
    </main>
  );
}
