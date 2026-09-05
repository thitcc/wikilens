# WikiLens

[![CI](https://github.com/thitcc/wikilens/actions/workflows/ci.yml/badge.svg)](https://github.com/thitcc/wikilens/actions/workflows/ci.yml)

A Windows overlay that answers your in-game question from the game's own wiki —
without alt-tabbing. Press a hotkey over the running game, type the question,
get a short answer drawn **only** from the wiki, with links to the pages it came
from.

<!-- screenshot: docs/screenshot.png — the panel over a running game (to be captured) -->

**How it works:** hotkey → a glass panel slides in and takes focus → you type →
WikiLens searches the game's MediaWiki, reads the top pages, hands them to a
language model with orders to answer *only* from that text, and streams the
answer back with its sources. If the pages don't contain the answer, it says so
instead of guessing.

## Requirements

- **Windows 10/11** (the only supported target).
- **WebView2 runtime** — preinstalled on Windows 11 and current Windows 10; if
  missing, install the [Evergreen WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
- A model to answer with: an **API key** for Anthropic, DeepSeek or OpenRouter,
  **or** a local OpenAI-compatible server (Ollama, LM Studio, llama.cpp, vLLM).
- A game running in **borderless or windowed** mode — exclusive fullscreen paints
  over every overlay.

## Install

Download the installer from the [Releases page](https://github.com/thitcc/wikilens/releases):
`WikiLens_<version>_x64-setup.exe` (bootstraps WebView2 if needed) or the
`.msi`. Prefer building from source? See below.

The installers are **unsigned**, so Windows SmartScreen will say *"Windows
protected your PC — Unknown publisher"*: click **More info → Run anyway**. A
code-signing certificate is a recurring cost this hobby project doesn't carry.
Antivirus heuristics may also take a second look — a global hotkey plus screen
capture is the same behavioral profile as a keylogger — and the trust-but-verify
path is to build it yourself from this repo.

## First run

1. Launch WikiLens. Nothing opens: it lives in the **system tray**.
2. Press **Ctrl+`** (the key left of 1). The panel slides in and focuses the input.
3. Open the header gear (**Settings**) and pick where answers come from — next section.
4. Pick a **game** from the chip in the header, ask something, and watch the
   status move through *Searching → Reading → Answering* while the answer
   streams in with 2–4 source links beneath it.
5. **Esc** (or Ctrl+` again) hides the panel; focus returns to the game.

## Where answers come from

The **Answers** stepper in Settings names which model writes your answers:
**Custom API** (your own cloud provider) or **Local AI** (a server on your
machine). Exactly one is the value.

**Custom API.** The provider lines below the stepper — Anthropic, DeepSeek,
OpenRouter — only hold keys: click one, paste the key in the field that opens,
**Save**. Saving stores the key and nothing else; which provider actually
answers is the **model chip in the panel footer**, where you also pick the
model. Keys are read at ask time, so adding one needs no restart. Without a key
for the selected provider the app answers *"No API key for `<provider>` yet —
add one in Settings."*

| Provider | Default model | Model override var |
|---|---|---|
| Anthropic | `claude-haiku-4-5-20251001` | `WIKILENS_ANTHROPIC_MODEL` |
| DeepSeek | `deepseek-v4-flash` | `WIKILENS_DEEPSEEK_MODEL` |
| OpenRouter | `openai/gpt-4o-mini` | `WIKILENS_OPENROUTER_MODEL` |

Model ids drift; set that provider's `WIKILENS_*_MODEL` variable to point at a
different default without touching code (an explicit footer pick still wins).

### Local AI mode

Answer from a model running on your own machine — no key, no cloud. Pick
**Local AI** in Settings → **Answers** and WikiLens talks to an
OpenAI-compatible server at the address on the row beneath (default
`http://localhost:11434/v1`, Ollama's). The footer chip then lists the
models your server actually has loaded — pick one there like any provider.

- **Server address**: paste and **Save**. Scheme-less pastes work
  (`localhost:11434` becomes `http://localhost:11434/v1`; a routable
  hostname defaults to `https://` instead — type `http://` explicitly for a
  LAN proxy); a bare origin gains `/v1`; a pasted full endpoint
  (`…/v1/chat/completions`, the URL LM Studio hands out) is trimmed back to
  its base; any other path (a `/api/v1` proxy) is kept as typed. Clearing
  the field restores the default. This covers **Ollama**, **LM Studio**
  (`http://localhost:1234/v1`), **llama.cpp server**, and **vLLM**.
- **API key (optional)**: only for servers started with one (LM Studio /
  llama.cpp `--api-key`). Stored DPAPI-encrypted like the provider keys;
  leave it unset for a stock Ollama.
- **Reads images** (the eye on the Local AI heading): local catalogs carry no
  capability metadata, so you declare it — flip the eye on for a vision model
  (LLaVA, Qwen-VL, …) and the capture chip arms; off means text-only.

Two timing notes for local servers: the first ask against a cold model waits
for it to load (WikiLens gives up after 60 seconds without a byte, which a
very large model can exceed), and the quick pre-search query rewrite has a
4-second budget a cold model will miss — after two misses WikiLens skips
rewrites for the session (asks still work). Keeping the model warm avoids
both: `ollama run <model>` before playing, or raise `OLLAMA_KEEP_ALIVE`
(Ollama unloads idle models after ~5 minutes).

## Using it

- **Ctrl+`** shows or hides the panel from anywhere (also in the tray menu).
  **Enter** sends the question; **Shift+Enter** adds a newline; **Esc** hides.
- **Game** is the chip in the header; **provider · model** is the chip in the
  footer. Both are owned menus, both picks are remembered between sessions.
- **Ask about what's on screen.** Press **Ctrl+Shift+C** (or the footer capture
  chip), drag a region, and it attaches to your next question as an image —
  vision-capable models only (rows marked *Image*; the eye, for Local AI).
- **The running game is offered, never forced.** Summon over a game WikiLens
  recognises and an accent chip beside the game chip names it; one click
  switches the wiki. Ignore it and nothing changes. Games it doesn't know —
  including ones you added yourself — produce no suggestion.
- **History.** The header's History button reopens any of your last 50 answers
  without asking again; **Clear history** wipes them.
- **Stop** (in the status row) halts a streaming answer and keeps what
  arrived; the header's **Start over** clears the panel for a fresh question.
- **Shortcuts.** Settings → Shortcuts: **Change** on a row, press the new combo
  (Esc cancels), **Reset** restores the default. Both the summon and capture
  shortcuts are configurable and persist across restarts.
- **Position.** The panel docks top-right by default. **Settings → Position**
  docks it to any corner or the center — or just drag it by its header, which
  flips Position to **Manual** and remembers the exact spot across restarts.
  Anchors and the dragged spot are remembered independently, so stepping
  between them restores each. Dropped near the bottom of the screen, the
  panel grows and opens its menus upward, like the bottom anchors. The
  padlock on the Position heading stops accidental drags (in every mode);
  the stepper keeps working while locked.
- **Theme.** Settings → Theme steps between **Default** and **Micrographics**;
  the pick applies live and is remembered.
- WikiLens only quits from the tray's **Quit** — closing the panel just hides it.

## Games

| Game | Wiki |
|---|---|
| Stardew Valley | stardewvalleywiki.com |
| Core Keeper | core-keeper.fandom.com |
| Conan Exiles | conanexiles.fandom.com |
| Warframe | wiki.warframe.com |
| Guild Wars 2 | wiki.guildwars2.com |
| Path of Exile | poewiki.net |
| Path of Exile 2 | poe2wiki.net |
| Abiotic Factor | abioticfactor.wiki.gg |
| Dave the Diver | dave-the-diver.fandom.com |
| The Elder Scrolls V: Skyrim | en.uesp.net |
| Fallout 4 | fallout.fandom.com |
| Grounded | grounded.wiki.gg |
| Grounded 2 | grounded.fandom.com |
| Terraria | terraria.wiki.gg |
| Minecraft | minecraft.wiki |

Stardew's self-hosted wiki uses the legacy search backend, so short keyword
questions (*"cauliflower"*) find pages more reliably there than full sentences.

**Add your own.** Any MediaWiki works: open the game menu, pick **Add a game…**,
paste the wiki's URL. WikiLens checks it live (site info plus one test search)
before saving, and the game stays across restarts. Built-in games are a
one-entry change in the source — see `CONTRIBUTING.md`.

<details>
<summary>How content is read</summary>

WikiLens reads each page's rendered HTML (`action=parse&prop=text`, one request
per page) and reduces it to plaintext, keeping infobox rows and data tables as
`label | value` lines — the numbers a wiki keeps in templates. If a page's
parse call fails or times out it falls back to one batched raw-wikitext request
(`prop=revisions`), which works on every MediaWiki but yields prose only, so a
stat living solely in an infobox can be missing on that path. `prop=extracts`
is avoided either way: many game wikis lack the TextExtracts extension, and
whole-article extracts are capped to a single page.

</details>

## Privacy

- Your provider API keys are stored encrypted for your Windows user (DPAPI, in
  `keys.json` in the app-data dir — another account or machine can't read them)
  and stay in the Rust process; a pasted key crosses to Rust once and is never
  sent back to the panel, displayed, or written to logs.
- The panel is locked down by CSP to talk only to the local Tauri IPC — it has
  no direct network, HTTP, or filesystem access. Every outbound request (wiki +
  the selected LLM provider) is made from Rust.
- Your question, the fetched wiki excerpts, and any screenshot you attached are
  sent to the model you selected, to generate the answer. The screenshot stays
  in the Rust process and goes out only with that one ask.
- Answer history lives in `history.json` in the app-data dir, local only.
- To suggest the game you're playing, WikiLens reads the name of the program
  owning the window it covers, each time you summon the panel. That stays on
  your machine: the executable's path and the window's title never leave the
  Rust process, and the only thing that reaches the panel is the id of a
  matched game (or nothing). It is never logged, never stored, and never sent
  to a provider. WikiLens asks Windows only for permission to read a process's
  name — never to read its memory.
- Wiki content is fetched at ask time and shown with links to its source pages;
  it stays under each wiki's own license (typically CC BY-SA).

## Build from source

Node.js **22**, a stable Rust toolchain, and the **Visual Studio C++ Build
Tools** (workload *Desktop development with C++* — the MSVC target links with
it; rustup's Windows installer offers to install it, and
[Tauri's prerequisites page](https://v2.tauri.app/start/prerequisites/#windows)
walks through it), then from the repo root:

```
npm install
npm run tauri dev      # dev build; reads .env
npm run tauri build    # installers under src-tauri/target/release/bundle/
```

<details>
<summary>Advanced: environment variables and debugging</summary>

Set these as OS environment variables (a dev build also reads `.env`; a
packaged build does not).

### Retrieval tuning (advanced)

Before answering, WikiLens runs a small retrieval pipeline: a raw keyword search
and an LLM **query rewrite** run concurrently, their hits are merged, and a local
fuzzy **title index** catches typos as a last resort. It works out of the box;
these variables are escape hatches for when you want to change or trace it.

| Variable | Default | Effect |
|---|---|---|
| `WIKILENS_QUERY_REWRITE` | on | Set to `0`/`false`/`off`/`no` to disable the LLM query rewrite entirely. |
| `WIKILENS_TITLE_INDEX` | on | Set to `0`/`false`/`off`/`no` to disable the last-resort fuzzy match of short queries against the game's page titles (it only fires when every search returned nothing). |
| `WIKILENS_TRACE_RETRIEVAL` | off | Set (to anything) to log each retrieval round as a JSON line on stderr for offline evaluation — public wiki data only, never keys or answer text. |

The picked model drives both the answer and the rewrite, in both modes. If
that model is known to be a *reasoning* model (they think out loud and return
unusable rewrites), the rewrite is skipped automatically; and after two
consecutive failed rewrites a per-session circuit breaker stops further
attempts and prints a one-time notice to the terminal. Restarting resets it.

### Debugging

Set `WIKILENS_DEBUG=1` to watch what each ask costs: a per-ask table on stderr
(phase timings, models, token counts, queries, page titles + char counts —
printed even when an ask fails) and an always-on-top glass **debug panel** with
live progress per phase and the session's ask history. It starts hidden — show
it with the **Debug** chip in the panel footer or the tray's "Show debug panel";
it never takes keyboard focus from your game. Neither view ever shows wiki text
or API keys.

### Migrating from the env-var era

`ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` / `OPENROUTER_API_KEY` are no longer
read. Paste those keys into **Settings → Answers** once and remove them from
your environment; WikiLens prints a one-line startup reminder while any legacy
variable is still set.

</details>

## Caveats

- **Borderless / windowed only.** Exclusive fullscreen paints over the overlay —
  expected, not a bug. Switch the game to borderless mode.
- **Windows only.** macOS and Linux are not targeted.
- **Global shortcuts swallow their combo system-wide**, which is why the
  defaults are Ctrl-based and the recorder refuses Shift-only combos (the
  original Shift+C summon key made a capital C untypeable everywhere).
- Answers are only as good as the wiki. WikiLens says so when the pages don't
  contain the answer, rather than guessing.

## Contributing · License

Bug reports and game requests use the issue forms; the development workflow,
verification commands, and how to add a built-in game are in
[`CONTRIBUTING.md`](CONTRIBUTING.md). Security reports: [`.github/SECURITY.md`](.github/SECURITY.md).

WikiLens is released under the [MIT License](LICENSE). The wiki pages kept as
test fixtures stay under their wikis' Creative Commons licenses — see
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
