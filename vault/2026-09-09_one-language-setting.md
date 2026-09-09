---
title: One Rust-owned language setting drives answers, retrieval and UI copy
type: decision
status: done
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, rust, frontend, tauri]
related:
  - "[[2026-09-09_ptbr-feasibility-assessment]]"
  - "[[2026-09-09_ptbr-answer-language]]"
  - "[[2026-09-09_ptbr-question-retrieval]]"
  - "[[2026-09-09_ptbr-ui-localization]]"
  - "[[2026-08-12_theme-persistence-localstorage]]"
  - "[[2026-07-29_keys-are-not-a-mode-choice]]"
  - "[[2026-08-24_local-ai-mode]]"
commit:
---

# One Rust-owned language setting drives answers, retrieval and UI copy

## Context
The pt-BR feasibility assessment ([[2026-09-09_ptbr-feasibility-assessment]])
split "pt-BR support" into four layers, and its layer readers each proposed
a different owner for "the language value": a UI locale in localStorage (the
theme pattern), a Rust `Language` in `settings.json`, an answer-language
switch inside the prompt, and a wiki-language preference on the game picker.
Left alone, the three plans would each mint their own switch.

The value cannot live only in the webview, because most of what must read
it is Rust. The answer prompt is `SYSTEM_PROMPT` in `llm.rs`, assembled by
the single `system_prompt()` seam that feeds both `build_anthropic_request`
and `build_openai_request`; the rewrite prompt `REWRITE_SYSTEM_PROMPT` — the
only translating stage of retrieval — sits beside it. Two answers the player
reads as "the answer" are authored in Rust, not by the model: the zero-hit
"I couldn't find anything on the {} wiki for that…" and the unreadable-pages
copy in `commands.rs`; so is `friendly_image_error` in `llm.rs`. The tray
items ("Show/Hide overlay", "Show debug panel", "Quit") and the tooltip from
`summon_tooltip` / `tooltip_unavailable` are OS-drawn, formatted in
`tray.rs` / `hotkey.rs`, where no webview string reaches. None of these
receive a language today: `ask` takes `game_id`, `provider_id`, `model`,
`question`, `image_id` and nothing else, and `SettingsFile` holds
`hotkeys`, `mode`, `position`, `local` and the flattened `extra` map — no
theme, no language.

The one precedent for a persisted player preference went the other way on
purpose: [[2026-08-12_theme-persistence-localstorage]] keeps the theme in
`src/theme.ts` under `wikilens.theme` because Rust never reads a theme. Its
rationale — "a UI pick, not behavior config" — is exactly what fails here:
the language changes what the model is told and what Rust writes.

## Decision
A single `language` value (`en` | `pt-BR`, default `en`) is persisted in
`settings.json` by `SettingsStore`, exposed as `SettingsInfo.language`, set
by a `set_language` command, and cloned from the `Mode` chain.

The clone is mechanical: a `Language` enum in `settings.rs` with
`as_str`/`parse` like `Mode`'s; a raw `Option<String>` file field loaded
tolerantly the way `resolve_mode` degrades an unknown mode, so a value a
future version wrote falls back alone instead of tripping the corrupt-file
path; a persist-then-commit setter shaped like `SettingsStore::set_mode`;
one `invoke_handler` line in `lib.rs` beside `commands::set_mode`; the
`api.ts` / `types.ts` twins; and the key set pinned by
`settings_info_serializes_exactly_the_known_fields` in
`config_guardrails.rs` growing by one. Unlike `mode`, which crosses as
`null` until chosen, `language` always crosses resolved — an unset file
reads `en` Rust-side — because with a default there is nothing for a null
to distinguish. `run_ask` snapshots it per ask next to `settings.mode()`.

That one value drives three things, each its own plan: the answer language
and the Rust-authored canned copy ([[2026-09-09_ptbr-answer-language]]); the
Portuguese retrieval path — pt stopwords, the rewrite's translate-to-English
clause, the diagnostic dead-end copy ([[2026-09-09_ptbr-question-retrieval]]);
and the frontend dictionary plus Rust error copy and tray strings
([[2026-09-09_ptbr-ui-localization]]).

It is **not** the wiki's language. Which wiki a game reads is a
per-`GameWiki` attribute of the registry — today all 15 built-ins are
English hosts and `GameWiki` has no `lang` field — and Portuguese editions
are the separate idea [[2026-09-09_ptbr-portuguese-wikis]]. One control per
job, as [[2026-07-29_keys-are-not-a-mode-choice]] put it: a pt-BR player
reading an English wiki is the designed case, not a gap.

## Consequences
- Good: one stepper in Settings, one stored value, one wire field; Rust and
  the frontend agree by construction because both read the same store, the
  way the Answers stepper's `mode` already works ([[2026-08-24_local-ai-mode]]).
  The default path stays byte-identical — `en` appends nothing to any
  prompt and selects the copy that exists today — so every English test and
  the English eval baseline keep passing. The value rides the same
  `settings.json` envelope as the hotkeys, mode and position, so a future
  export/import carries it.
- Cost / bad: the frontend learns the locale from `SettingsInfo`, which
  arrives asynchronously from `get_settings`, not from a synchronous
  localStorage read like the theme. Until it lands the panel paints `en`.
  Mitigation: the idiom `App.tsx` already uses for the mode
  (`settings?.mode ?? "custom"`) becomes `settings?.language ?? "en"`, and
  the window is created hidden, so the settings resolve before the first
  summon shows the panel. If that English frame is ever visible, cache the
  last value in localStorage purely as a paint hint — the store stays owner.
- Cost / bad: one more `#[tauri::command]`, IPC twin and test-backend
  fixture line — the cost the theme ADR avoided, accepted here because the
  field buys behaviour, not chrome.
- Follow-ups: no OS-locale sniffing. The app never guesses a preference on
  the player's behalf — the suggest-only stance foreground detection took in
  [[2026-08-04_game-auto-detection]] — so a fresh install is `en` until the
  stepper is moved; a one-time "Português?" offer chip is a separate idea,
  not part of this decision. The exact wire strings `"en"` and `"pt-BR"`
  get a pin like `mode_serde_matches_the_stored_wire_strings`: `Mode`
  relies on `#[serde(rename_all = "lowercase")]`, which cannot spell
  `pt-BR`, so this is the one place the clone is not byte-for-byte and the
  serde attribute must be written out.

## Alternatives considered
- **Frontend-only localStorage, the theme pattern** — zero Rust, zero IPC,
  one `wikilens.language` key beside `wikilens.theme`. Rejected: the answer
  prompt, the two canned answers, `friendly_image_error` and the tray stay
  English, and fixing any of them means a second channel to push the value
  into Rust — an argument or an event — at which point the webview holds the
  master copy of behaviour config and Rust a cache of it, the opposite of
  how `mode` is owned today.
- **Independent per-layer values** — each plan owns its own switch (a prompt
  toggle, a retrieval toggle, a UI locale). Rejected: three PRs each design
  persistence and UI for the same bit, there is no single stepper, and the
  combinations nobody wants (Portuguese answers, English dead-end copy)
  become reachable states that need copy and tests.
- **A `language` argument on `ask`** — keeps localStorage as the owner and
  threads the value per request. Rejected: it covers the prompt and the
  canned answers but not the tray menu or tooltip, which are built outside
  any ask, and it makes the answer language a webview-state artefact that
  the history, the debug table and any second caller of `ask` cannot see.
