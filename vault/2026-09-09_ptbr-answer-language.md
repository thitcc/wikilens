---
title: Answer in Brazilian Portuguese from the English wikis
type: plan
status: todo
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, llm, rust, frontend]
related:
  - "[[2026-09-09_one-language-setting]]"
  - "[[2026-09-09_ptbr-feasibility-assessment]]"
  - "[[2026-09-09_ptbr-question-retrieval]]"
  - "[[2026-08-13_settings-stepper]]"
  - "[[2026-07-29_settings-modes-and-keys]]"
  - "[[2026-08-22_retrieval-eval-suite]]"
  - "[[2026-08-24_local-ai-mode]]"
commit:
---

# Answer in Brazilian Portuguese from the English wikis

## Context / problem

The answer language is undecided, not English by design. `SYSTEM_PROMPT`
(649 chars), `SCREENSHOT_ADDENDUM` (484) and `REWRITE_SYSTEM_PROMPT` (841) in
`llm.rs` carry zero language words: the model answers in whatever language
the context pulls it toward, and the context is tens of KB of English
excerpts with the question as its last line, so a Portuguese question mostly
gets an English answer. Three pieces of "the answer" are authored Rust-side
and English regardless of model: the zero-hit canned answer and the
unreadable-pages answer in `run_ask` (`commands.rs`), and
`friendly_image_error` in `llm.rs`. The prompt has one seam —
`system_prompt(has_image)` feeds both `build_anthropic_request` and
`build_openai_request` — so the change is small: the critic's Tier 1
estimate is hours (half a day to a day including two eval rounds and manual
smoke), ~6 files.

The honest caveat, measured in [[2026-09-09_ptbr-feasibility-assessment]]:
on the 10 non-Fandom built-ins a Portuguese sentence got any hit in 1/18
tries (0 relevant) against 18/18 for the same questions in English, so
without [[2026-09-09_ptbr-question-retrieval]] this plan is mostly "the
not-found message in Portuguese" there. It pays off today on the 5 Fandom
wikis (11/11 Portuguese sentences hit) and for a player who asks in English
but wants to read Portuguese. The assessment made zero LLM calls — how each
model behaves under the addendum is unmeasured, hence the smoke step.

## Goal / non-goals

- Goal: with `language = pt-BR` the model answers in Brazilian Portuguese,
  and every Rust-authored answer or error the player reads *as the answer*
  is Portuguese too.
- Goal: English asks send the byte-identical prompt they always have — `en`
  appends nothing anywhere, with or without a screenshot.
- Non-goal: translating the question for search
  ([[2026-09-09_ptbr-question-retrieval]]); UI copy
  ([[2026-09-09_ptbr-ui-localization]]); Portuguese wikis
  ([[2026-09-09_ptbr-portuguese-wikis]]).

## Approach

1. Settings seam per [[2026-09-09_one-language-setting]]: a `Language` enum
   in `settings.rs` cloned from `Mode` (`as_str`/`parse`, a raw
   `Option<String>` field on `SettingsFile` — today hotkeys, mode, position,
   local, extra — with tolerant load, persist-then-commit setter). `Mode`
   derives `#[serde(rename_all = "lowercase")]`, which cannot spell `pt-BR`:
   give that variant an explicit `#[serde(rename = "pt-BR")]` and let the
   wire pin (twin of `mode_serde_matches_the_stored_wire_strings`) catch a
   drift. `SettingsInfo.language` in `commands.rs`, a `set_language` command
   beside `set_mode`, one `invoke_handler` line in `lib.rs`; `types.ts` /
   `api.ts` twins (`setLanguage`); `src/test/backend.ts` fixtures; update the
   `settings_info_serializes_exactly_the_known_fields` pin in
   `config_guardrails.rs` (`{hotkeys, mode, localMode, position}` gains
   `language`). Per the ADR the value always crosses resolved — an unset
   file reads `"en"` Rust-side, never `null` the way `mode` does.
2. Settings UI: a fourth stepper, **Language**, after Theme, wired like
   Theme and Position — a `LANGUAGE_OPTIONS` table on the `POSITION_OPTIONS`
   shape, `LANGUAGE_POPOVER_ID` beside `THEME_POPOVER_ID`, `"language"` in
   the `openStepper` union, `prevLabel`/`nextLabel`/`valueLabel` copy. The
   options read `English` / `Português (Brasil)`, each named in its own
   language and left untranslated by convention (the row a player who cannot
   read the current UI still recognises). Until plan 3 lands the stepper
   governs answers only — README says so in as many words.
3. Prompt: a `LANGUAGE_ADDENDUM` const beside `SCREENSHOT_ADDENDUM` in
   `llm.rs`; `system_prompt(has_image, language)` appends it only for pt-BR;
   `answer_streaming` gains the `language` parameter; `run_ask` reads
   `settings.language()` next to `settings.mode()`, snapshotted once per ask.
   Both English paths stay byte-identical — `system_prompt(false, En)` is
   `SYSTEM_PROMPT` and `system_prompt(true, En)` is unchanged. One extra
   clause for pt-BR game clients rides with the screenshot addendum and only
   when both `has_image` and pt-BR hold. `REWRITE_SYSTEM_PROMPT` is not
   touched here — the rewrite is plan 2's. Recommended wording, liftable:

   > Answer in Brazilian Portuguese. Keep item, character, location and
   > mechanic names exactly as the wiki writes them, so they match the
   > sources shown with the answer; when you are sure of the Brazilian
   > Portuguese term, add it once in parentheses after the wiki name.

   and, appended to the screenshot addendum under pt-BR only:

   > The game UI in the screenshot may be in another language; match what
   > you see to the wiki's page names before answering.
4. Rust canned copy: a small `match` on the language at the three sites —
   the zero-hit and unreadable-pages answers in `run_ask` and
   `friendly_image_error`. The pt-BR dead-end copy tells the player the wiki
   is English and suggests English keywords — the bridge to plan 2, whose
   diagnostic variant replaces it. The Portuguese strings are drafted in the
   PR itself (this doc stays English).
5. Tokens: `MAX_TOKENS` is 1024, tuned for English; Portuguese runs longer
   for the same content. Raise it for pt-BR only (1280–1400) or globally,
   and read the eval's `stopReason` column (`stopMaxTokens` in
   `eval/aggregate.mjs`) before and after — a pt-BR-only raise leaves the
   English number untouched by construction.
6. Tests: update `system_prompt_gains_addendum_only_with_image` for the new
   signature and add its twin (`en` equals `SYSTEM_PROMPT`; pt-BR ends with
   `LANGUAGE_ADDENDUM`; image + pt-BR carries both addenda in order); a wire
   pin for `Language` on the `mode_serde_matches_the_stored_wire_strings` /
   `set_mode_persists_and_reloads` model; Vitest in `SettingsMenu.test.tsx`
   for the stepper (arrow cycle, popover pick, `set_language` invoked). Add
   the missing prompt-parity test: `eval/lib.mjs` hand-copies
   `REWRITE_SYSTEM_PROMPT` and `ANSWER_SYSTEM_PROMPT` (parity true today,
   pinned by nothing — `eval/lib.test.mjs` pins the preprocess/simplify
   vectors and `buildUserMessage` bytes only). A new `eval/*.test.mjs` reads
   the const literals out of `src-tauri/src/llm.rs`, unescapes the Rust
   `\"`, and compares — `npm run test:node` picks it up — then the mirror
   exports `LANGUAGE_ADDENDUM` the same way.
7. Docs: README — a **Language** bullet in the "Using it" list as the single
   source (answers only until plan 3); CLAUDE.md §1 Settings sentence, §2
   map (`settings.rs`, the `commands.rs` list), §4 (`SettingsInfo` field);
   `docs/ai-workflow.html` — its prompt quote is already stale (the caption
   pins a `llm.rs` line that has since moved, and the text predates the
   `<wiki_excerpt>` fencing), so refresh it or replace it with a link;
   `docs/smoke-checklist.md` rows under §6 Ask round-trip and §7 Answers
   (Anthropic + Local, pt-BR on).
8. Verification: `/check`; one English eval run before and one after
   (≈25 min each at the 300 ms pace) to prove the byte-identical path did
   not drift; manual pt-BR smoke on a cloud model and on the Local rig
   ([[2026-08-24_local-ai-mode]]) — small models follow the English context,
   so expect flakiness, and the eval's fixed cloud target never measures
   Local.
9. Single PR off `feat/ptbr-answer-language`; the behaviour change named in
   the PR: a new Settings stepper, and with pt-BR on, Portuguese answers and
   canned copy. `en` is unchanged.

## Decisions & trade-offs

- **Explicit setting over "match my question"**: detecting the question's
  language is flaky on short and proper-noun questions ("Excalibur",
  "Pip-Boy") and on small Local models, and the question is the last line
  after tens of KB of English excerpts — detection would fight the context.
  One value the player flips, owned per the ADR.
- **English wiki names with an optional gloss over localized client
  names**: the model cannot know the official pt-BR client terms without a
  Portuguese wiki ([[2026-09-09_ptbr-portuguese-wikis]]); wiki names keep
  the answer consistent with the Sources list under it.
- **Addendum outside `SYSTEM_PROMPT` over editing it**: the untrusted-excerpt
  fencing pin (`system_prompt_marks_excerpts_untrusted`) and the mirror's
  `ANSWER_SYSTEM_PROMPT` stay byte-for-byte; the language is a conditional
  tail exactly like the screenshot one.
- **History stays untagged over a `language` field now**: `HistoryEntry`
  keeps its shape and mixed-language rows are accepted; an optional field is
  a follow-up if the history menu ever wants to filter by it.

## Status log

- 2026-09-09 — created as `todo` from the feasibility assessment
  ([[2026-09-09_ptbr-feasibility-assessment]]); depends on the setting in
  [[2026-09-09_one-language-setting]]. Plan 2 follows it.
