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

## Setup: where answers come from

Open the header gear (**Settings**). The **Answers** rows say which model writes
your answers: the built-in model when this install has one, and **your own
provider**. Exactly one wears the check.

The provider lines below — Anthropic, DeepSeek, OpenRouter — are not that
choice. They only hold keys: click one to paste a key in the field that opens,
and press **Save**. Saving stores the key and nothing else, so it never switches
what is answering out from under you; when you want your own provider to answer,
click that row. Which provider it uses is the footer chip on the panel itself.
Stored keys are encrypted for your Windows user (DPAPI) in the app-data dir, are
read **in the Rust process only**, and are never displayed back in any form —
the only action on a stored key is the trash beside its line. Keys are read at
ask time, so adding one needs no restart. If the selected provider has no key yet, the app
answers with *"No API key for `<provider>` yet — add one in Settings."*

| Provider | Default model | Model override var |
|---|---|---|
| Anthropic | `claude-haiku-4-5-20251001` | `WIKILENS_ANTHROPIC_MODEL` |
| DeepSeek | `deepseek-v4-flash` | `WIKILENS_DEEPSEEK_MODEL` |
| OpenRouter | `openai/gpt-4o-mini` | `WIKILENS_OPENROUTER_MODEL` |

> **Migrating from the env-var era:** `ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` /
> `OPENROUTER_API_KEY` are no longer read. Paste those keys into
> **Settings → Answers** once, then remove them from your `.env` — it
> now only serves the model overrides and tuning vars below. WikiLens prints a
> one-line startup reminder if any legacy variable is still set.

**Changing the model:** each provider ships a sensible default; override it without
touching code by setting that provider's `WIKILENS_*_MODEL` variable. This matters
because model ids drift over time (e.g. DeepSeek retired its `deepseek-chat` alias
in favor of `deepseek-v4-flash`).

### Default mode (optional)

One env-configured model source with the vendor hidden: the footer chip reads
just **Default**, there is no provider or model menu, and asks resolve entirely
from these variables. It appears in Settings → **Answers** as **Built into
WikiLens**; that row and **Your own provider** are what switch between the
two. On first launch WikiLens auto-senses (a complete
`WIKILENS_DEFAULT_*` set selects Default once — your stored choice wins forever
after). All four are required together:

| Variable | Meaning |
|---|---|
| `WIKILENS_DEFAULT_API_PROVIDER` | Wire protocol: `anthropic` or `openai` (never a vendor id — a neutral proxy works) |
| `WIKILENS_DEFAULT_API_URL` | Full chat/messages endpoint URL |
| `WIKILENS_DEFAULT_API_KEY` | The credential sent to that endpoint |
| `WIKILENS_DEFAULT_ANSWER_MODEL` | Model id for answers |

Optional: `WIKILENS_DEFAULT_REWRITE_MODEL` (a fast model for the pre-search
query rewrite; defaults to the answer model) and `WIKILENS_DEFAULT_VISION`
(truthy if the answer model reads images; default text-only — the capture
chip follows it). Packaged apps should set these as OS env vars (`.env` works
for `npm run tauri dev`).

## Retrieval tuning (advanced)

Before answering, WikiLens runs a small retrieval pipeline: a raw keyword search
and an LLM **query rewrite** run concurrently, their hits are merged, and a local
fuzzy **title index** catches typos as a last resort. It works out of the box;
these variables are escape hatches for when you want to change or trace it.

| Variable | Default | Effect |
|---|---|---|
| `WIKILENS_QUERY_REWRITE` | on | Set to `0`/`false`/`off`/`no` to disable the LLM query rewrite entirely. |
| `WIKILENS_TITLE_INDEX` | on | Set to `0`/`false`/`off`/`no` to disable the last-resort fuzzy match of short queries against the game's page titles (it only fires when every search returned nothing). |
| `WIKILENS_TRACE_RETRIEVAL` | off | Set (to anything) to log each retrieval round as a JSON line on stderr for offline evaluation — public wiki data only, never keys or answer text. |

The picked model drives both the answer and the rewrite (Default mode: the
answer model, or `WIKILENS_DEFAULT_REWRITE_MODEL` when set). Two behaviors are
automatic, with no variable to set: if that model is known to be a *reasoning*
model (those think out loud and return unusable rewrites), the rewrite is
skipped at zero cost — picking a fast non-reasoning model is how you keep it.
And after two consecutive failed rewrites, a per-session circuit breaker stops
further attempts and prints a one-time notice to the terminal naming the fix;
restarting WikiLens resets it.

## Debugging

Set `WIKILENS_DEBUG=1` to watch what each ask costs. You get two views of the
same data: a per-ask table on stderr (phase timings, models, token counts,
queries, page titles + char counts — printed even when an ask fails), and an
**always-on-top glass debug panel** — same look as the overlay — with live
progress bars per phase, expandable details, and a history of the session's
asks. Like the overlay, it starts hidden: show or hide it with the **Debug**
chip in the overlay's footer (or the tray's "Show debug panel"); it appears
at the top-left and you can drag it anywhere by its header. It never takes
keyboard focus from your game, and it records asks even while hidden, so
nothing is lost before you open it or after you close it. Neither view ever
shows wiki text or API keys.

## Commands

Run frontend/Tauri commands from the repo root; run `cargo` commands in `src-tauri/`.

| Task | Command |
|---|---|
| Run the app (dev) | `npm install` then `npm run tauri dev` |
| Build a release installer | `npm run tauri build` |
| Bump the release version (release PR only) | `npm run bump 0.2.0` |
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
4. Open a PR — title: an imperative, sentence-case headline (no `type:` prefix);
   description per `.github/PULL_REQUEST_TEMPLATE.md` (plain-language summary,
   behavior-level bullets, technical details quoted); CI
   (`.github/workflows/ci.yml`) must be green before merging.
5. Merge with a **merge commit** (never squash) — the planning vault pins commit
   hashes from PR branches, and squashing would orphan them.

## Using it

- **Ctrl+`** (the key left of 1) — show/hide the overlay from anywhere (also
  available from the tray).
- Pick your **LLM provider** and **game** from the dropdowns in the header; both
  choices are remembered between sessions.
- When shown, the panel takes focus so you can type immediately.
- **Enter** sends your question; **Shift+Enter** adds a newline.
- **Esc** (or Ctrl+` again) hides the panel; focus returns to the game.
- **Changing the shortcuts:** the header's gear opens **Settings** — in its
  Shortcuts section, press **Change** on a row, then press the new combo (Esc
  cancels). Both the summon and capture shortcuts are configurable; choices
  persist in `settings.json` in the app-data dir, and **Reset** restores a
  default.
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

> Maintainers: this is the quick first-run walkthrough. The full pre-release
> checklist (hotkey, tray, DPI, transparency, Esc layering, packaged build) is
> [`docs/smoke-checklist.md`](docs/smoke-checklist.md), and the release walk
> itself is [`docs/release.md`](docs/release.md).

1. From the repo root: `npm install` then `npm run tauri dev`.
2. Wait for the tray icon to appear (the window starts hidden), then press
   **Ctrl+`** (the key left of 1).
3. The panel slides in from the right and focuses the input.
4. Add a key: the header gear (**Settings**) → **Answers** — click a provider's
   line, paste its key in the field that opens, and press **Save** (the key is
   never displayed again). Then click **Your own provider** to answer with it,
   and pick which provider from the footer chip.
5. Choose a **game**, then ask a question — e.g. **Conan Exiles** → *"how do I
   make steel bars?"*, or **Core Keeper** → *"best way to get wood"*.
6. Expect the status to move through *Searching → Reading → Answering*, the answer
   to **stream in** as markdown, and **2–4 source links** to appear beneath it.
   Clicking a source opens the wiki page in your browser.
7. Try the other providers (whichever keys you added) — the same question should
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
- **Global shortcuts swallow their combo system-wide.** That's why the defaults
  are Ctrl-based: the original **Shift+C** summon key made a capital **C**
  untypeable everywhere (registered as a bare Shift+letter, it fired instead of
  typing) — resolved by the **Ctrl+`** default, and the reason the shortcut
  recorder refuses Shift-only combos.
- Answers are only as good as the wiki. WikiLens will say so when the pages don't
  contain the answer, rather than guessing.

## Security & privacy

- Your provider API keys are stored encrypted for your Windows user (DPAPI, in
  `keys.json` in the app-data dir — another account or machine can't read them)
  and stay in the Rust process; a pasted key crosses to Rust once and is never
  sent back to the webview, displayed, or written to logs.
- The webview is locked down by CSP to talk only to the local Tauri IPC — it has
  no direct network, HTTP, or filesystem access. Every outbound request (wiki +
  the selected LLM provider) is made from Rust.
- Your question and the fetched wiki excerpts are sent to the LLM provider you
  select, to generate the answer.
