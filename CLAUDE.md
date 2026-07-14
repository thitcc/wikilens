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
├── vault/                        # planning vault: plans, decisions, notes (see §7)
├── docs/                         # dev guides: manual smoke checklist, AI-workflow explainer
├── src/                          # Frontend (React + TS)
│   ├── main.tsx                  # React entry
│   ├── App.tsx                   # Layout + state: header/prompt/answer, events, ask flow
│   ├── api.ts                    # ONLY bridge to Rust: invoke() + event listeners (typed)
│   ├── types.ts                  # Shared types: GameInfo, ProviderInfo, ModelInfo/List, Source, AskResult, AskStatus
│   ├── modelPick.ts              # stored model pick + vision resolution (pure helpers, unit-tested)
│   ├── styles.css                # Transparent body + glass dark panel
│   ├── test/                     # Vitest harness: setup, fake IPC backend (mockIPC), mount helpers
│   └── components/
│       ├── GameChip.tsx          # header chip: current game, opens the game menu
│       ├── GameMenu.tsx          # game menu: filter, Recent, monogram tiles, pinned "Add a game…"
│       ├── ModelChip.tsx         # footer chip: current provider · model, opens the menu
│       ├── ModelMenu.tsx         # combined provider/model menu (filter, collapsible groups)
│       ├── PromptInput.tsx       # textarea; Enter submits, Shift+Enter = newline
│       ├── AnswerView.tsx        # streamed markdown (react-markdown; links open externally)
│       ├── AddGameMenu.tsx       # add-game popover (via the game menu's pinned action): suggest/probe/add + remove
│       └── SourceList.tsx        # wiki source links (open in system browser)
└── src-tauri/                    # Backend (Rust) — run cargo commands here
    ├── tauri.conf.json           # window "overlay" (transparent, right-dock), CSP
    ├── capabilities/default.json # webview permissions (no http/fs)
    └── src/
        ├── main.rs               # thin entry → wikilens_lib::run()
        ├── lib.rs                # dotenv + builder: plugins, tray, hotkey, commands, state
        ├── state.rs              # AppState: shared reqwest::Client, ask-in-progress flag, model-list cache
        ├── window.rs             # toggle/show/hide + top-right float, DPI-aware sizing
        ├── hotkey.rs             # Shift+C registration (release-safe)
        ├── tray.rs               # tray icon: Show/Hide, Quit
        ├── commands.rs           # #[tauri::command] ask / hide_overlay / list_games / suggest_wikis / add_game / remove_game / list_providers / list_models
        ├── error.rs              # AppError (thiserror) + Into<String>
        ├── http.rs               # shared client factory: redirect policy, connect/read timeouts
        ├── providers.rs          # LLM provider registry + curated model fallbacks
        ├── llm.rs                # streaming client: Anthropic + OpenAI-compatible SSE
        ├── models.rs             # model catalogs: live fetch + parsers → {id, label}
        └── wiki/{mod,games,user,probe,search,fetch,html,wikitext}.rs  # registry (+ user store, probe validation) + search + fetch rendered HTML → plaintext
```

**Data flow:** `hotkey → window toggle → frontend prompt → ask command → wiki
search/fetch → LLM stream → events → UI`.

## 3. Commands

Frontend/Tauri from repo root; `cargo` from `src-tauri/`:

- Dev app: `npm run tauri dev`
- Frontend build: `npm run build` · Type-check: `npx tsc --noEmit` ·
  Tests: `npm test` (Vitest; `npm run test:watch` while developing)
- Rust: `cd src-tauri && cargo check` · `cargo test` ·
  lint: `cargo clippy --all-targets -- -D warnings`
- Live wiki/API suites: `cargo test -- --ignored` (from `src-tauri/`) — the
  network-hitting `#[ignore]`d tests. Cadence: before each release, when
  adding/changing a game or provider, ~monthly otherwise
  (see `docs/smoke-checklist.md`)
- Verify all: `/check` bundles the type-check + `npm test` + clippy +
  `cargo test` — the code gates CI runs (`.github/workflows/ci.yml`, every PR
  and push to main). CI additionally audits dependencies (`cargo audit`,
  `npm audit --audit-level=high`) — CI-only, since they depend on the network
  and advisory databases
- Release: `npm run tauri build`, then walk the manual smoke checklist
  (`docs/smoke-checklist.md`) — the runtime-only surface (overlay, hotkey,
  tray, DPI, packaged keys) has no automated coverage

## 4. Conventions

- **All HTTP happens in Rust** — and every client comes from
  `http::build_client()` (shared User-Agent, ≤5-hop redirect policy refusing
  https→http downgrades, connect/read timeouts). Read bodies via
  `http::read_body_capped` / `read_error_body`, never bare `.text()` — new
  fetch paths inherit the byte caps only through the helpers. The webview has
  no network/http/fs permissions (see `capabilities/default.json`).
- **API keys:** `ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` / `OPENROUTER_API_KEY`,
  from a `.env` file (dotenvy, loaded at the top of `run()`) or OS env vars (which
  take precedence). Read Rust-side only in `commands.rs`; never logged, never
  sent to the frontend. `ProviderInfo` (id, name, resolved default model + label)
  is the only provider data crossing IPC — it deliberately does not report which
  keys are configured.
- **Model precedence:** an explicit UI pick (footer chip menu, stored per provider
  in `localStorage["wikilens.selectedModel.<id>"]`) wins; else the
  `WIKILENS_<PROVIDER>_MODEL` env override (e.g. `WIKILENS_DEEPSEEK_MODEL`); else
  the built-in `default_model`. `ask` takes the model id; blank falls back to the
  provider default, and ids are deliberately **not** validated Rust-side — the
  provider is the authoritative validator (a stale id surfaces in the error box).
- **Model lists** (`list_models`, hybrid — see the sourcing ADR in `vault/`):
  live fetch (8s timeout, parsers trim to `{id, label}`) → session cache (**live
  lists only** — fallbacks always retry next open) → the provider's tiny
  `curated_models`, flagged `source: "fallback"` so the menu can say
  "offline list" without revealing why.
- **Frontend → Rust only via `src/api.ts`.** Components never import
  `@tauri-apps/*` directly.
- **Commands return `Result<T, String>`** with user-readable messages; internal
  fallible code uses `AppError` (`error.rs`), converted to `String` at the boundary.
- **Namespaced events** (payloads):
  - `overlay://shown` — `()`; frontend focuses the prompt input.
  - `ask://status` — `"searching" | "reading" | "answering"`.
  - `ask://delta` — `string` chunk of the streamed answer.
- **Adding a built-in game** = one `GameWiki` entry in `wiki/games.rs`. Nothing
  else — but verify the endpoint live first, derive `page_url` from the wiki's
  real `articlepath` (minecraft.wiki and wiki.warframe.com serve pages under
  `/w/`, not `/wiki/`), and set `search_namespace` for shared wikis that keep
  each game in its own namespace (UESP: Skyrim = `"134"`; default search there
  returns zero hits). Prefer official/independent wikis over stale Fandom
  copies. Anchor each new game with a golden query (`GOLDEN_CASES` in
  `wiki/mod.rs` — an offline test fails if a built-in lacks one; verify the
  new case live via `cargo test golden -- --ignored`).
- **User-added games** (`add_game`/`suggest_wikis`/`remove_game`): every URL is
  probe-validated in Rust (`wiki/probe.rs` — siteinfo + one test search;
  endpoints derived from the wiki's own `articlepath`/`scriptpath`, never
  assumed from the typed URL) and persisted to `wikis.json` in the app-data dir
  (`wiki/user.rs`, a managed `UserWikiStore` created in `.setup()`). `ask`
  resolves built-ins first, then the store — the registry stays Rust-side and
  `ask` never fetches a URL the frontend supplies per-request.
- **Adding an LLM provider** = one `Provider` entry in `providers.rs` (incl. its
  `models_endpoint`, `models_need_key`, and a 2–4-entry `curated_models` fallback);
  behavior differences collapse to `ProviderKind` (Anthropic native vs
  OpenAI-compatible, shared by DeepSeek/OpenRouter). `llm.rs` and `models.rs`
  branch request-build + parsing on it.
- Concurrency: `ask` rejects if one is already running (`AppState::ask_in_progress`).
- **Debugging an ask:** `WIKILENS_DEBUG=1` prints a per-ask table to stderr —
  phase timings, models, token counts, queries, page titles + char counts;
  never wiki text or keys (`src-tauri/src/debug.rs`, print-on-Drop so error
  exits still report). Independent of `WIKILENS_TRACE_RETRIEVAL`.
- **Delivery: every change lands via PR** (ADR:
  `vault/2026-07-13_pr-delivery-workflow.md`). Branch `<type>/<slug>` off `main`
  (e.g. `ci/github-actions-pipeline`), conventional commit prefixes (`feat:`,
  `fix:`, `chore:`, `ci:`, `docs(vault):`), push, `gh pr create --assignee @me`
  (always assign the owner) — no direct commits to `main`. Run `/check` before pushing (it mirrors CI's code gates;
  CI adds the dependency audits) and `/vault-lint` when `vault/` changed. A PR
  merges only when CI is green, and the **human merges — always a merge
  commit, never squash/rebase** (vault docs pin
  branch `commit:` hashes; squashing orphans them); Claude never merges. The
  `docs(vault): close …` commit rides in the same PR as the code it closes.
- **PR titles & descriptions.** The title is a plain-language headline of the
  outcome: imperative verb first, sentence case, no `type:` prefix or other
  decorations, ≤ ~70 chars — "Make PR descriptions readable at a glance",
  not "docs: adopt the human-readable PR description anatomy". Commits keep
  the conventional prefixes; merge-commit-only history means the title never
  doubles as a commit message. Descriptions follow
  `.github/PULL_REQUEST_TEMPLATE.md` — and since
  `gh pr create --body` bypasses template auto-fill, write the body to that
  skeleton yourself: plain-language 2–3-sentence summary linking the vault plan
  doc; change bullets at behavior level (what works differently — never
  file/function jargon), technical detail in a `>` quote under the bullet only
  when it adds something; "⚠️ Behavior changes" (or "None"); a Verification
  table (check | result). Each paragraph/bullet is **one source line** —
  GitHub renders newlines in PR bodies as line breaks, so hard-wrapped text
  comes out ragged. ~20 rendered lines; war stories go in `<details>` folds.

## 5. Gotchas

- `transparent: true` needs `background: transparent` on `html, body` in
  `styles.css`, or the window renders as a black rectangle.
- Position with **monitor offset + scale factor** (see `window::position_top_right`)
  — required for correct placement on multi-monitor / high-DPI setups.
- The floating panel's geometry is **split across two runtimes**: `window.rs`
  constants (`PANEL_GAP`, `SHADOW_ROOM_*`, `PANEL_HEIGHT_FRAC`) size the window;
  the CSS margins / `--panel-gap` / `--shadow-room-*` tokens in `styles.css`
  must match, or the shadow clips and the panel drifts off its gap. The
  transparent gap/apron ring still captures mouse input while the overlay is
  shown (Tauri transparent windows aren't click-through) — kept small on
  purpose.
- **Exclusive-fullscreen** games cover the overlay. Expected, not a bug.
- MediaWiki etiquette: keep the custom `User-Agent` (`wiki::USER_AGENT`); page
  fetches are **sequential** (one `action=parse` request per page, 12s timeout) —
  don't parallelize them. Failures fall back to a single batched `prop=revisions`
  request.
- Multi-word searches must send `srwhat=text`: default-engine wikis
  (stardewvalleywiki.com has no search extension) otherwise title-match them and
  return 0 hits — "best crops for winter" finds nothing while "wood" works. But
  keep it **off for single-word queries**: fulltext demotes exact-title pages
  ("wood" ranks Wood Chipper above Wood), and single words title-match fine on
  both engine types. A no-op either way on Elasticsearch-backed wikis (Fandom).
- Content is read as **rendered HTML** (`action=parse&prop=text`, per page) and
  reduced by `wiki::html::to_plaintext`, which keeps infobox rows and data tables
  as `label | value` pipe lines — the data raw wikitext structurally lacks (it's
  template/Lua-generated server-side). Raw wikitext (`prop=revisions`, batched,
  cleaned by `wiki::wikitext::to_plaintext` — prose only, tables stripped) remains
  the fallback when a parse call fails or times out. `prop=extracts` is still
  avoided: many game wikis lack TextExtracts, and whole-article extracts are
  capped to one page (it would silently drop the other search hits).
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
- **Native `<select>` popups are OS-drawn** (WebView2 renders them outside
  the page): over the glass they appear as an unstylable white system sheet
  no token can reach. That's why every picker is an **owned menu** (game,
  model, add-game) — never reintroduce a native select for anything shown
  over the game.
- **Esc is layered by event phase:** each menu's Esc handler (game, model,
  add-game) is a *capture-phase* window listener that stops propagation; App's
  Esc-hides-overlay listener is *bubble-phase* on the same window. First Esc
  closes the menu, the second hides the overlay — keep the phases straight or
  one Esc does both. App enforces **one open menu at a time** (`openMenu`
  union state) so capture handlers never stack.
- Menus must stay **direct children of `.panel`** (`.content` has
  `overflow-y: auto` and would clip them), and `--menu-clearance` /
  `--menu-clearance-top` are paired constants (like the window.rs float
  geometry): panel padding + footer/header height + gap (top is 42px for the
  chip-height header — retuned from 52 when the bordered select left).
  Top-anchored menus cap their height against the **window viewport**
  (`100vh` minus gap/clearance/apron), not the panel — the panel hugs its
  content, and a panel-relative cap strangles the menu to one row on an idle
  panel (the window always holds the 70% cap, and `.panel` doesn't clip its
  absolute children). Retune the clearances when the footer's or header's
  metrics change.
- OpenRouter's catalog is 300+ models (~1–2 MB raw; reqwest's `gzip` feature
  keeps it ~150–300 KB on the wire) — parsers trim to `{id, label}` before IPC,
  and its menu group starts collapsed, which also defers the fetch until first
  expand.

## 6. Roadmap (do not implement unless asked)

- Foreground-window game auto-detection.
- SQLite cache of fetched wiki pages.
- User-configurable hotkey.
- Answer history.
- Overlay design follow-ups: accent-direction exploration and a high-contrast
  bright-scene variant (dropped from the 2026-07-05 design-sync plan).

## 7. Planning vault

Plans, decisions, research, and retros live in **`vault/`** as
`YYYY-MM-DD_<kebab-slug>.md` with YAML frontmatter (`title, type, status, created,
updated, tags, related, commit`). Open the `vault/` folder itself as the Obsidian
vault (not the repo root — that would index `node_modules`).

- **Before non-trivial work:** create a `type: plan` doc from `vault/templates/plan.md`
  and set `status: active`. For an architecturally-significant choice, create a
  `type: decision` doc (MADR-lite) from `vault/templates/decision.md`. Scaffold either
  with the `/plan-doc` or `/decision-doc` slash command (in `.claude/commands/`, run
  from the repo root) — it fills the filename, dates, and frontmatter for you.
- **On finishing:** set `status: done`, bump `updated:`, add the `commit:` ref (a single
  hash, or a `[list, of, hashes]` when the work spans several commits), and add
  `related:` wikilinks. Done means: frontmatter complete, `status` accurate,
  `updated` bumped, ≥1 `related` link or tag, `commit` set when tied to code.
- **Vocabularies:** `type` ∈ {plan, decision, research, fix, retro, note};
  `status` ∈ {idea, todo, active, blocked, done, dropped}. Reuse topic tags from the
  registry in `vault/index.md` — don't invent synonyms (e.g. use `llm`, not `ai`).
- **Linting:** the `vault-lint` skill (or `/vault-lint`) validates every doc against
  these conventions — vocab, registry tags, `YYYY-MM-DD_slug` filenames, ISO dates,
  quoted/resolvable wikilinks. Run it after creating or closing a doc and before
  committing `vault/` changes; engine is `.claude/skills/vault-lint/lint.mjs`.
- Filenames are permanent IDs — don't rename. Wikilinks in frontmatter must be
  quoted (`"[[...]]"`); dates are bare ISO. No secrets in notes.
- `README.md` and this file are user/dev guides and are **not** part of the vault.
- The roadmap above (§6) is the single source of truth for future ideas; promote an
  item to a full `plan` doc only when work on it starts.
