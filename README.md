# WikiLens

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
- An **Anthropic API key**.

## Setup: the Anthropic API key

WikiLens reads your key from the `ANTHROPIC_API_KEY` environment variable **in the
Rust process only** — it is never sent to the frontend or logged. If the variable
is missing, the app shows a clear setup message instead of answering.

Set it in the terminal you launch WikiLens from:

```powershell
# PowerShell (current session)
$env:ANTHROPIC_API_KEY = "sk-ant-..."

# Or persist it for your user account (new terminals only)
setx ANTHROPIC_API_KEY "sk-ant-..."
```

```bash
# bash / Git Bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

The key is read at the moment you ask a question. If you set it after launching,
restart WikiLens (a process can't see env vars set after it started).

## Commands

Run frontend/Tauri commands from the repo root; run `cargo` commands in `src-tauri/`.

| Task | Command |
|---|---|
| Run the app (dev) | `npm install` then `npm run tauri dev` |
| Build a release installer | `npm run tauri build` |
| Frontend production build | `npm run build` |
| Type-check the frontend | `npx tsc --noEmit` |
| Check the Rust code | `cd src-tauri && cargo check` |
| Run Rust unit tests | `cd src-tauri && cargo test` |

## Using it

- **Shift+C** — show/hide the overlay from anywhere (also available from the tray).
- When shown, the panel takes focus so you can type immediately.
- **Enter** sends your question; **Shift+Enter** adds a newline.
- **Esc** (or Shift+C again) hides the panel; focus returns to the game.
- The app starts hidden and lives in the **system tray**. It only quits via the
  tray's **Quit** item — closing the window just hides it.

## Supported games

Adding a game is a one-line change in `src-tauri/src/wiki/games.rs`.

| Game | Wiki | Search | Answer content | Notes |
|---|---|---|---|---|
| **Conan Exiles** | Fandom | ✅ | ✅ | Full support (wiki exposes the MediaWiki *TextExtracts* API). |
| **Stardew Valley** | Self-hosted | ✅ (keyword) | ⚠️ not yet | Wiki lacks the *TextExtracts* extension, so page text can't be pulled; search works best with short keyword queries (its legacy search backend is strict about multi-word phrases). |
| **Core Keeper** | Fandom | ✅ | ⚠️ not yet | This Fandom instance doesn't have *TextExtracts* enabled, so page text can't be pulled. |

> **Why the ⚠️?** WikiLens fetches clean plaintext via MediaWiki's `prop=extracts`
> (the *TextExtracts* extension). Wikis without that extension return search
> results but no extractable text, so answers for those games will say the pages
> couldn't be read. Adding a fallback (wikitext or rendered-HTML extraction) for
> those wikis is on the roadmap — see `CLAUDE.md`. Endpoints verified 2026-07-02.

## Manual smoke test

The best game to demo end-to-end today is **Conan Exiles** (its wiki supports full
text extraction).

1. Set your key: `$env:ANTHROPIC_API_KEY = "sk-ant-..."`.
2. From the repo root: `npm install` then `npm run tauri dev`.
3. Wait for the tray icon to appear (the window starts hidden), then press **Shift+C**.
4. The panel slides in from the right and focuses the input.
5. Pick **Conan Exiles** and ask something like *"how do I make steel bars?"*.
6. Expect the status to move through *Searching → Reading → Answering*, the answer
   to **stream in** as markdown, and **2–4 source links** to appear beneath it.
   Clicking a source opens the wiki page in your browser.
7. Press **Esc** to hide the panel.

> The scaffold plan's original example (*Stardew Valley → "best crops for winter"*)
> will search but return "couldn't read the pages" until a text-extraction
> fallback is added — see the Supported games note above.

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

- Your Anthropic API key stays in the Rust process; it is never exposed to the
  webview or written to logs.
- The webview is locked down by CSP to talk only to the local Tauri IPC — it has
  no direct network, HTTP, or filesystem access. Every outbound request (wiki +
  Anthropic) is made from Rust.
- Your question and the fetched wiki excerpts are sent to the Anthropic API to
  generate the answer.
