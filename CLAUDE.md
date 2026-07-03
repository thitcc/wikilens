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
│   ├── types.ts                  # Shared types: GameInfo, ProviderInfo, Source, AskResult, AskStatus
│   ├── styles.css                # Transparent body + glass dark panel
│   └── components/
│       ├── GamePicker.tsx        # <select> of supported games
│       ├── ProviderPicker.tsx    # <select> of LLM providers
│       ├── PromptInput.tsx       # textarea; Enter submits, Shift+Enter = newline
│       ├── AnswerView.tsx        # streamed markdown (react-markdown; links open externally)
│       └── SourceList.tsx        # wiki source links (open in system browser)
└── src-tauri/                    # Backend (Rust) — run cargo commands here
    ├── tauri.conf.json           # window "overlay" (transparent, right-dock), CSP
    ├── capabilities/default.json # webview permissions (no http/fs)
    └── src/
        ├── main.rs               # thin entry → wikilens_lib::run()
        ├── lib.rs                # dotenv + builder: plugins, tray, hotkey, commands, state
        ├── state.rs              # AppState: shared reqwest::Client + ask-in-progress flag
        ├── window.rs             # toggle/show/hide + right-edge, DPI-aware positioning
        ├── hotkey.rs             # Shift+C registration (release-safe)
        ├── tray.rs               # tray icon: Show/Hide, Quit
        ├── commands.rs           # #[tauri::command] ask / hide_overlay / list_games / list_providers
        ├── error.rs              # AppError (thiserror) + Into<String>
        ├── providers.rs          # LLM provider registry (Anthropic, DeepSeek, OpenRouter)
        ├── llm.rs                # streaming client: Anthropic + OpenAI-compatible SSE
        └── wiki/{mod,games,search,fetch,wikitext}.rs  # registry + search + fetch wikitext → plaintext
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
- **API keys:** `ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` / `OPENROUTER_API_KEY`,
  from a `.env` file (dotenvy, loaded at the top of `run()`) or OS env vars (which
  take precedence). Read Rust-side only in `commands::run_ask`; never logged, never
  sent to the frontend. `ProviderInfo` (id+name) is the only provider data crossing
  IPC — it deliberately does not report which keys are configured.
- **Model per provider:** built-in `default_model`, overridable via
  `WIKILENS_<PROVIDER>_MODEL` (e.g. `WIKILENS_DEEPSEEK_MODEL`) — no code change.
- **Frontend → Rust only via `src/api.ts`.** Components never import
  `@tauri-apps/*` directly.
- **Commands return `Result<T, String>`** with user-readable messages; internal
  fallible code uses `AppError` (`error.rs`), converted to `String` at the boundary.
- **Namespaced events** (payloads):
  - `overlay://shown` — `()`; frontend focuses the prompt input.
  - `ask://status` — `"searching" | "reading" | "answering"`.
  - `ask://delta` — `string` chunk of the streamed answer.
- **Adding a game** = one `GameWiki` entry in `wiki/games.rs`. Nothing else.
- **Adding an LLM provider** = one `Provider` entry in `providers.rs`; behavior
  differences collapse to `ProviderKind` (Anthropic native vs OpenAI-compatible,
  shared by DeepSeek/OpenRouter). `llm.rs` branches request-build + SSE parse on it.
- Concurrency: `ask` rejects if one is already running (`AppState::ask_in_progress`).

## 5. Gotchas

- `transparent: true` needs `background: transparent` on `html, body` in
  `styles.css`, or the window renders as a black rectangle.
- Position with **monitor offset + scale factor** (see `window::position_right_edge`)
  — required for correct docking on multi-monitor / high-DPI setups.
- **Exclusive-fullscreen** games cover the overlay. Expected, not a bug.
- MediaWiki etiquette: keep the custom `User-Agent` (`wiki::USER_AGENT`); page
  fetches are a single batched request — don't parallelize them.
- Content is read as raw wikitext (`prop=revisions`, one batched request) and
  cleaned by `wiki::wikitext::to_plaintext`. This is used for **all** wikis, not
  `prop=extracts`: many game wikis lack TextExtracts, and a whole-article extracts
  request is capped to one page (it would silently drop the other search hits).
  The cleaner keeps prose but strips template/infobox tables.
- The spec'd **Shift+C** is a bare Shift+letter global hotkey: on Windows it
  swallows Shift+C system-wide, so a capital `C` can't be typed into the prompt.
  Search is case-insensitive so lowercase works; prefer a Ctrl/Alt combo when the
  hotkey becomes configurable (see `hotkey.rs`).
- Opening links needs both the opener command **and** a URL scope: the capability
  grants `opener:allow-open-url` **with** an inline `http(s)://*` scope. Without the
  scope, `open_url` returns `ForbiddenUrl` at runtime (compiles fine).
- `.env` is loaded via dotenvy at the top of `run()` for dev; a **packaged app's
  cwd is unpredictable**, so shipped installs should supply keys via OS env vars.
- OpenAI-compatible SSE (DeepSeek/OpenRouter) emits `:` comment/keep-alive lines
  and can report errors mid-stream on an HTTP-200 body; `parse_openai_sse_line`
  (via the `SseLine` enum) handles `[DONE]`, comments, null content, and errors.

## 6. Roadmap (do not implement unless asked)

- Rendered-HTML extraction (`action=parse&prop=text`) so infobox/stat tables are
  captured too — the current wikitext fallback keeps prose but drops those tables.
- Foreground-window game auto-detection.
- SQLite cache of fetched wiki pages.
- User-configurable hotkey.
- Answer history.

## 7. Planning vault

Plans, decisions, research, and retros live in **`vault/`** as
`YYYY-MM-DD_<kebab-slug>.md` with YAML frontmatter (`title, type, status, created,
updated, tags, related, commit`). Open the `vault/` folder itself as the Obsidian
vault (not the repo root — that would index `node_modules`).

- **Before non-trivial work:** create a `type: plan` doc from `vault/templates/plan.md`
  and set `status: active`. For an architecturally-significant choice, create a
  `type: decision` doc (MADR-lite) from `vault/templates/decision.md`.
- **On finishing:** set `status: done`, bump `updated:`, add the `commit:` hash, and
  add `related:` wikilinks. Done means: frontmatter complete, `status` accurate,
  `updated` bumped, ≥1 `related` link or tag, `commit` set when tied to code.
- **Vocabularies:** `type` ∈ {plan, decision, research, fix, retro, note};
  `status` ∈ {idea, todo, active, blocked, done, dropped}. Reuse topic tags from the
  registry in `vault/index.md` — don't invent synonyms (e.g. use `llm`, not `ai`).
- Filenames are permanent IDs — don't rename. Wikilinks in frontmatter must be
  quoted (`"[[...]]"`); dates are bare ISO. No secrets in notes.
- `README.md` and this file are user/dev guides and are **not** part of the vault.
- The roadmap above (§6) is the single source of truth for future ideas; promote an
  item to a full `plan` doc only when work on it starts.
