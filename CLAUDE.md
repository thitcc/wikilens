# CLAUDE.md — WikiLens

## 1. Project summary

WikiLens is a **Windows** desktop overlay for wiki-heavy games. A global hotkey
(**Ctrl+`** by default) slides a glass panel in from the right edge; the player
types a question and gets an answer generated **only** from the game's MediaWiki
content (hotkey → panel → RAG over the wiki → streamed answer + sources). A
second hotkey (**Ctrl+Shift+C** by default) — or the footer capture chip — grabs
a screen region and attaches it to the prompt as an image (vision-capable models
only). Both shortcuts are configurable from the header gear's **Shortcuts**
popover (persisted to `settings.json` in the app-data dir). Works over
**borderless/windowed** games only — exclusive fullscreen covers the overlay.
Tauri v2 + Rust backend, Vite + React + TypeScript frontend.

## 2. Architecture map

```
wikilens/
├── index.html, capture.html, debug.html, vite.config.ts, tsconfig*.json   # Vite/TS config; three rollup inputs (overlay + region-select + debug pages)
├── vault/                        # planning vault: plans, decisions, notes (see §7)
├── docs/                         # dev guides: manual smoke checklist, AI-workflow explainer
├── src/                          # Frontend (React + TS; *.test.* files are colocated Vitest suites)
│   ├── main.tsx                  # React entry
│   ├── App.tsx                   # Layout + state: header/prompt/answer, events, ask flow, capture attachment chip
│   ├── api.ts                    # ONLY bridge to Rust: invoke() + event listeners (typed)
│   ├── types.ts                  # Shared types: GameInfo, ProviderInfo, ModelInfo/List, Source, AskResult, AskStatus
│   ├── modelPick.ts              # stored model pick + vision resolution (pure helpers, unit-tested)
│   ├── hotkeys.ts                # recorder pure helpers: combo→accelerator, validation, labels (paired with hotkey.rs)
│   ├── menuPlacement.ts          # model-menu drop/flip measurement (paired constants — see §5)
│   ├── menuNav.ts                # nextHighlight(): arrow-key highlight movement for the menus (pure, unit-tested)
│   ├── menuScroll.ts             # centerRowInList() + keepRowInView(): center the picked row on open, reveal the highlighted row on arrows
│   ├── styles.css                # Transparent body + glass dark panel
│   ├── test/                     # Vitest harness: setup, fake IPC backend (mockIPC), mount helpers
│   ├── capture/main.ts           # region-select page (vanilla TS, own bundle): drag → finish/cancel_capture
│   ├── debug/                    # debug-window page (React, own bundle): events.ts IPC boundary, askState reducer, DebugApp/AskCard, own styles.css
│   └── components/
│       ├── GameChip.tsx          # header chip: current game, opens the game menu
│       ├── GameMenu.tsx          # game menu: filter, Recent, monogram tiles, pinned "Add a game…"
│       ├── ModelChip.tsx         # footer chip: current provider · model, opens the menu
│       ├── ModelMenu.tsx         # combined provider/model menu (filter, collapsible groups)
│       ├── SettingsMenu.tsx      # Shortcuts popover: a key-recorder row per hotkey (suspend → record → save)
│       ├── PromptInput.tsx       # textarea; Enter submits, Shift+Enter = newline
│       ├── AnswerView.tsx        # streamed markdown (react-markdown; links open externally)
│       ├── AddGameMenu.tsx       # add-game popover (via the game menu's pinned action): suggest/probe/add + remove
│       ├── Badge.tsx             # capability pill (e.g. "Image" on vision-capable model rows)
│       └── SourceList.tsx        # wiki source links (open in system browser)
└── src-tauri/                    # Backend (Rust) — run cargo commands here
    ├── tauri.conf.json           # windows "overlay" (right-dock) + "capture" (region-select), both transparent; CSP
    ├── capabilities/             # webview permissions: default.json (overlay — no http/fs) + capture.json (capture — events + show/hide/focus only) + debug.json (debug — events + start-dragging only)
    └── src/
        ├── main.rs               # thin entry → wikilens_lib::run()
        ├── lib.rs                # dotenv + builder: plugins, tray, hotkey, commands, state
        ├── state.rs              # AppState: shared reqwest::Client, ask-in-progress flag, model-list + title-index caches, pending shot + attachment, rewrite breaker
        ├── settings.rs           # SettingsStore: settings.json (app-data) — the configurable hotkeys; loaded in setup before registration
        ├── window.rs             # toggle/show/hide + top-right float, DPI-aware sizing
        ├── hotkey.rs             # global shortcuts: defaults (Ctrl+` summon, Ctrl+Shift+C capture), accelerator (de)serialization, live re-registration (release-safe)
        ├── tray.rs               # tray icon: Show/Hide, Quit
        ├── capture.rs            # region capture: freeze monitor snapshot → crop/downscale → PNG attachment held in AppState
        ├── commands.rs           # #[tauri::command] ask / cancel_ask / hide_overlay / show_overlay / debug_available / toggle_debug_window / list_games / suggest_wikis / add_game / remove_game / list_providers / list_models / begin,finish,cancel,clear_capture / get_settings / set_hotkey / suspend,resume_hotkeys
        ├── error.rs              # AppError (thiserror) + Into<String>
        ├── http.rs               # shared client factory: redirect policy, connect/read timeouts
        ├── providers.rs          # LLM provider registry + curated model fallbacks
        ├── llm.rs                # streaming client: Anthropic + OpenAI-compatible SSE
        ├── models.rs             # model catalogs: live fetch + parsers → {id, label}
        ├── debug.rs              # WIKILENS_DEBUG=1 per-ask stderr table (print-on-Drop; see §4) + debug:// event payloads/sink
        ├── debug_window.rs       # visual debug window (flag-gated): glass, draggable, never activates; create/show/toggle + the emit_to sink
        ├── config_guardrails.rs  # test-only: parses the shipped config/capability files, pins the security invariants
        ├── test_support.rs       # test-only: shared wiremock fixtures + proptest strategies
        └── wiki/{mod,games,user,probe,search,fetch,html,wikitext,titles}.rs  # registry (+ user store, probe validation) + search + fetch rendered HTML → plaintext; titles = per-game typo-recovery index
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
  - `overlay://shown` — `()`; frontend focuses the prompt input and selects
    the old question — unless the panel hid <2s ago (see `overlay://hidden`).
  - `overlay://hidden` — `()`; fired by every hide path (Esc, hotkey toggle,
    tray, Alt+F4, capture). The frontend timestamps it and skips the next
    show's select-all within 2s, so an accidental hide can't arm a
    draft-replacing keystroke (born as the capital-C trap under the old
    Shift+C default — see §5). The settings recorder also disarms on it.
  - `ask://status` —
    `"searching" | "understanding" | "retrying" | "reading" | "answering"`
    (`understanding` only while the rewrite's candidate searches run;
    `retrying` only when the first search found nothing).
  - `ask://delta` — `string` chunk of the streamed answer.
  - `capture://hotkey` — `()`; the capture shortcut (default Ctrl+Shift+C),
    routed to the overlay webview, which invokes `begin_capture` (the
    frontend stays the single capture entry point).
  - `capture://armed` — `()`; sent to the reused capture webview each time
    Rust shows it, so it re-arms its drag handlers.
  - `capture://attached` — `{id, thumbUri, width, height}` on successful
    `finish_capture`. Only the ≤88px thumbnail data-URI crosses IPC; the full
    PNG (longest edge ≤1568px) stays in `AppState` until `ask` consumes it
    by id.
  - `capture://error` — `string`; user-readable capture failure (e.g. the
    selection was too small).
  - `debug://…` — the debug-window family (`ask-started`, `phase`,
    `candidates`, `usage`, `pages`, `finished`), emitted by `debug.rs` through
    the sink `debug_window.rs` injects, targeted at the debug webview only
    (`emit_to`). Every payload carries an `askId`; `finished` fires from
    `Drop` on every exit path (the partial-table analogue). Same contract as
    the stderr table: titles/counts/timings only — never wiki text, never
    keys (payload key sets are pin-tested).
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
- Concurrency: `ask` rejects if one is already running (`AppState::ask_in_progress`);
  `cancel_ask` (the status row's Stop) aborts the in-flight one by dropping its
  future mid-await — `ask` then settles with the `wikilens::ask-cancelled`
  sentinel (mirrored in `api.ts`), which the frontend maps to a quiet reset
  that keeps the partial answer.
- **Debugging an ask:** `WIKILENS_DEBUG=1` prints a per-ask table to stderr —
  phase timings, models, token counts, queries, page titles + char counts;
  never wiki text or keys (`src-tauri/src/debug.rs`, print-on-Drop so error
  exits still report). The same flag also creates the **visual debug window**
  (`debug_window.rs` → `src/debug/`): a glass panel like the overlay,
  draggable by its header, with live per-ask cards fed by the
  `debug://…` events — progress bars, collapsible details, ~25-ask session
  history. It starts **hidden**, matching the overlay: open it from the
  overlay footer's Debug chip (`toggle_debug_window`; the chip renders only
  when `debug_available` says the window exists) or the tray's "Show debug
  panel"; closing/hiding never loses history, and the hidden webview keeps
  recording asks run before the first show. The table stays byte-identical — the window is additive.
  Independent of
  `WIKILENS_TRACE_RETRIEVAL`. The five
  retrieval-tuning env vars (rewrite toggle/model/provider, title index, trace)
  are documented in README's "Retrieval tuning (advanced)" table — that table is
  the single source; don't re-list them here.
- **Delivery: every change lands via PR** (ADR:
  `vault/2026-07-13_pr-delivery-workflow.md`). Branch `<type>/<slug>` off `main`
  (e.g. `ci/github-actions-pipeline`), conventional commit prefixes (`feat:`,
  `fix:`, `chore:`, `ci:`, `docs(vault):`), push, `gh pr create --assignee @me`
  (always assign the owner) — no direct commits to `main`. Run `/check` before pushing (it mirrors CI's code gates;
  CI adds the dependency audits) and `/vault-lint` when `vault/` changed. A PR
  merges only when CI is green, and the **human merges — always a merge
  commit, never squash/rebase** (vault docs pin
  branch `commit:` hashes; squashing orphans them); the agent never merges. The
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
- **Codex reads this file too.** `CLAUDE.md` is the canonical guidance for both
  agents: the tracked `.codex/config.toml` sets
  `project_doc_fallback_filenames = ["CLAUDE.md"]`, so Codex loads it directly.
  Never create a root `AGENTS.md` — it outranks the fallback and would shadow
  this file with a stale copy. Claude's `/name` workflows map to Codex `$name`
  skills, generated locally into the gitignored `.agents/skills/` — after
  changing `CLAUDE.md` or anything under `.claude/`, and on a fresh clone
  before using Codex skills, run
  `node .claude/skills/sync-agents/generate.mjs`. Codex loads project config
  only for a trusted checkout, and `.codex/config.toml` / instruction-discovery
  changes take effect in a fresh Codex session.

## 5. Gotchas

- `transparent: true` needs `background: transparent` on `html, body`, or the
  window renders as a black rectangle — in `styles.css` for the overlay AND in
  `src/debug/styles.css` for the debug window (each bundle has its own sheet).
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
  request. The sequential rule covers those heavy page fetches only: the ≤2
  rewrite-candidate **search** GETs are deliberately concurrent (cheap, capped
  by `REWRITE_SEARCH_LIMIT`).
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
- **(Resolved by the Ctrl+` default.)** A bare Shift+letter global hotkey — the
  originally spec'd **Shift+C** — swallows that letter system-wide on Windows,
  so a capital `C` could never be typed into the prompt (the capital-C
  draft-loss trap). That history is why the summon default is **Ctrl+`**, why
  the shortcut recorder refuses Shift-only combos (`validateCombo` in
  `src/hotkeys.ts`), and why the 2s select-all suppression exists (§4
  `overlay://hidden`). Note the accelerator is a **virtual key**
  (`Code::Backquote` → `VK_OEM_3`, layout-resolved by Windows); it sits at the
  key left of 1 on US and ABNT2 layouts (verified on-device,
  vault/2026-07-19_hotkey-config.md).
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
  Top-anchored menus (game, add-game) cap their height against the **window
  viewport** (`100vh` minus gap/clearance/apron), not the panel — the panel
  hugs its content, and a panel-relative cap strangles the menu to one row on
  an idle panel (the window always holds the 70% cap, and `.panel` doesn't
  clip its absolute children). The **model menu measures per open**
  (`src/menuPlacement.ts`, constants paired with their CSS/window.rs twins by
  comment): it drops below the panel at a fixed ~460px (`.menu--down`), and
  flips to the upward `--menu-clearance` anchoring with
  `height = min(460, room above)` when a tall panel leaves more room above
  than below. Geometry is **not** re-measured while a menu is open (a panel
  growing under a streaming answer is accepted — the next open corrects).
  Retune the clearances when the footer's or header's metrics change.
- OpenRouter's catalog is 300+ models (~1–2 MB raw; reqwest's `gzip` feature
  keeps it ~150–300 KB on the wire) — parsers trim to `{id, label}` before IPC,
  and its menu group starts collapsed, which also defers the fetch until first
  expand.
- The debug window's `focusable(false)` is **load-bearing**, not cosmetic:
  tao's `show()` issues an activating `SW_SHOW`, so `focused(false)` alone
  only covers creation — a tray re-show would steal keyboard focus from the
  game. `WS_EX_NOACTIVATE` (via `focusable(false)`) covers creation, clicks,
  and re-shows. If WebView2 wheel-scroll/text-selection ever misbehaves under
  it, the documented fallback is `focusable(true)` + `focus(false)`.
- The debug window exists **only when `WIKILENS_DEBUG` was truthy at startup**
  (dynamic creation in `.setup()`; env can't change mid-process). `emit_to`
  to a nonexistent label is a **silent no-op** — `debug_window::sink` returns
  `None` when the window is absent, so don't bypass it with a raw
  `emit_to("debug", …)` and expect an error.
- `data-tauri-drag-region` needs `core:window:allow-start-dragging` in that
  window's capability — without it the attribute compiles and renders fine
  but drag **silently no-ops** (ACL rejection in the webview console only).
  Bare/`"true"` drags only when the mousedown target IS the attributed
  element; `"deep"` (the debug header) drags the whole subtree while clickable
  children still block. Its double-click maximize is denied by both the
  capability and `maximizable(false)` — a maximized glass sheet would cover
  the game.

## 6. Roadmap (do not implement unless asked)

- Foreground-window game auto-detection.
- SQLite cache of fetched wiki pages.
- Answer history.
- Menu combobox semantics (`aria-activedescendant`): expose the arrow-key
  highlight to assistive tech conformantly — needs ModelMenu's interactive
  group headers restructured out of the list first (a valid listbox can't
  contain them).
- Compose the next question while an answer is still streaming.
- Overlay design follow-ups: accent-direction exploration and a high-contrast
  bright-scene variant (dropped from the 2026-07-05 design-sync plan).
- Panel corner-pick: let the player choose which corner the overlay docks to
  (top-right collides with common game HUDs — 2026-07-18 critique).
- Footer vocabulary collapse: player-language model status in the footer, with
  the provider/model machinery one level down (2026-07-18 critique).

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

## Design Context

- `PRODUCT.md` is the strategic design context (register, users, positioning,
  design principles); `DESIGN.md` is the visual system, and its YAML
  frontmatter is the **normative token source** the impeccable design detector
  reads — new sizes/radii/colors get documented there, never silently.
- `.impeccable/` holds the tracked machine-readable sidecar + live-mode
  config; critique snapshots under `.impeccable/critique/` are local-only
  (gitignored).

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Treat an **absent edge as inconclusive**, never as evidence two things are unrelated — the AST extractor cannot see path-qualified Rust calls (`crate::http::build_client()` has 28 cross-file call sites but zero cross-file edges), emits no nodes for `const` items (`GOLDEN_CASES`), and splits cross-language IPC types into unlinked nodes (`GameInfo` exists once per language). When a query comes up empty, fall back to grep before concluding anything.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- In GRAPH_REPORT.md, only the **Surprising Connections** and **Graph Freshness** sections carry actionable signal. Do not re-investigate the god-node, cohesion, or weakly-connected-node sections: they are structural extraction artifacts, already triaged with verdicts and mechanisms in `vault/2026-07-17_graphify-findings-review.md`.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
