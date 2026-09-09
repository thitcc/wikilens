---
title: Make Portuguese questions retrieve from the English wikis
type: plan
status: todo
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, rag, wiki, llm, testing]
related:
  - "[[2026-09-09_one-language-setting]]"
  - "[[2026-09-09_ptbr-feasibility-assessment]]"
  - "[[2026-09-09_ptbr-answer-language]]"
  - "[[2026-07-04_query-preprocessing-zero-hit-retry]]"
  - "[[2026-07-07_retrieval-quality-improvements]]"
  - "[[2026-07-10_rewrite-circuit-breaker]]"
  - "[[2026-07-10_reasoning-skip-and-capability-tags]]"
  - "[[2026-08-26_local-thinking-off-switch]]"
  - "[[2026-08-22_retrieval-eval-suite]]"
commit:
---

# Make Portuguese questions retrieve from the English wikis

## Context / problem

The feasibility assessment ([[2026-09-09_ptbr-feasibility-assessment]])
measured a split, not a wall. On the 5 Fandom-hosted built-ins
(UnifiedSearch: corekeeper, conanexiles, davethediver, fallout4, grounded2)
Portuguese sentences already retrieve: 11/11 got hits ("como fazer uma
fornalha" → Furnace on core-keeper.fandom.com, "armadura de poder" → Power
armor), Core Keeper 5/6 machine-judged relevant. That is a third-party
leniency with no contract — nothing in the app asked for it. On the 10
non-Fandom built-ins, 1/18 Portuguese sentences got any hit (0 relevant)
against 18/18 for the same questions in English: stardew pt 0/7 vs en 7/7,
minecraft pt 1/6 (via a mangled did-you-mean, 4 irrelevant pages) vs en 6/6.
Bare Portuguese nouns fare better (12/19 overall; 6/13 on non-Fandom — 3
identical proper nouns/acronyms, 1 cognate via the title index, 1 Cirrus
suggestion, 1 wiki redirect), but players type sentences.

Every deterministic rung is English-only. `preprocess_query`
(`wiki/search.rs`) strips a 54-entry English `STOPWORDS` list — of 71
Portuguese function words sampled, 4 drop by coincidence — so como/onde/para
ride into `srsearch`, and under AND semantics a word no English page contains
excludes every page. `simplify_query` keeps ≥4-char or Capitalised tokens,
which lets the four-letter Portuguese function words through. The
did-you-mean retry trusts the suggestion verbatim. The title index
(`wiki/titles.rs`: `MATCH_THRESHOLD` 0.92, `LENGTH_RATIO_FLOOR` 0.6,
`MAX_TITLES` 6,000) needs a cognate: bomba → Bomb fired, fornalha/Furnace
scores 0.72, ferro/Iron 0.48. Walking the full ladder on 19 Portuguese asks
over stardew/minecraft/corekeeper: 12 dead ends (the canned "I couldn't find
anything on the {} wiki for that…" in `run_ask`), simplify retries 0/8 hit,
title-index attempts 0/10 on sentences.

The LLM rewrite (`rewrite_query`, `llm.rs`) is the only stage that can
translate, and `REWRITE_SYSTEM_PROMPT` says nothing about language (0
language words in 841 chars). Six conditions make it yield nothing, each
leaving the English rungs alone with a Portuguese query:
`WIKILENS_QUERY_REWRITE` off (`stage_enabled`); the known-reasoning skip (the
`rewrite_skip_reasoning` flag on `AskTargets` — DeepSeek's `default_model`
`deepseek-v4-flash` is curated `reasoning: true` in `providers.rs`, so for
that provider's default the skip is permanent,
[[2026-07-10_reasoning-skip-and-capability-tags]]); the session breaker
(`rewrite_breaker_tripped`, `REWRITE_BREAKER_LIMIT` 2,
[[2026-07-10_rewrite-circuit-breaker]]); the 4 s `REWRITE_TIMEOUT` or a
non-2xx; an unparseable body (`parse_rewrite_queries` → `[]`); and the echo
filter in `run_ask`, which drops candidates equal (ASCII case-insensitive)
to the preprocessed query before `REWRITE_SEARCH_LIMIT` takes its two.

Nothing measures any of this. `eval/questions.json` holds 221 questions (87
with facts), 0 in Portuguese, 0 `lang` fields; the judge prompt
(`JUDGE_SYSTEM`, `eval/run-answer.mjs`) is English-instructed. CLAUDE.md §3
gates every retrieval change on a before/after eval run (a 3-round run ≈ 25
min at the runner's 300 ms `--pace`), so a Portuguese fixture must exist
before any code below can be judged. The assessment made zero LLM calls: how
a given model translates under the rewrite prompt is unmeasured.

## Goal / non-goals

- Goal: with `language = pt-BR` (the setting [[2026-09-09_one-language-setting]]
  defines and [[2026-09-09_ptbr-answer-language]] ships), a Portuguese
  question on a non-Fandom wiki reaches the right English page whenever the
  rewrite runs, and fails honestly — a dead-end answer that names the cause —
  when it cannot.
- Goal: the English path stays byte-identical (every change sits behind the
  language value), and the Fandom third keeps its baseline hit rate.
- Goal: a Portuguese eval fixture and a `byLang` report, so this and every
  later retrieval change is measured in both languages.
- Non-goal: routing to Portuguese-language wikis
  ([[2026-09-09_ptbr-portuguese-wikis]]).
- Non-goal: a separate translation round-trip on every ask — the rewrite
  already paces the first phase ([[2026-08-17_core-pipeline-analysis]]);
  rejected as the default, revisitable.

## Approach

1. Portuguese fixture first — the gate. 30–60 hand-written player-voice
   Portuguese questions in `eval/questions.json`, each with `lang: "pt-BR"`
   (absent = `en`), stable ids, gold = the English page titles, stratified
   by the fixture's existing `wikis.<id>.engine` values (1 `default`, 9
   `cirrus`, 5 `unified` today). `lang` joins `style`/`source` as a closed
   vocabulary in
   `eval_fixture_questions_are_well_formed` (`config_guardrails.rs`; a
   `Value` read, so the field is additive). `eval/aggregate.mjs` gains
   `byLang` beside `byEngine`; `JUDGE_SYSTEM` in `eval/run-answer.mjs`
   learns the Portuguese abstain phrases so a pt dead end is labelled
   `abstain`, not `wrong`; `eval/README.md` "Growing the set" gains a
   Languages paragraph. Baseline run before any code — expect the dead-end
   numbers above.
2. Rewrite prompt. One sentence appended to `REWRITE_SYSTEM_PROMPT` only when
   `pt-BR` is active (the conditional-addendum idiom plan 1 uses on the
   answer prompt): the wiki is written in English, so every query must use
   the wiki's English page names — translate common nouns, keep proper nouns
   as the player wrote them. `rewrite_query` takes the language; the English
   call sends the byte-identical prompt. Mirror the addendum in `eval/lib.mjs`
   (its `REWRITE_SYSTEM_PROMPT` is a hand copy) and let the prompt-parity
   test plan 1 adds pin both.
3. Portuguese stopwords. A second, language-keyed list in `wiki/search.rs`
   (articles, prepositions, interrogatives, the common light verbs) that
   `preprocess_query` applies only for `pt-BR`, so English users never meet
   it. Every entry is vetted against all 15 built-in wikis' page titles per
   the `STOPWORDS` rule — an entry that is a real page title stays
   searchable. `simplify_query` inherits the fix through `preprocess_query`.
   Mirror in `eval/lib.mjs` (`preprocessQuery`, `simplifyQuery`) with
   vectors in `eval/lib.test.mjs` and the `search.rs` tests. Plainly:
   stopwords cannot rescue a common-noun question — "conseguir Excalibur" and
   "derrotar dragão" measured 0 hits, and no function-word list puts "dragão"
   on an English page; they only clean the input for the rewrite and the
   suggester.
4. Suggestion sanity, and the wrong comment. The did-you-mean retry in
   `run_ask` accepts any suggestion that differs from the query; add a pure
   check in `wiki/search.rs` that rejects a suggestion changing more than
   half the tokens (the minecraft pt case: a mangled suggestion returned 4
   irrelevant pages and scored as the wiki's one hit). Fix the `search_full`
   doc comment that lists Fandom among the CirrusSearch wikis returning
   `query.searchinfo.suggestion` — Fandom's UnifiedSearch omits `searchinfo`
   entirely (re-verified live during the assessment).
5. No-rewrite paths — honest, and rarer where cheap. The pt-BR variant of the
   zero-hit answer (plan 1 makes the canned copy language-selected) names the
   cause from the three flags already in scope at the dead end (`rewrite_on`,
   `rewrite_skip_reasoning`, `rewrite_breaker_tripped()`): this wiki is
   English and the query rewrite was off / skipped for a reasoning model /
   tripped this session — try English keywords. Local thinking-off is
   [[2026-08-26_local-thinking-off-switch]]: link it, don't re-plan it.
   DeepSeek's reasoning default is an open decision — force a translation leg
   for reasoning models when `pt-BR` is active, send the provider's
   thinking-off parameter, or accept the copy-only outcome. Recommended:
   copy-only first, then measure demand from `byLang` and history.
6. Threading. `run_ask` already reads `settings.mode()`; read the language
   beside it (the accessor plan 1 adds) and pass it to `preprocess_query`,
   `simplify_query` and `rewrite_query` — `AskTargets` stays as is. The
   debug table gains a `lang` header row (`DebugReport::new`, `debug.rs`) and
   the debug window a `lang` field on `AskStartedPayload` / `DebugAskStarted`
   (`src/types.ts`), updating the payload key-set pin in `debug.rs`; a
   language tag is a count-class value, no text crosses.
7. Verification. `/check`. English eval before/after (one 3-round run each;
   insurance on a byte-identical path). Portuguese fixture before/after with
   `byLang` and `byEngine` reported; acceptance:
   the `default` and `cirrus` strata move off their baseline, and the
   `unified` (Fandom) stratum's hit rate does not drop — that third already
   works today, and the pt stopwords plus the merged candidates run there
   too. Live smoke on the Local rig (the 5070 Ti): a Portuguese question
   through a small model, watching the breaker and the dead-end copy;
   `docs/smoke-checklist.md` §6 gains the pt rows.
8. Single PR off `feat/ptbr-question-retrieval`, after
   [[2026-09-09_ptbr-answer-language]] (it needs the setting). Docs: README
   "Retrieval tuning (advanced)" gains the best-effort sentence; CLAUDE.md §3
   names the pt fixture in the eval cadence, §4 the language-keyed stopwords.

## Decisions & trade-offs

- **Gated on the language setting over unconditional**: the English path is
  untouched, so the eval gate is trivially satisfied for English and the
  221-question fixture keeps its meaning; a pt-BR player must flip the
  stepper first — acceptable, documented in README.
- **Language-keyed stopwords over one global list**: the shadowing risk is
  global (a dropped token is dropped for every wiki and every player), the
  benefit is pt-only. Two lists, each vetted against the same 15 title sets.
- **Fandom cross-language matching as a bonus, not a contract**: it is
  UnifiedSearch behaviour, unversioned and unrequested; the plan measures it
  (the `unified` stratum) so a change there is noticed, but no code path
  depends on it.
- **Prompt-only translation, best-effort by construction**: one appended
  sentence rides a 4 s, 256-token, breaker-guarded call that six conditions
  can silence. README says so; a dedicated translation leg is the rejected
  default (latency), revisited only if pt dead ends dominate `byLang`.

## Status log

- 2026-09-09 — created as `todo` ([[2026-09-09_ptbr-feasibility-assessment]]);
  no code yet; runs after [[2026-09-09_ptbr-answer-language]].
