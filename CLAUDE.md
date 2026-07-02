# CLAUDE.md — WikiLens

## 1. Project summary

WikiLens is a **Windows** desktop overlay for wiki-heavy games. A global hotkey
(**Shift+C**) slides a glass panel in from the right edge; the player types a
question and gets an answer generated **only** from the game's MediaWiki content
(hotkey → panel → RAG over the wiki → streamed answer + sources). Works over
**borderless/windowed** games only — exclusive fullscreen covers the overlay.
Tauri v2 + Rust backend, Vite + React + TypeScript frontend.

## 2. Architecture map

```
wikilens/
├── index.html, vite.config.ts, tsconfig*.json   # Vite/TS config
├── src/                          # Frontend (React + TS)
│   ├── main.tsx                  # React entry
│   ├── App.tsx                   # Layout + state: header/prompt/answer, events, ask flow
│   ├── api.ts                    # ONLY bridge to Rust: invoke() + event listeners (typed)
│   ├── types.ts                  # Shared types: GameInfo, Source, AskResult, AskStatus, StreamEvent
│   ├── styles.css                # Transparent body + glass dark panel
│   └── components/
│       ├── GamePicker.tsx        # <select> of supported games
│       ├── PromptInput.tsx       # textarea; Enter submits, Shift+Enter = newline
│       ├── AnswerView.tsx        # streamed markdown (react-markdown; links open externally)
│       └── SourceList.tsx        # wiki source links (open in system browser)
└── src-tauri/                    # Backend (Rust) — run cargo commands here
    ├── tauri.conf.json           # window "overlay" (transparent, right-dock), CSP
    ├── capabilities/default.json # webview permissions (no http/fs)
    └── src/
        ├── main.rs               # thin entry → wikilens_lib::run()
        ├── lib.rs                # builder: plugins, tray, hotkey handler, commands, state
        ├── state.rs              # AppState: shared reqwest::Client + ask-in-progress flag
        ├── window.rs             # toggle/show/hide + right-edge, DPI-aware positioning
        ├── hotkey.rs             # Shift+C registration (release-safe)
        ├── tray.rs               # tray icon: Show/Hide, Quit
        ├── commands.rs           # #[tauri::command] ask / hide_overlay / list_games
        ├── error.rs              # AppError (thiserror) + Into<String>
        ├── llm.rs                # Anthropic Messages API (streaming SSE)
        └── wiki/{mod,games,search,fetch}.rs  # registry + MediaWiki search + plaintext extracts
```

**Data flow:** `hotkey → window toggle → frontend prompt → ask command → wiki
search/fetch → LLM stream → events → UI`.

## 3. Commands

Frontend/Tauri from repo root; `cargo` from `src-tauri/`:

- Dev app: `npm run tauri dev`
- Frontend build: `npm run build` · Type-check: `npx tsc --noEmit`
- Rust: `cd src-tauri && cargo check` · `cd src-tauri && cargo test`
- Release: `npm run tauri build`

## 4. Conventions

- **All HTTP happens in Rust.** The webview has no network/http/fs permissions
  (see `capabilities/default.json`).
- **API key:** `ANTHROPIC_API_KEY` env var, read Rust-side in `commands::ask`.
  Never log it, never send it to the frontend.
- **Frontend → Rust only via `src/api.ts`.** Components never import
  `@tauri-apps/*` directly.
- **Commands return `Result<T, String>`** with user-readable messages; internal
  fallible code uses `AppError` (`error.rs`), converted to `String` at the boundary.
- **Namespaced events** (payloads):
  - `overlay://shown` — `()`; frontend focuses the prompt input.
  - `ask://status` — `"searching" | "reading" | "answering"`.
  - `ask://delta` — `string` chunk of the streamed answer.
- **Adding a game** = one `GameWiki` entry in `wiki/games.rs`. Nothing else.
- Concurrency: `ask` rejects if one is already running (`AppState::ask_in_progress`).

## 5. Gotchas

- `transparent: true` needs `background: transparent` on `html, body` in
  `styles.css`, or the window renders as a black rectangle.
- Position with **monitor offset + scale factor** (see `window::position_right_edge`)
  — required for correct docking on multi-monitor / high-DPI setups.
- **Exclusive-fullscreen** games cover the overlay. Expected, not a bug.
- MediaWiki etiquette: keep the custom `User-Agent` (`wiki::USER_AGENT`); page
  fetches are a single batched request — don't parallelize them.
- Content extraction uses `prop=extracts` (*TextExtracts*). Wikis without that
  extension (Stardew Valley, Core Keeper here) return search hits but empty text;
  see the roadmap.
- The spec'd **Shift+C** is a bare Shift+letter global hotkey: on Windows it
  swallows Shift+C system-wide, so a capital `C` can't be typed into the prompt.
  Search is case-insensitive so lowercase works; prefer a Ctrl/Alt combo when the
  hotkey becomes configurable (see `hotkey.rs`).
- Opening links needs both the opener command **and** a URL scope: the capability
  grants `opener:allow-open-url` **with** an inline `http(s)://*` scope. Without the
  scope, `open_url` returns `ForbiddenUrl` at runtime (compiles fine).

## 6. Roadmap (do not implement unless asked)

- Text-extraction fallback (wikitext or rendered-HTML) for wikis lacking *TextExtracts*.
- Foreground-window game auto-detection.
- SQLite cache of fetched wiki pages.
- User-configurable hotkey.
- Answer history.
