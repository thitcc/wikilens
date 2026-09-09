---
title: Localize the overlay UI and Rust-authored copy to Brazilian Portuguese
type: plan
status: todo
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, frontend, rust, overlay, testing]
related:
  - "[[2026-09-09_one-language-setting]]"
  - "[[2026-09-09_ptbr-feasibility-assessment]]"
  - "[[2026-09-09_ptbr-answer-language]]"
  - "[[2026-08-13_settings-stepper]]"
  - "[[2026-08-12_theme-switching-micrographics]]"
  - "[[2026-07-19_hotkey-config]]"
  - "[[2026-07-18_impeccable-design-context]]"
commit:
---

# Localize the overlay UI and Rust-authored copy to Brazilian Portuguese

## Context / problem

Layer 3 of the pt-BR assessment ([[2026-09-09_ptbr-feasibility-assessment]]):
once [[2026-09-09_ptbr-answer-language]] lands, a player who flips the
Language stepper gets Portuguese answers inside an English panel, and every
error Rust writes stays English. The counts, measured 2026-09-09:

- Frontend: 215 user-visible English strings in 20 files — 151 static, 42
  interpolated/plural/date, 20 labelling Rust wire values (`ask://status`
  and friends), 2 brand. Excluding the debug window (22, `WIKILENS_DEBUG`
  only), the brand (2) and the key-cap labels (10) leaves 181 player-facing
  sites in 15 files; heaviest `SettingsMenu.tsx` 78, `App.tsx` 32,
  `historyTime.ts` 17. No i18n layer (0 `Intl.*` calls). `<html lang="en">`
  is hardcoded in `index.html`, `capture.html` and `debug.html`; nothing
  sets `documentElement.lang`. The game menu sorts with an OS-locale
  `localeCompare` (`GameMenu.tsx`); the game and history filters are
  accent-sensitive `includes` (`GameMenu.tsx`, `HistoryMenu.tsx`). The
  panel is 420 CSS px (`window.rs`) with 5 CSS ellipsis guards; the hotkey
  row and the status row are un-truncated.
- Tests: 370 assertions in 25 files pin English copy, plus 15 through one
  regex constant (`HINT` in `App.slowHint.test.tsx`).
- Rust: 77 user-readable English strings across 15 files. `error.rs` has 18
  `AppError` variants; 7 are `#[error("{0}")]` pass-throughs (`LocalMode`,
  `Capture`, `VisionUnsupported`, `Probe`, `InvalidGame`, `Hotkey`,
  `BodyTooLarge`) composed at 23 construction sites outside tests, so the
  boundary — `impl From<AppError> for String` — cannot re-word them today.
  19 of the 33 `#[tauri::command]`s return `Result<T, String>` through 20
  `.map_err(String::from)` sites; 23 Rust tests (25 assertion lines) assert
  English error text. The tray is 5 OS-drawn, Rust-formatted strings:
  "Show/Hide overlay", "Show debug panel", "Quit", the summon tooltip
  (`summon_tooltip`, `tray.rs`) and the unavailable tooltip
  (`tooltip_unavailable`, `hotkey.rs`). `HistoryEntry.provider_name`
  freezes the display word "Local" (`LOCAL_TARGET_NAME`, `target.rs`) into
  `history.json`. `tauri.conf.json` has no `nsis.languages` / WiX language.

The only precedent for a presentation choice is the theme
([[2026-08-12_theme-switching-micrographics]]): frontend-only, `src/theme.ts`
plus `localStorage["wikilens.theme"]`. It cannot carry a language — tray,
canned answers and every error string are composed in Rust, which never
sees localStorage. The ADR [[2026-09-09_one-language-setting]] puts the value
Rust-side (`SettingsInfo.language`, `set_language`); plan 1 ships it with
its stepper, and this plan consumes it on every surface plan 1 leaves English.

## Goal / non-goals

- Goal: with `language = pt-BR`, every player-facing string in the overlay,
  the capture page, the tray and the Rust-authored errors is Portuguese,
  and the flip is live (no restart). Under `en` the rendered copy, the wire
  payloads and the stderr/debug output are byte-identical to today, and the
  suites keep running `en` by default — none of the 370 copy assertions or
  23 Rust text assertions move for the locale.
- Non-goal: the debug window — its 22 strings are dev-facing and exist only
  under `WIKILENS_DEBUG`; they stay English and `debug.html` keeps
  `lang="en"` truthfully.
- Non-goal: key-cap labels — `Ctrl`, `Alt`, `Shift`, `Win` and the default
  labels "Ctrl+`" / "Ctrl+Shift+C" in `hotkeys.ts` stay as printed on ABNT2
  keycaps. The 2 brand strings stay.
- Non-goal: the Micrographics register's marks — CSS `content:`
  pseudo-elements in `styles.css` (`/03`, brackets, `⌖`, `↗`), untranslated
  by construction. The one register literal, `MICRO_PROMPT_PLACEHOLDER`
  ("Ask_about_the_game…", `src/instrument.ts`), and the uppercased tracked
  labels are decided per line on the proof sheet (step 7); default: marks
  stay, words translate.
- Non-goal: the answer language (plan 1), the Portuguese retrieval path
  ([[2026-09-09_ptbr-question-retrieval]]) and Portuguese wikis.

## Approach

1. Frontend dictionary, no library. `src/i18n.ts` exports a typed
   `Messages` record per locale (`en` is the type; `pt-BR` must satisfy it)
   and a plain `t(key, params?)` that substitutes `{token}` placeholders.
   `App` owns `locale`, read from `SettingsInfo.language` (the ADR), and
   passes it down (a tiny context if drilling gets ugly). The existing theme
   effect in `App.tsx` also sets `document.documentElement.lang`, so
   `index.html`'s `lang="en"` becomes a first-paint value like the theme's.
   The capture page is static (`#hint` in `capture.html`, "Drag to capture ·
   Esc to cancel") and never reloads, so `capture.rs` emits
   `capture://armed` with a `{lang, hint}` payload instead of `()` and `arm`
   in `src/capture/main.ts` writes both — an event-contract change,
   documented in CLAUDE.md §4. `debug/main.tsx` is left alone (non-goal).
2. Sweep the 181 sites in 15 files. Module-scope copy tables become key
   tables or `(t) =>` builders: `ROLE_NAMES` and `POSITION_OPTIONS` in
   `SettingsMenu.tsx`, the inline `{id, name, label}` option arrays the
   Answers and Theme steppers receive, `STATUS_LABEL` in `App.tsx` (its keys
   are the `ask://status` wire values and stay; only the labels move). Rich
   messages that split-render around a JSX child become token messages: the
   `<kbd>` chip placeholder in `App.tsx` (`{keys}`), the provider key-help
   sentence around `help.host` (`{host}`), the local-key sentence around
   `<code>--api-key</code>` (`{flag}`). Plurals get `one`/`other` keys —
   `answerReadyLabel` in `App.tsx` ("Answer ready — N sources") is the
   player-facing case; the "N asks" counter is the debug window's and stays.
   `validateCombo` in `hotkeys.ts` returns reason codes instead of finished
   sentences; the armed row maps them to copy at render. `relativeTime` in
   `historyTime.ts` takes a locale — hand month and band tables per locale
   keep the file's stated goal (pure, fixed clocks, never OS-locale
   dependent); `Intl.*` with an explicit locale is the fallback. Menu sort
   moves to `Intl.Collator(locale, { sensitivity: "base" })`; the game and
   history filters NFD-fold both sides so "acao" finds "Ação".
3. Settings: the Language stepper from plan 1 now also flips the UI — the
   `set_language` result carries fresh `SettingsInfo` and `App` re-renders
   on it. Option names stay in their own language ("English", "Português
   (Brasil)").
4. Rust copy — design A, the owner's pick: Rust formats its own
   player-facing text by language. A new `copy.rs` holds a string table
   keyed by `Language` (`copy::msg(Language, key)`). The 7 `#[error("{0}")]`
   variants become typed reasons the boundary can format — e.g.
   `CaptureError::TooSmall`, `ProbeError::NoApiAt { base }`,
   `HotkeyError::Claimed { label }` — replacing the string composition at
   the 23 construction sites. `impl From<AppError> for String` formats
   through a new `AppError::user_message(Language)`, reading the language
   from a process-wide cell (`AtomicU8` / `OnceLock`-style) set on settings
   load in `.setup()` and again by `set_language`; the 19 `Result<T,
   String>` commands and their 20 `.map_err(String::from)` sites stay
   untouched. `Display` stays English for logs and the debug table;
   third-party tails (io, reqwest, provider bodies) ride after the
   translated sentence as a muted detail line, untranslated.
   `begin_capture` already emits `capture://error` via `String::from(e)`,
   so the boundary change covers it with no new code; the two canned
   answers in `commands.rs` (zero-hit, unreadable pages) and
   `friendly_image_error` in `llm.rs` read the same table (plan 1 seeds
   them with a `match`; this plan moves them into the table). Tray:
   `set_language` re-labels the `toggle` / `show-debug` / `quit` items via
   `MenuItem::set_text` and rebuilds the tooltip through
   `update_summon_tooltip`; `tooltip_unavailable` formats from the table
   too. `HistoryEntry.provider_name` persists the provider id (`local` in
   Local mode) and the frontend labels it — an unknown value renders as-is,
   so the "Local" / "Anthropic" words in existing `history.json` rows keep
   reading without a migration; update the
   `history_entry_serializes_exactly_the_known_fields` pin.
   Test hazard: `cargo test` runs tests as parallel threads in one process,
   so no test may flip the global cell — a pt-BR expectation would race
   every English one. pt-BR copy tests call the table directly
   (`copy::msg(Language::PtBr, key)`, `AppError::user_message(PtBr)`); the
   global is read at the IPC boundary only. That is what keeps the 23
   English-text assertions deterministic: with the cell untouched they run
   under `en` exactly as today.
   Glossary: Rust copy names UI locations (`MissingApiKey` says "add one in
   Settings", pinned by `missing_api_key_error_points_at_settings_not_a_value`),
   so keep a shared glossary of UI nouns (Settings, Answers, Position,
   Summon, the Image badge, Add a game…) that `copy.rs` and `i18n.ts` both
   follow; a Node parity test over the glossary is optional.
5. Installer: `bundle.windows.nsis.languages` (plus the WiX `language`) in
   `tauri.conf.json`; no CI change — `release.yml` builds from the config.
6. Tests. Vitest keeps `en` as the default locale (the fake backend in
   `src/test/backend.ts` answers `language: "en"`), so 0 of the 370 copy
   assertions move. About 18 structural edits are unavoidable: `historyTime`
   (12 — `MONTHS` and the band labels become locale-keyed), `hotkeys` (4 —
   reasons become codes), the harness (2 — the `^Game:` / `^Model:` regexes
   in `src/test/harness.tsx`). A small pt-BR smoke suite mounts with
   `language: "pt-BR"` and checks the status labels, one plural, one date
   label, `documentElement.lang`, and one Rust error rendered through the
   fake backend. Rust tests asserting English text stay green under `en`;
   add a table-completeness test on both sides (`copy.rs`, `i18n.ts`):
   every key in both locales, no `{token}` left unsubstituted.
7. Proof sheet (`.claude/skills/proof-sheet/SKILL.md`,
   [[2026-07-18_impeccable-design-context]]) with the longest pt-BR strings
   in both themes at 420 px — hotkey row, status row, badges, steppers, the
   Micrographics uppercase labels — decided mid-session; then device smoke
   at 100 % / 150 % DPI; add a pt-BR block to `docs/smoke-checklist.md`.
8. Docs: README (plan 1's Language paragraph grows to cover UI copy),
   CLAUDE.md §2 (`i18n.ts`, `copy.rs`), §4 (the `capture://armed` payload,
   an "add copy in both locales" convention), §5 (the test hazard as a
   gotcha), PRODUCT.md one sentence on language scope plus the glossary,
   DESIGN.md nothing — no token changes.
9. One PR off `feat/ptbr-ui-localization`, or two — frontend dictionary
   first, Rust copy second; the stepper already exists from plan 1, so the
   split is clean. After plan 1. Behaviour change: the stepper flips the UI.

## Decisions & trade-offs

- **Design A (Rust formats its own copy) over error codes (B)**: B would
  have Rust return machine codes and the frontend hold every string. Rust
  already owns the language and needs a table anyway for the tray tooltip
  and the canned answers, which no webview renders; B's cost is the IPC
  error contract of 19 `Result<T, String>` commands, the guardrail pins on
  the serialized shapes, and 23 Rust tests asserting text — all touched. A
  leaves them alone by reading one process-wide cell at the boundary; its
  cost is the test hazard and two dictionaries that must agree, paid with
  the glossary and the completeness tests.
- **No i18n dependency over i18next / FormatJS**: 215 strings, two locales,
  two plural forms, no ICU dates. A typed record and a `{token}` substitute
  cover it; a library adds a runtime and a license entry for nothing the
  table cannot express.
- **`en` as the default test locale over parametrised suites**: 370
  assertions stay as written; the pt-BR smoke suite proves the switch, the
  completeness test proves coverage. Running every suite twice would double
  the Vitest time to check what a table lookup already guarantees.
- **Fixed `en` first-run default over OS-locale sniffing**: the repo's
  suggest-only stance (game detection never changes the selection) — a
  Portuguese Windows is a hint, not a choice. A one-time "Português?" offer
  chip is a separate idea, per the ADR.
- **Debug window excluded over full coverage**: 22 strings behind a dev
  flag, read by whoever runs `WIKILENS_DEBUG=1`; translating them buys no
  player anything and costs a second dictionary bundle.

## Status log

- 2026-09-09 — created as `todo` from the feasibility assessment; layer 3
  of four. Sequenced after [[2026-09-09_ptbr-answer-language]], which lands
  the setting and the stepper this plan consumes.
