# WikiLens

[![CI](https://github.com/thitcc/wikilens/actions/workflows/ci.yml/badge.svg)](https://github.com/thitcc/wikilens/actions/workflows/ci.yml)

A Windows desktop overlay for wiki-heavy games (Stardew Valley, Conan Exiles,
Core Keeper, …). Press a global hotkey while in-game, type a natural-language
question, and get a concise answer drawn **only** from the game's official wiki,
with links to the source pages.

**How it works:** hotkey → a glass panel slides in from the right edge and takes
focus → you type a question → WikiLens searches the game's MediaWiki, fetches the
top pages as plaintext, sends them to an LLM with instructions to answer *only*
from that content, and streams the answer back with its sources.

Built with **Tauri v2 + Rust** (backend) and **Vite + React + TypeScript**
(frontend). All network access happens in Rust; the webview has no HTTP
permissions and never sees your API key.

---

## Requirements

- **Windows 10/11** (this is the only supported target for now).
- **WebView2 runtime** — preinstalled on Windows 11 and current Windows 10; if
  missing, install the [Evergreen WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
- **Node.js ≥ 18** and npm.
- **Rust** (stable toolchain) + Cargo.
- An **API key** for at least one supported LLM provider — **Anthropic**,
  **DeepSeek**, or **OpenRouter**.

## Setup: API keys

WikiLens supports three LLM providers — choose one from the provider dropdown in
the panel. Keys are read **in the Rust process only**; they are never sent to the
frontend or logged. You only need a key for the provider you actually use.

| Provider | Env var | Default model | Model override var |
|---|---|---|---|
| Anthropic | `ANTHROPIC_API_KEY` | `claude-haiku-4-5-20251001` | `WIKILENS_ANTHROPIC_MODEL` |
| DeepSeek | `DEEPSEEK_API_KEY` | `deepseek-chat` | `WIKILENS_DEEPSEEK_MODEL` |
| OpenRouter | `OPENROUTER_API_KEY` | `openai/gpt-4o-mini` | `WIKILENS_OPENROUTER_MODEL` |

**Option A — `.env` file (easiest for development):** copy the template and fill
in the keys you have.

```bash
cp .env.example .env
# then edit .env and paste your key(s)
```

`.env` is gitignored and loaded on startup; it works for `npm run tauri dev`.

**Option B — OS environment variables** (required for the *packaged* app, whose
working directory is unpredictable, so `.env` may not be found):

```powershell
# PowerShell (current session)
$env:DEEPSEEK_API_KEY = "sk-..."
# Or persist for your user account (new terminals only)
setx ANTHROPIC_API_KEY "sk-ant-..."
```

```bash
# bash / Git Bash
export OPENROUTER_API_KEY="sk-or-..."
```

OS environment variables **take precedence** over `.env` values. Keys are read when
you ask a question; if you set one after launching, restart WikiLens. If the
selected provider's key is missing, the app shows a clear "set `<PROVIDER>_API_KEY`"
message instead of answering.

**Changing the model:** each provider ships a sensible default; override it without
touching code by setting that provider's `WIKILENS_*_MODEL` variable. This matters
because model ids drift over time (e.g. DeepSeek may move `deepseek-chat` →
`deepseek-v4-flash`).

## Commands

Run frontend/Tauri commands from the repo root; run `cargo` commands in `src-tauri/`.

| Task | Command |
|---|---|
| Run the app (dev) | `npm install` then `npm run tauri dev` |
| Build a release installer | `npm run tauri build` |
| Frontend production build | `npm run build` |
| Type-check the frontend | `npx tsc --noEmit` |
| Check the Rust code | `cd src-tauri && cargo check` |
| Lint the Rust code | `cd src-tauri && cargo clippy --all-targets -- -D warnings` |
| Run Rust unit tests | `cd src-tauri && cargo test` |

## Development workflow

All changes land through a pull request — no direct commits to `main`:

1. Branch off `main` as `<type>/<slug>` (e.g. `ci/github-actions-pipeline`).
2. Commit with conventional prefixes (`feat:`, `fix:`, `chore:`, `ci:`, `docs(vault):`).
3. Verify locally before pushing — the same gates CI runs: `npx tsc --noEmit`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test`.
4. Open a PR; CI (`.github/workflows/ci.yml`) must be green before merging.
5. Merge with a **merge commit** (never squash) — the planning vault pins commit
   hashes from PR branches, and squashing would orphan them.

## Using it

- **Shift+C** — show/hide the overlay from anywhere (also available from the tray).
- Pick your **LLM provider** and **game** from the dropdowns in the header; both
  choices are remembered between sessions.
- When shown, the panel takes focus so you can type immediately.
- **Enter** sends your question; **Shift+Enter** adds a newline.
- **Esc** (or Shift+C again) hides the panel; focus returns to the game.
- The app starts hidden and lives in the **system tray**. It only quits via the
  tray's **Quit** item — closing the window just hides it.

## Supported games

Adding a game is a one-line change in `src-tauri/src/wiki/games.rs`.

| Game | Wiki | Search | Answer content | Notes |
|---|---|---|---|---|
| **Conan Exiles** | Fandom | ✅ | ✅ | Full support. |
| **Core Keeper** | Fandom | ✅ | ✅ | Full support. |
| **Stardew Valley** | Self-hosted | ✅ (keyword) | ✅ | Search works best with short keyword queries (its legacy search backend is strict about multi-word phrases). |

> **How content is read.** WikiLens fetches each page's raw wikitext
> (`prop=revisions`, one batched request) and converts it to plaintext. This works
> on **every** MediaWiki wiki — unlike `prop=extracts` (the *TextExtracts*
> extension), which many game wikis lack (Core Keeper, Stardew) *and* which caps
> whole-article requests to a single page. The conversion keeps article **prose**
> but drops template/infobox tables, so a stat that lives only in an infobox may
> be missing. Endpoints verified 2026-07-02.

## Manual smoke test

1. Set a key: `cp .env.example .env` and fill in one provider's key (or export it,
   e.g. `$env:ANTHROPIC_API_KEY = "sk-ant-..."`).
2. From the repo root: `npm install` then `npm run tauri dev`.
3. Wait for the tray icon to appear (the window starts hidden), then press **Shift+C**.
4. The panel slides in from the right and focuses the input.
5. Choose your **provider** (the one you set a key for) and a **game**, then ask a
   question — e.g. **Conan Exiles** → *"how do I make steel bars?"*, or **Core
   Keeper** → *"best way to get wood"*.
6. Expect the status to move through *Searching → Reading → Answering*, the answer
   to **stream in** as markdown, and **2–4 source links** to appear beneath it.
   Clicking a source opens the wiki page in your browser.
7. Try the other providers (whichever keys you set) — the same question should
   stream an answer from each. Press **Esc** to hide the panel.

> **Stardew Valley** search works best with short keyword queries (e.g. *"cauliflower"*
> rather than a full sentence), because its self-hosted wiki uses the legacy search
> backend. Its page content reads fine via the wikitext fallback.

## Caveats

- **Borderless / windowed only.** The overlay works over games running in
  borderless-windowed or windowed mode. Games in **exclusive fullscreen** will
  paint over the overlay — that's expected, not a bug. Switch the game to
  borderless mode.
- **Windows only** for now (macOS/Linux are not targeted).
- **The Shift+C hotkey swallows Shift+C system-wide.** Because it's registered as a
  bare Shift+letter global hotkey, you can't type a **capital C** into the prompt
  (pressing Shift+C toggles the overlay), and an in-game Shift+C toggles the panel.
  Wiki search is case-insensitive, so lowercase queries work fine. A Ctrl/Alt-based
  combo is preferable and is planned as part of the configurable-hotkey work.
- Answers are only as good as the wiki. WikiLens will say so when the pages don't
  contain the answer, rather than guessing.

## Security & privacy

- Your provider API keys stay in the Rust process; they are never exposed to the
  webview or written to logs.
- The webview is locked down by CSP to talk only to the local Tauri IPC — it has
  no direct network, HTTP, or filesystem access. Every outbound request (wiki +
  the selected LLM provider) is made from Rust.
- Your question and the fetched wiki excerpts are sent to the LLM provider you
  select, to generate the answer.
