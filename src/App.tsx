import { Fragment, useEffect, useRef, useState } from "react";
import {
  ASK_CANCELLED,
  ask,
  beginCapture,
  cancelAsk,
  clearCapture,
  clearHistory,
  debugAvailable,
  getSettings,
  hideOverlay,
  listGames,
  listHistory,
  listModels,
  listProviders,
  onAskDelta,
  onAskStatus,
  onCaptureAttached,
  onCaptureError,
  onCaptureHotkey,
  onOverlayHidden,
  onOverlayShown,
  PANEL_HEIGHT_UNBOUNDED,
  setOverlayHeight,
  showOverlay,
  toggleDebugWindow,
} from "./api";
import type {
  AskStatus,
  AttachmentInfo,
  GameInfo,
  HistoryEntry,
  Mode,
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
import { GameSuggestion } from "./components/GameSuggestion";
import { HistoryMenu } from "./components/HistoryMenu";
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

/** Bootstrap Icons "arrow-counterclockwise" (MIT), sized like the header
 * gear. Static, so it lives at module scope rather than being rebuilt on
 * every render. */
const RESET_ICON = (
  <svg
    aria-hidden="true"
    width="12"
    height="12"
    viewBox="0 0 16 16"
    fill="currentColor"
  >
    <path
      fillRule="evenodd"
      d="M8 3a5 5 0 1 1-4.546 2.914.5.5 0 0 0-.908-.417A6 6 0 1 0 8 2z"
    />
    <path d="M8 4.466V.534a.25.25 0 0 0-.41-.192L5.23 2.308a.25.25 0 0 0 0 .384l2.36 1.966A.25.25 0 0 0 8 4.466" />
  </svg>
);

/** Bootstrap Icons "clock-history" (MIT), sized like the header gear: a
 * clock face with a dashed trailing arc — "where time went", not settings. */
const HISTORY_ICON = (
  <svg
    aria-hidden="true"
    width="12"
    height="12"
    viewBox="0 0 16 16"
    fill="currentColor"
  >
    <path d="M8.515 1.019A7 7 0 0 0 8 1V0a8 8 0 0 1 .589.022zm2.004.45a7 7 0 0 0-.985-.299l.219-.976q.576.129 1.126.342zm1.37.71a7 7 0 0 0-.439-.27l.493-.87a8 8 0 0 1 .979.654l-.615.789a7 7 0 0 0-.418-.302zm1.834 1.79a7 7 0 0 0-.653-.796l.724-.69q.406.429.747.91zm.744 1.352a7 7 0 0 0-.214-.468l.893-.45a8 8 0 0 1 .45 1.088l-.95.313a7 7 0 0 0-.179-.483m.53 2.507a7 7 0 0 0-.1-1.025l.985-.17q.1.58.116 1.17zm-.131 1.538q.05-.254.081-.51l.993.123a8 8 0 0 1-.23 1.155l-.964-.267q.069-.247.12-.501m-.952 2.379q.276-.436.486-.908l.914.405q-.24.54-.555 1.038zm-.964 1.205q.183-.183.35-.378l.758.653a8 8 0 0 1-.401.432z" />
    <path d="M8 1a7 7 0 1 0 4.95 11.95l.707.707A8.001 8.001 0 1 1 8 0z" />
    <path d="M7.5 3a.5.5 0 0 1 .5.5v5.21l3.248 1.856a.5.5 0 0 1-.496.868l-3.5-2A.5.5 0 0 1 7 9V3.5a.5.5 0 0 1 .5-.5" />
  </svg>
);

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

/** The Default-mode variant: there is no model menu to pick from. Keeps the
 * same leading clause (test suites match on it). */
const CAPTURE_NEEDS_VISION_DEFAULT =
  "This model can't read images — remove the screenshot to ask.";

/** The footer's word for the Default model source — the single constant the
 * whole frontend renders for it (mirrors Rust `target::DEFAULT_TARGET_NAME`). */
const DEFAULT_MODE_LABEL = "Default";

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

/** Backstop for the frame-synced summon arm: if the double-rAF wait somehow
 * never fires, this timer arms the entrance anyway so the pre-summon hold
 * can't leave the panel invisible. Exported for the fake-timer tests. */
export const SUMMON_ARM_FALLBACK_MS = 100;

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
  // The game Rust identified under the panel on the last summon. A suggestion
  // only — nothing reads it except the header chip below.
  const [detectedGame, setDetectedGame] = useState<string | null>(null);
  // At most one popover at a time — their capture-phase Esc handlers would
  // otherwise stack, and one Esc would close both.
  const [openMenu, setOpenMenu] = useState<
    "game" | "addGame" | "model" | "settings" | "history" | null
  >(null);
  // Past answered asks (list_history, newest first) — gates the header
  // History chip and feeds its menu. Refreshed after every submit settles.
  const [history, setHistory] = useState<HistoryEntry[]>([]);
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
  // A-01 (DESIGN.md §6): the webview resumes presenting frames a beat after
  // win.show(), so arming on overlay://shown lets the 120ms clock run before
  // anything reaches the screen — the entrance degrades to a pop-in with an
  // end wobble on every summon after the first. preSummon holds the panel in
  // the keyframe's "from" state from hide (and mount — the window starts
  // hidden) until the frame-synced arm below; summoning then plays the
  // entrance, dropped on the animation's end (name-filtered — child
  // animationends bubble here) and on hide. Paint-only: focus never waits
  // on either.
  const [summoning, setSummoning] = useState(false);
  const [preSummon, setPreSummon] = useState(true);
  // The configured shortcuts (null until get_settings resolves — the copy
  // below falls back to the shipped defaults, and the gear stays disabled).
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  // True once get_settings settled either way — the provider fetch waits for
  // the mode to be known (a failed get_settings settles as Custom).
  const [settingsSettled, setSettingsSettled] = useState(false);
  // True once list_providers settled — gates the zero-keys CTA (a bare empty
  // list can't distinguish "no keys" from "not fetched yet").
  const [providersSettled, setProvidersSettled] = useState(false);
  // Bumped when the Settings panel saves/removes a key → providers re-fetch.
  const [keysVersion, setKeysVersion] = useState(0);

  /** The effective model-source mode; never-chosen displays as Custom (the
   * same normalization SettingsMenu applies). */
  const mode: Mode = settings?.mode ?? "custom";

  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const gameChipRef = useRef<HTMLButtonElement | null>(null);
  const settingsChipRef = useRef<HTMLButtonElement | null>(null);
  const historyChipRef = useRef<HTMLButtonElement | null>(null);
  // Latest capture handler, so the mount-only hotkey listener always sees
  // current state (e.g. `busy`) instead of a stale mount-time closure.
  const requestCaptureRef = useRef<() => void>(() => {});
  // When the overlay last hid (overlay://hidden), for the select-all
  // suppression below. 0 = never, so the first show always selects.
  const lastHiddenAtRef = useRef(0);
  // Token for the pending summon arm: bumped on hide to cancel a wait still
  // in flight, and by the arm itself so the slower of its two paths
  // (double-rAF vs the backstop timer) no-ops.
  const summonArmRef = useRef(0);
  // Window-height reporting (set_overlay_height): panel + .content + the
  // .content-sizer wrapper feed reportOverlayHeight; openMenuRef mirrors
  // state for the mount-only observer; lastHeightRef dedupes (±1px DPI
  // tolerance); heightTimerRef is the trailing throttle.
  const panelRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const contentSizerRef = useRef<HTMLDivElement | null>(null);
  const openMenuRef = useRef<
    "game" | "addGame" | "model" | "settings" | "history" | null
  >(null);
  const lastHeightRef = useRef<number | null>(null);
  const heightTimerRef = useRef<number | null>(null);
  // Bumped by Start over: an ask that settles after a clear must not
  // resurrect its answer or error. Stop's "never reset state here" rule
  // holds — the resolve path checks the epoch instead.
  const askEpochRef = useRef(0);
  // The epoch the in-flight ask was submitted under. The ask://delta and
  // ask://status listeners compare it to askEpochRef before applying: a
  // chunk already in flight when Start over wipes the panel must not
  // repaint orphan text on it (deltas keep draining until the abort lands).
  const streamEpochRef = useRef(0);

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

  // The keyed-provider list — Custom mode only, once the mode is known
  // (settings settled; a failed get_settings settles as Custom). Re-runs when
  // the panel saves/removes a key (keysVersion) and on a mode flip back to
  // Custom — Default mode never calls list_providers at all.
  useEffect(() => {
    if (!settingsSettled || mode !== "custom") return;
    let active = true;
    listProviders()
      .then((list) => {
        if (!active) return;
        setProviders(list);
        setProvidersSettled(true);
        setSelectedProvider((current) => {
          // Prefer the stored id when state was zeroed by an earlier empty
          // list — a removed key's pick revives with the key.
          const want =
            current || localStorage.getItem(PROVIDER_STORAGE_KEY) || "";
          if (want && list.some((p) => p.id === want)) return want;
          return list[0]?.id ?? "";
        });
      })
      .catch((e) => {
        if (!active) return;
        setProvidersSettled(true);
        setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [settingsSettled, mode, keysVersion]);

  // Load the configured shortcuts once. A failure is non-fatal: the copy
  // falls back to the shipped defaults and the gear stays disabled — and it
  // settles the mode as Custom, so the provider fetch above still runs.
  useEffect(() => {
    let active = true;
    getSettings()
      .then((info) => {
        if (!active) return;
        setSettings(info);
        setSettingsSettled(true);
      })
      .catch(() => {
        if (active) setSettingsSettled(true);
      });
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

  // Past answered asks, once on mount — the History chip renders only when
  // some exist. A failed fetch just means no chip — never an error box.
  useEffect(() => {
    let active = true;
    listHistory()
      .then((list) => {
        if (active) setHistory(list);
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
  // returning users instead of wrongly treating them as text-only. Custom mode
  // only, and only once the mode is actually known — a stale pick must never
  // fire a vendor-shaped fetch in (or racing into) Default mode.
  useEffect(() => {
    if (
      !settingsSettled ||
      mode !== "custom" ||
      !selectedProvider ||
      !modelPick ||
      typeof modelPick.vision === "boolean"
    ) {
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
  }, [settingsSettled, mode, selectedProvider, modelPick]);

  // Subscribe to backend events for the lifetime of the app.
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];
    const register = (unlisten: () => void) => {
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    };

    // A-01 arm: wait two rAFs after overlay://shown — the first presented
    // frame proves the resumed webview is actually on screen — then start
    // the entrance from its held "from" state. The backstop timer covers a
    // webview that never pumps rAF; whichever fires first consumes the
    // token, and a hide mid-wait cancels both.
    const armSummon = () => {
      const token = ++summonArmRef.current;
      // Re-assert the hold (a no-op on the normal hide→show path): the wait
      // must never present the at-rest panel.
      setPreSummon(true);
      setSummoning(false);
      const arm = () => {
        if (summonArmRef.current !== token) return;
        summonArmRef.current += 1;
        setPreSummon(false);
        setSummoning(true);
      };
      requestAnimationFrame(() => requestAnimationFrame(arm));
      window.setTimeout(arm, SUMMON_ARM_FALLBACK_MS);
    };

    // Dev-serve only: an HMR reload while the window is visible gets no
    // overlay://shown, and the mount-time hold would keep the panel
    // invisible until the next hide+show. Arm now instead. undefined in
    // production builds — but NOT under Vitest, where import.meta.hot is
    // defined (measured) even though no reload happened, so every mount
    // armed and scheduled the fallback timer. With fake timers set to
    // advance with real time, that timer could fire mid-render under load
    // and clear the mount-time hold the summon suite asserts, making it
    // flaky in proportion to how many other suites were running.
    if (
      import.meta.hot &&
      import.meta.env.MODE !== "test" &&
      document.visibilityState === "visible"
    ) {
      armSummon();
    }

    void onOverlayShown((info) => {
      setOpenMenu(null);
      // Store, never apply: the chip is offered, and only a click switches.
      // A plain setState is why this listener needs no ref indirection —
      // the detection is validated against `games` at render time, where the
      // list is current, instead of inside this mount-time closure where it
      // would still be empty.
      setDetectedGame(info.detectedGame);
      armSummon();
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
      // Close any menu now, not on the next show: a menu holds the window at
      // the 70% cap, and closing while hidden lets the shrink-to-content
      // happen invisibly instead of on re-summon.
      setOpenMenu(null);
      // Cancel a pending arm and re-hold the panel for the next entrance.
      // Reduced-motion never fires animationend; a mid-animation hide
      // shouldn't leave the class armed either.
      summonArmRef.current += 1;
      setSummoning(false);
      setPreSummon(true);
    }).then(register);
    // Both stream listeners are gated on the clear epoch: after Start over,
    // stragglers from the aborted ask are dropped instead of resurrecting
    // text or the status row on the cleared panel. A plain Stop keeps
    // streaming until the abort drains — the kept-partial behavior.
    void onAskStatus((s) => {
      if (streamEpochRef.current === askEpochRef.current) setStatus(s);
    }).then(register);
    void onAskDelta((chunk) => {
      if (streamEpochRef.current === askEpochRef.current) {
        setAnswer((prev) => prev + chunk);
      }
    }).then(register);
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

  /** Report the panel's desired height to Rust (which clamps to the 70% cap
   * and resizes the window). Menu open → the unbounded sentinel: menus render
   * into the window space around the panel and assume the cap. Otherwise the
   * panel's border box plus `.content`'s scrollback — once the window hugs,
   * `.panel`'s max-height equals its own height and streamed growth lives
   * only in the scrollback, so the border box alone would go silent. The
   * ±1px tolerance absorbs the fractional-DPI echo of our own resize. */
  function reportOverlayHeight() {
    const panel = panelRef.current;
    const content = contentRef.current;
    if (!panel || !content) return;
    const desired =
      openMenuRef.current !== null
        ? PANEL_HEIGHT_UNBOUNDED
        : Math.ceil(
            panel.getBoundingClientRect().height +
              Math.max(0, content.scrollHeight - content.clientHeight),
          );
    const last = lastHeightRef.current;
    if (last !== null && Math.abs(desired - last) <= 1) return;
    // Optimistic (collapses bursts), but re-armed on failure: a swallowed
    // report must not convince the dedupe it landed — a lost menu-open
    // sentinel would leave the menu clipped with no resize to correct it.
    lastHeightRef.current = desired;
    void setOverlayHeight(desired).catch(() => {
      lastHeightRef.current = null;
    });
  }

  // Mount + every menu open/close. Opening must pin the window at the cap
  // (the OS resize lands async — ModelMenu re-measures on `resize`); closing
  // hugs the content again. openMenu starts null, so this is also the
  // initial mount report.
  useEffect(() => {
    reportOverlayHeight();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openMenu]);

  // Content growth/shrink → resize the window. Three targets, one observer:
  // .panel (attachment strip mounts), .content (squeezed by siblings while
  // the panel sits at max-height), and .content-sizer (an answer streaming
  // past the cap grows only scrollback — the outer two boxes go silent
  // there). Trailing 100ms throttle: ≤10 IPC/s while streaming, measured at
  // fire time so the last report always sees the settled layout.
  useEffect(() => {
    const observer = new ResizeObserver(() => {
      if (heightTimerRef.current !== null) return;
      heightTimerRef.current = window.setTimeout(() => {
        heightTimerRef.current = null;
        reportOverlayHeight();
      }, 100);
    });
    for (const el of [
      panelRef.current,
      contentRef.current,
      contentSizerRef.current,
    ]) {
      if (el) observer.observe(el);
    }
    return () => {
      observer.disconnect();
      if (heightTimerRef.current !== null) {
        window.clearTimeout(heightTimerRef.current);
        heightTimerRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
  // Default mode: the env-declared flag (opt-in, text-only unless set);
  // Custom mode: the pick/provider resolution.
  const vision =
    mode === "default"
      ? (settings?.defaultMode.vision ?? false)
      : activeModelVision(provider, modelPick);
  // The matching "why not" copy (Default mode has no menu to pick from).
  const captureVisionCopy =
    mode === "default" ? CAPTURE_NEEDS_VISION_DEFAULT : CAPTURE_NEEDS_VISION;

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
      setError(captureVisionCopy);
      return;
    }
    setOpenMenu(null);
    void beginCapture().catch((e) => setError(String(e)));
  }
  // Point the mount-only hotkey listener at the current closure each render.
  requestCaptureRef.current = handleCaptureRequest;
  // Mirror for the mount-only ResizeObserver's report closure.
  openMenuRef.current = openMenu;

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

  // The header's Start over: back to the default panel — draft, answer,
  // sources, error, announcement, and screenshot all go; a running ask is
  // stopped. Unlike Stop, this DOES reset state: the epoch bump makes an ask
  // that settles afterwards drop its own result instead of clobbering the
  // cleared panel (the resolve path checks it).
  function handleClear() {
    askEpochRef.current += 1;
    if (busy) void cancelAsk().catch(() => {});
    setQuestion("");
    setAnswer("");
    setSources([]);
    setError(null);
    setStatus(null);
    setAnnouncement(null);
    setSlowHint(false);
    setAttachment(null);
    // Same rule as the attachment ✕: a failed Rust clear is harmless.
    void clearCapture().catch(() => {});
    inputRef.current?.focus();
  }

  // Restore a past answer without re-asking: the panel looks exactly as it
  // did when that answer landed — question in the prompt (Enter re-asks it),
  // answer + sources below. The epoch bump makes a delta still draining
  // after a Stop drop instead of appending onto the restored answer (the
  // same gate as Start over); the chip is disabled while busy, so no
  // in-flight ask can race this.
  function handleHistoryPick(entry: HistoryEntry) {
    askEpochRef.current += 1;
    setQuestion(entry.question);
    setAnswer(entry.answer);
    setSources(entry.sources);
    setError(null);
    setStatus(null);
    setAnnouncement(null);
    setSlowHint(false);
    // The attachment stays: it belongs to the player's next ask, not to the
    // restored answer. Switch to the entry's game so a re-ask goes to the
    // right wiki; a removed game leaves the selection untouched (the row
    // still named it via the denormalized gameName).
    if (games.some((g) => g.id === entry.gameId)) {
      handleGameChange(entry.gameId);
    }
    closeMenu();
  }

  // The history menu's pinned action. On success the chip unmounts (empty
  // history), so close the menu with it; a rejection propagates to the menu,
  // which shows it inline and stays open.
  function handleHistoryClear(): Promise<void> {
    return clearHistory().then(() => {
      setHistory([]);
      closeMenu();
    });
  }

  async function handleSubmit() {
    const trimmed = question.trim();
    if (busy || !trimmed || !selectedGame) return;
    // Custom mode needs a provider (zero keyed providers blocks here);
    // Default mode resolves its target Rust-side and ignores these args.
    if (mode !== "default" && !selectedProvider) return;
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
    // Start over invalidates this ask's right to publish its result — and
    // its stream (the delta/status listeners compare these two refs).
    const epoch = askEpochRef.current;
    streamEpochRef.current = epoch;

    try {
      // Custom mode: the explicit pick when there is one, else the provider
      // default (a blank value would fall back Rust-side; this is the same
      // rule applied eagerly so the chip and the request always agree).
      // Default mode: both args blank — run_ask ignores them by contract.
      const providerId = mode === "default" ? "" : selectedProvider;
      const model =
        mode === "default" ? "" : (modelPick?.id ?? provider?.defaultModel ?? "");
      const result = await ask(
        selectedGame,
        providerId,
        model,
        trimmed,
        attachment?.id,
      );
      // A Start over while we awaited owns the panel now — drop the result.
      if (epoch !== askEpochRef.current) return;
      setAnswer(result.answer);
      setSources(result.sources);
      setAnnouncement(answerReadyLabel(result.sources.length));
      // Clears-on-success, mirroring the Rust slot: the model answered, so the
      // screenshot is spent. A failed ask keeps it (this line isn't reached).
      setAttachment(null);
    } catch (e) {
      // A cancelled ask resets quietly: no error box, and whatever partial
      // answer already streamed stays on screen. (After a plain Stop, a delta
      // racing the abort may still append — harmless, it lands on the kept
      // text; after Start over the stream gate drops it.) A cleared ask
      // (epoch moved) swallows its error the same way.
      if (epoch === askEpochRef.current && String(e) !== ASK_CANCELLED) {
        setError(String(e));
      }
    } finally {
      // Unconditional: busy/status describe the in-flight request, not the
      // panel content — a post-clear settle must still release them.
      setBusy(false);
      setStatus(null);
      // Refresh the History chip's list — Rust records answered asks only,
      // so re-listing beats guessing (a cancelled/failed ask refreshes to an
      // unchanged list, and an epoch-dropped answer still shows up here).
      listHistory()
        .then(setHistory)
        .catch(() => {});
    }
  }

  const showPlaceholder = !error && !busy && !answer;
  // Start over renders only when there's something to start over from — the
  // same "no affordance without an action" rule as SettingsMenu's Reset.
  const clearable =
    question !== "" || answer !== "" || error !== null || attachment !== null || busy;
  // The phase to display, null outside a healthy busy ask. A narrowed value
  // (not a boolean) so STATUS_LABEL[activeStatus] type-checks in the JSX.
  const activeStatus = !error && busy ? status : null;
  // Configured shortcut copy, falling back to the shipped defaults until
  // get_settings resolves (or if it failed).
  const summonKeys = labelParts(
    settings?.hotkeys.summon.accelerator ?? DEFAULT_SUMMON_LABEL,
  );
  const captureLabel = settings?.hotkeys.capture.label ?? DEFAULT_CAPTURE_LABEL;
  // The game-detection offer, derived rather than stored — which is what keeps
  // it stateless: there is nothing to dismiss, nothing to clear on hide, and
  // no memory of what the player already declined. It resolves to nothing when
  // the detection matches the current pick, when the id isn't a game we can
  // offer (a removed user wiki, a stale rule), when nothing was detected, and
  // while an ask is in flight — switching mid-stream would leave the chip
  // disagreeing with the answer's own sources (the `disabled={busy}` rule the
  // game chip already follows).
  const suggestedGame =
    detectedGame && detectedGame !== selectedGame && !busy
      ? games.find((g) => g.id === detectedGame)
      : undefined;

  return (
    <div
      ref={panelRef}
      className={
        "panel" +
        (preSummon ? " panel--pre-summon" : "") +
        (summoning ? " panel--summoning" : "")
      }
      onAnimationEnd={(e) => {
        if (e.animationName === "panel-summon") setSummoning(false);
      }}
    >
      <header className="panel-header">
        {/* The gear rides with the brand (the game chip owns the right edge).
            Disabled until get_settings resolves — the popover needs data. */}
        <span className="brand-cluster">
          <span className="brand">WikiLens</span>
          <button
            type="button"
            ref={settingsChipRef}
            className={
              "quiet-chip icon-chip settings-chip" +
              (openMenu === "settings" ? " is-open" : "")
            }
            disabled={busy || !settings}
            aria-haspopup="dialog"
            aria-expanded={openMenu === "settings"}
            aria-label="Settings"
            title="Settings"
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
          {history.length > 0 && (
            <button
              type="button"
              ref={historyChipRef}
              className={
                "quiet-chip icon-chip history-chip" +
                (openMenu === "history" ? " is-open" : "")
              }
              // Disabled while busy (the gear rule): restoring mid-ask would
              // fight the stream for the panel.
              disabled={busy}
              aria-haspopup="dialog"
              aria-expanded={openMenu === "history"}
              aria-label="History"
              title="History"
              // Same closeMenu() rule as the game chip (focus contract).
              onClick={() =>
                openMenu === "history" ? closeMenu() : setOpenMenu("history")
              }
            >
              {HISTORY_ICON}
            </button>
          )}
          {clearable && (
            <button
              type="button"
              className="quiet-chip icon-chip reset-chip"
              aria-label="Start over"
              title="Start over"
              // Keep the pointer press from stealing focus off the prompt —
              // the same invariant as Stop; handleClear refocuses for
              // keyboard users. Deliberately NOT disabled while busy: its
              // whole job includes abandoning a running ask.
              onMouseDown={(e) => e.preventDefault()}
              onClick={handleClear}
            >
              {RESET_ICON}
            </button>
          )}
        </span>
        {/* The suggestion sits left of the game chip it would fill; both keep
            the header's right edge (the header is space-between). */}
        <span className="game-cluster">
          {suggestedGame && (
            <GameSuggestion
              gameName={suggestedGame.name}
              onAccept={() => {
                handleGameChange(suggestedGame.id);
                // The click focused the chip, which then unmounts (the
                // suggestion now matches the selection) — focus would drop to
                // <body> and deaden the keyboard. Same contract as closeMenu().
                inputRef.current?.focus();
              }}
            />
          )}
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
        </span>
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
            <div className="attachment-hint">{captureVisionCopy}</div>
          )}
        </div>
      )}

      <div className="content" ref={contentRef}>
        {/* .content-sizer wraps the scrollable children so the height
            reporter can observe natural content growth even while .content
            itself is pinned and scrolling (see reportOverlayHeight). Menus
            are NOT here — they stay direct .panel children. */}
        <div className="content-sizer" ref={contentSizerRef}>
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
      </div>

      <footer className="panel-footer">
        {mode === "default" ? (
          // Static, non-interactive: the vendor never surfaces; the
          // explanation lives in the Settings panel's Model source section.
          <span className="quiet-chip static-chip">{DEFAULT_MODE_LABEL}</span>
        ) : provider ? (
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
        ) : providersSettled && providers.length === 0 ? (
          // Zero keyed providers: the dead-end state gets a way out. A second
          // click while Settings is open just re-opens it — harmless.
          <button
            type="button"
            className="quiet-chip"
            disabled={busy || !settings}
            onClick={() => setOpenMenu("settings")}
          >
            Set up a model
          </button>
        ) : null}
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
      {openMenu === "model" && mode === "custom" && provider && (
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
          onKeysChanged={() => setKeysVersion((v) => v + 1)}
          onClose={closeMenu}
          triggerRef={settingsChipRef}
        />
      )}
      {openMenu === "history" && (
        <HistoryMenu
          entries={history}
          onPick={handleHistoryPick}
          onClear={handleHistoryClear}
          onClose={closeMenu}
          chipRef={historyChipRef}
        />
      )}
    </div>
  );
}

export default App;
