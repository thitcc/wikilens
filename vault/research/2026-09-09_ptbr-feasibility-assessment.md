---
title: pt-BR feasibility — what "only English works" actually means, measured
type: research
status: done
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, wiki, rag, llm, frontend]
related:
  - "[[2026-09-09_one-language-setting]]"
  - "[[2026-09-09_ptbr-answer-language]]"
  - "[[2026-09-09_ptbr-question-retrieval]]"
  - "[[2026-09-09_ptbr-ui-localization]]"
  - "[[2026-09-09_ptbr-portuguese-wikis]]"
  - "[[2026-08-22_retrieval-eval-suite]]"
  - "[[2026-08-17_core-pipeline-analysis]]"
  - "[[2026-07-04_query-preprocessing-zero-hit-retry]]"
commit: 2ae431a
---

# pt-BR feasibility — what "only English works" actually means, measured

Evidence doc for the pt-BR support plans. Prompted by the owner's question
"how hard would pt-BR support be"; answered on 2026-09-09 by six layer
readers (frontend copy, Rust copy, retrieval chain, LLM prompts, wiki
registry/probe/eval/docs, Portuguese-wiki availability) plus a critic,
against `main` at `0780619`. Every file:line below was re-opened in that tree
before being written down; live numbers come from the probes under Method.
The three plans, the idea and the ADR link back here instead of repeating
the counts.

## Question

How hard is pt-BR support, and what exactly does the app assume about
English, layer by layer? The premise going in — "only English works" — turned
out partly wrong and partly worse than stated: Fandom's search already
matches Portuguese questions on 5 of the 15 built-in wikis, while on the
other 10 a Portuguese sentence retrieves nothing and no deterministic
fallback can rescue it; the answer language is undecided by the prompt rather
than English by design; and "pt-BR support" is four different deliverables
whose cost runs from hours to weeks.

## Method

- Six readers in parallel, one layer each, returning findings with
  file:line and a fix sketch; one critic re-checked every headline against
  the tree and reconciled the readers' contradictions.
- 176 live read-only GETs, polite User-Agent
  (`WikiLens-assessment/0.1 (feasibility probe …)`):
  - 135 in the search experiment over 9 built-ins (72 search + 25
    `list=allpages` + 9 langlinks/pt-wiki checks in the ladder probe; 20
    search in the raw-rung probe; 2 reverse langlinks; 1 raw Fandom curl +
    1 HEAD; 5 follow-ups), 300 ms pace. It replays the production zero-hit
    ladder — raw search → did-you-mean → `simplify_query` → title index,
    `srwhat=text` iff more than one word — with `preprocess_query`,
    `simplify_query`, `normalize` and `jaro_winkler` copied verbatim into a
    scratch binary. 29 Portuguese questions, each with an English twin; 19
    through the full ladder on stardew, minecraft and corekeeper, the other
    10 raw rung only.
  - 40 over 21 candidate Portuguese hosts (siteinfo + statistics +
    langlinks, `list=recentchanges` since 2026-08-10, one search), max 3 per
    host, 2 s between requests.
  - 1 by the critic: `como fazer uma fornalha` on core-keeper.fandom.com,
    re-run to settle the Fandom `searchinfo` contradiction.
- gw2 was skipped everywhere — wiki.guildwars2.com WAF-blocks this network.
- Zero LLM calls. Whether any model translates a Portuguese question
  implicitly, or answers in Portuguese, is unmeasured; every statement about
  model behaviour below is inferred from the prompts, not observed.
- String and test counts come from scripted extraction (a TypeScript AST
  walk for the frontend, `grep`/`rg` classification for Rust) plus hand
  review; the raw inventories stayed in the session scratchpad.

## Findings — answer language

The prompts never name a language. `SYSTEM_PROMPT` (`llm.rs:36`, 649 chars /
104 words), `SCREENSHOT_ADDENDUM` (`llm.rs:42`, 484 chars) and
`REWRITE_SYSTEM_PROMPT` (`llm.rs:423`, 841 chars) contain 0 occurrences of
language / English / Portuguese / translate. The player's question is the
last line of the user message (`build_user_message`, `llm.rs:379-391`, the
`Player question:` prefix at 388) after up to four 8000-char English
excerpts, so the answer language is whatever the model defaults to under an
overwhelmingly English context — model-dependent, not designed.

The seam is single: `system_prompt(has_image)` (`llm.rs:331-337`) is the one
formatter, called by `build_anthropic_request` (`llm.rs:224`; the call at
`:235` fills the top-level `system`) and `build_openai_request`
(`llm.rs:266`; the call at `:282`, system role). A `LANGUAGE_ADDENDUM`
appended there the way `SCREENSHOT_ADDENDUM` is lands once for every
provider and keeps the text-only English path byte-identical.

Three pieces of "the answer" are Rust-authored English no prompt can change:
the zero-hit canned answer (`commands.rs:1317-1320`, "I couldn't find
anything on the {} wiki for that…"), the unreadable-pages answer
(`commands.rs:1354`) and the two image-refusal strings in
`friendly_image_error` (`llm.rs:299-316`). On the 10 non-Fandom wikis the
first is the most common outcome for a Portuguese question (12 of 19
full-ladder asks, below).

`MAX_TOKENS = 1024` (`llm.rs:23`) was tuned for English output; the
Portuguese token-inflation ratio was not measured here.

Pin blast radius, measured: `llm.rs` has 47 tests (33 sync + 14 tokio); 2
assert on `SYSTEM_PROMPT` — `system_prompt_marks_excerpts_untrusted`
(`llm.rs:810-814`, three substrings) and
`system_prompt_gains_addendum_only_with_image` (`llm.rs:880-886`) — and only
the latter's two `system_prompt(…)` calls (`:881-882`) move for a signature
change. 4 insta snapshots exist, 0 pin a prompt. The eval mirror
hand-copies both prompts (`eval/lib.mjs:43` `REWRITE_SYSTEM_PROMPT`, `:46`
`ANSWER_SYSTEM_PROMPT`); they match Rust today (compared by script)
but no test pins the parity — `eval/lib.test.mjs` pins `preprocess` /
`simplify` vectors (11-32) and `buildUserMessage` bytes (125-128) only.
`docs/ai-workflow.html:499-504` already quotes a pre-2026-07-13 prompt.

## Findings — retrieval

Nothing in `run_ask` (`commands.rs:1022`) names a language, yet every
deterministic stage encodes English:

- `STOPWORDS` (`wiki/search.rs:26-33`) is 54 English entries. Of 71 sampled
  Portuguese function words, 4 are dropped — `a`, `as`, `do`, `me` — and
  only because they collide with English spellings. Under AND-semantics
  every surviving Portuguese token must appear on the English page.
- `simplify_query` (`search.rs:56-65`) keeps tokens of ≥4 chars or
  Capitalised, which inverts for Portuguese (long function words `como`,
  `para`, `onde`, `fazer`; short content nouns `aço`, `pá`): `como fazer
  aço` simplifies to `como fazer`.
- The title index (`wiki/titles.rs`) needs Jaro-Winkler ≥ 0.92
  (`MATCH_THRESHOLD`, `titles.rs:41`) with `LENGTH_RATIO_FLOOR` 0.6 (`:46`)
  over at most `MAX_TITLES` 6,000 alphabetical titles (`:26`). Translations
  never cross it: fornalha/Furnace 0.72, ferro/Iron 0.48; only
  near-identical cognates do (bomba → Bomb fired live on stardew). Indexes
  built during the experiment: stardew 2,813 titles, corekeeper 3,450,
  minecraft capped at 6,000 with the walk incomplete at "Banner patterns".
- CirrusSearch's did-you-mean spell-corrects Portuguese function words into
  English near-spellings (`code farmer uma fornalha`, `one encontrar fern`)
  — 6 of 6 minecraft Portuguese sentences got one, and the one retry that
  hit (`code farmer tnt`, 43 hits) returned 4 irrelevant pages the pipeline
  would have fetched and answered from.

The split, measured: on the 10 non-Fandom wikis (stardew = MediaWiki default
engine on MySQL; the rest CirrusSearch, gw2 unverified) 1 of 18 Portuguese
sentences got any hit (0 relevant) against 18 of 18 for the same questions
in English. On the 5 Fandom-hosted wikis (corekeeper, conanexiles,
davethediver, fallout4, grounded2 — UnifiedSearch) 11 of 11 Portuguese
sentences hit cross-language (`como fazer uma fornalha` → Furnace; `armadura
de poder` → Power armor; Core Keeper 5 of 6 machine-judged relevant) —
pt-BR already works there with zero code, as third-party behaviour with no
contract. The full ladder on 19 Portuguese asks ended in 12 dead ends; the
simplify retry fired 8 times and hit 0; the title index ran 10 times on
sentences and matched 0. Bare Portuguese nouns did better (12 of 19 hit),
but on non-Fandom wikis 6 of 13 — 3 identical proper nouns/acronyms, 1
cognate via the title index, 1 Cirrus suggestion, 1 wiki redirect (Appendix
A names them). Stripping function words alone is not a fix: `conseguir
Excalibur`, `derrotar dragão`, `Abigail gosta` all returned 0.

The LLM rewrite (`rewrite_query`, `llm.rs:440`; user turn
`Game: {game}\nPlayer question: {question}` at 446) is the only stage that
can translate, and six conditions make it yield nothing: (1)
`WIKILENS_QUERY_REWRITE` off (`commands.rs:1115`); (2) the known-reasoning
skip (`:1120`, `rewrite_skip_reasoning` resolved at `:1008`) — DeepSeek's
`default_model` `deepseek-v4-flash` (`providers.rs:92`) is curated
`reasoning: true` (`:100`), so this condition is permanent for that
provider's default; (3) the tripped session breaker (`:1127`;
`REWRITE_BREAKER_LIMIT` 2 at `state.rs:16`,
[[2026-07-10_rewrite-circuit-breaker]]); (4) a non-2xx or the 4 s
`REWRITE_TIMEOUT` (`llm.rs:410`, `:455-464`); (5) an unparseable body → `[]`
(`parse_rewrite_queries`, `llm.rs:567-585`); (6) the echo-of-raw filter
(`commands.rs:1204`). `REWRITE_MAX_TOKENS` is 256 (`llm.rs:400`). In every
one of the six a Portuguese question on a non-Fandom wiki is a guaranteed
dead end. Local AI is the worst case: `build_completion_openai`
(`llm.rs:502`) sends no `reasoning_effort`, and the Local reasoning skip is
the `:thinking` suffix only (`target.rs:230`, `models.rs:54-55`), so a
thinking-capable Ollama pick trips the breaker for the session
([[2026-08-26_local-thinking-off-switch]]).

Two notes for the code fall out of the probe: Fandom UnifiedSearch omits
`query.searchinfo` entirely (no `totalhits`, no `suggestion`), so the doc
comment at `search.rs:78-83` listing Fandom among suggestion-returning wikis
is wrong; and the screenshot path never touches retrieval — `image_png` goes
only to `llm::answer_streaming` (`commands.rs:1363`), the rewrite is
text-only.

Eval gate: `eval/questions.json` holds 221 questions (87 with facts), 0
Portuguese, 0 `lang` fields, 15 fixture wikis (14 built-in + palword; gw2
absent), engines tagged `default` 1 / `unified` 5 / `cirrus` 9. A 3-round
retrieval run took ≈ 25 min at the 300 ms pace (`eval/run-retrieval.mjs:43`;
`eval/out/r2` `meta.json` 22:58:27 → `round-3.jsonl` 23:24:01). The judge
prompt (`eval/run-answer.mjs:50`) is English-instructed; `aggregate.mjs` has
`bySource`/`byStyle`/`byGame`/`byEngine` (136-139) and no language
dimension; the eval target is a fixed cloud model, so Local AI is never
measured by it. Nothing about Portuguese retrieval can be measured until a
pt fixture exists ([[2026-08-22_retrieval-eval-suite]]).

## Findings — frontend copy

No i18n layer: `package.json` dependencies are `@tauri-apps/api`,
`@tauri-apps/plugin-opener`, `react`, `react-dom`, `react-markdown`; 0
`Intl.*` calls anywhere in `src/`. 215 user-visible English strings in 20
files — 151 static, 42 interpolated/plural/date, 20 labelling Rust wire
values (`ask://status` tokens → `STATUS_LABEL`, `src/App.tsx:152`, etc.),
2 brand. Excluding the debug window (22, `WIKILENS_DEBUG` only), brand (2)
and key-cap labels (10: Ctrl/Alt/Shift/Win, "Ctrl+`", "Ctrl+Shift+C" —
`src/hotkeys.ts:32-33`, `:167`) leaves 181 player-facing translatable sites
in 15 files. Heaviest: `SettingsMenu.tsx` 78, `App.tsx` 32,
`historyTime.ts` 17, `ModelMenu.tsx` 15, `hotkeys.ts` 14, `AddGameMenu.tsx`
13, `debug/AskCard.tsx` 13.

Structural English: sentences spliced around inline elements (the idle
placeholder's `<kbd>` chips, `App.tsx:1296`; the key-help sentences around a
link and a `<code>` element, `SettingsMenu.tsx:673`, `:715`); suffix plurals
("1 source" / "N sources"); `historyTime.ts` hand-tables English month
abbreviations (`MONTHS`, `:14`) and the `Nm ago` / `Jul 30` formats
(`:35-40`), deliberately avoiding `toLocaleDateString`; `validateCombo`
(`src/hotkeys.ts:111`) returns finished English sentences from a pure
module. Menu sort is OS-locale `localeCompare` with no locale argument
(`GameMenu.tsx:107`); filters are accent-sensitive `toLowerCase().includes`
(`GameMenu.tsx:117`, `HistoryMenu.tsx:91-92`, `ModelMenu.tsx:216-217`).
`<html lang="en">` is hardcoded on all three pages (`index.html:2`,
`capture.html:2`, `debug.html:2`) and nothing sets
`document.documentElement.lang`. The capture page's one string is static
HTML (`capture.html:58`).

Tests: 370 assertions in 25 files pin English copy, plus 15 through one
regex constant; 0 of them move if Vitest keeps the `en` locale — the
unavoidable churn is ~18 structural edits (`historyTime` 12, `hotkeys` 4,
harness 2 — `src/test/harness.tsx:18-19` gates 18 suites on `^Game:` and
`^Model:` prefix matches).

Layout: the panel is 420 CSS px (`window.rs:17`); 5 CSS ellipsis guards
exist, but `.hotkey-role` (`styles.css:1136`) and the status row are
un-truncated, and the Micrographics theme uppercases seven selectors
(`styles.css:1698-1826`) — jsdom cannot see overflow, so this is proof-sheet
and device-smoke territory. Theme is frontend-only (`src/theme.ts:7`,
localStorage `wikilens.theme`,
[[2026-08-12_theme-persistence-localstorage]]).

## Findings — Rust copy

77 user-readable English strings across 15 files (`commands.rs` 17,
`error.rs` 12, `capture.rs` 11, `keys.rs` 6, `http.rs` 5, `target.rs` 4,
`hotkey.rs` 4, the rest ≤ 3), reaching the player as `Result<T, String>`
command errors rendered verbatim, as the `capture://error` payload
(`commands.rs:819`), or as the two canned answers above. `AppError`
(`error.rs`) has 18 variants; 7 are `#[error("{0}")]` pass-throughs
(`LocalMode`, `Capture`, `VisionUnsupported`, `Probe`, `InvalidGame`,
`Hotkey`, `BodyTooLarge` — `error.rs:44`, `50`, `56`, `61`, `66`, `84`,
`89`) whose English is pre-formatted at 23 construction sites outside
tests (34 counting tests) with no settings in scope; `impl From<AppError>
for String` (`error.rs:124`) is the boundary. 19 of the 33
`#[tauri::command]`s return `Result<T, String>`; `commands.rs` has 20
`.map_err(String::from)` sites; 23 Rust tests (25 assertion lines) assert
English error text. Third-party tails (`io::Error`,
reqwest chains via `error_chain` at `error.rs:97`, provider bodies) are
already OS- or vendor-language and stay so under any design.

`SettingsFile` (`settings.rs:229-242`) persists `hotkeys{summon,capture}`,
`mode`, `position{mode,anchor,manual,locked}`, `local{base_url,vision}` and
the `extra` flatten — no theme, no language. The chain to clone for a
language value is `Mode`: enum + `as_str`/`parse` (`settings.rs:47-69`),
setter (`:463-471`), `SettingsInfo` (`commands.rs:140-146`) and the
`set_mode` command (`:645-653`), the `lib.rs:179` handler line, and the
`settings_info_serializes_exactly_the_known_fields` pin
(`config_guardrails.rs:379-394`, today `{hotkeys, mode, localMode,
position}`).

Outside IPC: the tray's five OS-drawn, Rust-formatted strings — "Show/Hide
overlay", "Show debug panel", "Quit" (`tray.rs:36`, `:40`, `:45`), the
tooltip "WikiLens — press {label} to open" (`tray.rs:23`) and its
unavailable variant (`hotkey.rs:136`). `HistoryEntry.provider_name`
(`history.rs:46`) freezes the display word "Local" (`LOCAL_TARGET_NAME`,
`target.rs:60`) into `history.json`; the entry's field set is pinned at
`config_guardrails.rs:475`. Rust copy also names UI locations by literal
("Settings → Answers", `target.rs:187-188`; "Image badge", `llm.rs:315`) —
a hidden coupling with the frontend dictionary. `tauri.conf.json` has no
`nsis.languages` / `wix.language`, so the installers are English.

## Findings — wiki registry and probe

`GameWiki` is `{id, name, api_url, page_url, search_namespace}` with no
language field (`wiki/games.rs:16-34`); `search_namespace` (`:32-33`,
additive `#[serde(default, skip_serializing_if)]`) is the precedent for
adding one. 15 built-ins (ids at `games.rs:55`–`150`), all English hosts.
The rendered-HTML reducer (`wiki/html.rs`) is language-neutral by
construction — its drop rules are CSS hooks (`toc`, `navbox`, `printfooter`,
`catlinks`, `pi-*`), 0 English text literals. The wikitext fallback is not:
`render_link_inner` drops only `file:`/`image:`/`category:`/`:category:`
(`wikitext.rs:129-132`) and `MAGIC_WORDS` lists 14 canonical forms
(`:26-41`), so on the degraded path a pt page leaks `Categoria:…` and
`__SEMTDC__` as prose.

The add-game probe carries a wrong-wiki trap: `normalize_base_url`
(`wiki/probe.rs:52-80`) keeps scheme + host and drops any path unless it
ends in `api.php`; `CANDIDATE_API_PATHS` (`:27`) is `/api.php`,
`/w/api.php`, `/mediawiki/api.php`; `candidate_domains` (`:303-316`) probes
only `{slug}.wiki.gg` / `{slug}.fandom.com` roots. A pasted Fandom
`/pt-br/wiki/…` or wiki.gg `/pt/wiki/…` page URL therefore resolves to the
English root `api.php`, passes validation, and is saved under the player's
Portuguese name. `parse_siteinfo` discards `general.lang` (`:112-117`)
although the response carries it. Subdomain-shaped hosts (pt.minecraft.wiki,
pt.stardewvalleywiki.com) do flow through — and then `add_game` mints
`id = slugify(name)` and refuses a built-in collision
(`commands.rs:313-321`), so "Minecraft" cannot be the name.

`GOLDEN_CASES` has 21 rows (`wiki/mod.rs:25`); the live runner
`golden_queries_hit_expected_pages` (`:119`) has 4 `.expect()` calls in its
loop (`:124-149`) and gw2 is row 10 (`:44`), so a pt golden appended after
it is unverifiable from this network; `every_builtin_game_has_a_golden_query`
(`:280`) demands one per new id. `detect/rules.rs` holds 31 language-neutral
rules with one `game_id` each, so a per-language twin would never be
suggested without a resolver ([[2026-08-04_game-auto-detection]]).

## Findings — Portuguese wikis

Supply, not code. Of the 15 games: 1 healthy — pt.minecraft.wiki (8,667
articles, 103 active users, >500 mainspace edits in 13 days). 2 thin but
alive — pt.stardewvalleywiki.com (1,983 articles, >500 edits by 2
translators in 9 days) and terraria.wiki.gg/pt (2,666 articles, 56 edits /
7 users in 30 d). 3 stale — warframe.fandom.com/pt-br (1 edit/30 d, top
pages last edited 2018-07 / 2019-02), fallout.fandom.com/pt-br (0 edits/30 d)
and pt.uesp.net for Skyrim (Portuguese prose under English titles, 0
edits/30 d). 1 stub — conanexiles.fandom.com/pt-br (318 articles). 7 none —
poe, poe2, corekeeper (`/pt-br` returns 410 Gone), grounded, grounded2,
davethediver, abioticfactor. 1 not probed — gw2. Hosts, paths and counts per
game in Appendix B. `siteinfo` `statistics.activeusers` is stale on MW 1.35
(Stardew reports 0 beside >500 edits) — `recentchanges` is the reliable
signal.

Structure: every Fandom/wiki.gg pt edition carries the language prefix in
both `api.php` and `articlepath` (`/pt-br/api.php` + `/pt-br/wiki/$1`), so
`page_url` must come from `articlepath`, never from the typed host. 5 pt
hosts expose EN langlinks (Fornalha 16 langs on pt.minecraft.wiki, Abacaxi
11 on pt.stardewvalleywiki.com, Guia 13 on terraria.wiki.gg/pt, Pip-Boy and
Ferreiro on the Fandom pt-br editions); Warframe pt-br and UESP-pt expose
none. The forward direction works too (stardewvalleywiki.com and
minecraft.wiki Furnace → pt Fornalha; core-keeper.fandom.com Furnace has
only `pl`), so a langlinks bridge is possible where both editions exist.

## What the critic corrected

- **"0 hits for Portuguese"** — refuted for the Fandom third of the registry
  (11/11 sentences hit; re-verified live: `como fazer uma fornalha` →
  `[Furnace, Cooking Pot]` with no `searchinfo` key). Holds for the 10
  non-Fandom wikis (1/18, 0 relevant).
- **15 built-ins, not 14** as the brief said (ids at `games.rs:55`–`150`);
  one reader's "16 English hosts" counted the `x.example` test fixture.
- **"A zero-code pt wiki via Add a game exists today"** — only for
  subdomain-shaped hosts (Minecraft, Stardew) or an exact `…/pt-br/api.php`
  paste, and even then the name collides with the built-in; a pasted Fandom
  or wiki.gg language-path URL saves the English wiki. Warframe pt-br,
  listed as usable by one reader, is stale — the availability data wins.
- **Theme is not the pattern for the language value.** All readers agree
  theme is frontend-only, but one recommended copying it; the answer
  prompt, both canned answers, `friendly_image_error` and the tray copy live
  in Rust and `ask` carries no language, so Rust must own the value — the
  ADR [[2026-09-09_one-language-setting]].
- **Smaller reconciliations**: `STOPWORDS` is at `search.rs:26-33` (one
  reader said ~35); the eval fixture's "5 non-ASCII questions" are
  typography (U+2013/2014/2022/2212), none Portuguese; the settings seam is
  "a day", not "hours", once the pin, fixture and types twin are counted;
  the eval-gate cost was asserted by three readers and measured by the
  critic (≈ 25 min per 3-round run).
- **Gaps the critic added**: the screenshot path never touches retrieval;
  no single owner for the language value (four readers proposed four);
  Local AI as Tier 2's worst case; history rows carry no language tag
  (`history.rs:33-52`); no pt-BR glossary or product statement; DeepSeek's
  default never rewrites. Closed surfaces, recorded so nobody re-audits
  them: no native dialogs, language-neutral detection,
  `THIRD-PARTY-LICENSES.txt` unaffected unless a dependency is added.

## Conclusion — tiered estimate

"pt-BR support" is four deliverables with independent costs. The critic's
estimate, unchanged:

| Tier | Scope | Effort | Files | Main risk | Doc |
|---|---|---|---|---|---|
| 1 | Answers in pt-BR from the English wikis | hours — half a day to a day incl. two eval rounds + manual smoke | ~6 | language behaviour unmeasurable by the eval (fixed cloud target, English fixture); the canned dead end stays the most common outcome until Tier 2 | [[2026-09-09_ptbr-answer-language]] |
| 2 | Portuguese questions vs the English wikis | a day or two of code + 2–4 days for the pt fixture/judge; ~a week end to end | ~12 | the only translating stage is optional — six no-run conditions, permanent for DeepSeek's default; Fandom's leniency has no contract | [[2026-09-09_ptbr-question-retrieval]] |
| 3 | UI + Rust copy in pt-BR | ~a week — 2–3 d frontend, 2–3 d Rust copy + settings seam, 1 d proof sheet + device smoke | ~45 | two dictionaries that must agree; layout overflow at 420 px has no automated coverage | [[2026-09-09_ptbr-ui-localization]] |
| 4 | Portuguese wikis as sources | weeks + ongoing registry/eval maintenance | ~22 | supply — 1 healthy wiki in 15; stale editions are worse than English; Fandom pt editions can vanish (410) | [[2026-09-09_ptbr-portuguese-wikis]] |

Tiers 1–3 share one `language` setting ([[2026-09-09_one-language-setting]]);
Tier 4 is a per-wiki attribute and stays an idea until Tiers 1–2 show demand.

## Appendix A — Portuguese-question search experiment

Method above; one sample per cell from one residential connection. "pt" = a
Portuguese sentence, "en" = its English twin, "bare" = a single Portuguese
noun through the full ladder (the three full-ladder wikis only). Hit = at
least one title returned by any rung; JW = Jaro-Winkler on the normalized
strings.

| Wiki | Engine | pt hit/tested | en hit/tested | bare hit/tested | Notes |
|---|---|---|---|---|---|
| stardew | default (MySQL) | 0/7 | 7/7 | 3/7 | bare: Krobus, Abigail identical; bomba → Bomb via title index (JW 0.960); fornalha/Furnace 0.721, abacaxi/Pineapple 0.418, ferro/Iron 0.483 |
| minecraft | cirrus | 1/6 | 6/6 | 3/6 | the 1 hit = did-you-mean `code farmer tnt` → 4 irrelevant pages; 6/6 sentences got a mangled suggestion; bare: TNT identical, `Ferro` → Iron (wiki redirect), dragão → `dragon` (Cirrus suggestion) → Ender Dragon |
| corekeeper | unified (Fandom) | 6/6 | 6/6 | 6/6 | 5/6 relevant; `como derrotar o Glurch` → Bosses (miss under strict gold); no `searchinfo` in any response |
| conanexiles | unified (Fandom) | 3/3 | 3/3 | — | `como fazer aço` → Khari Steel …; `como domar um cavalo` → Horse (Variant B), Saddle |
| fallout4 | unified (Fandom) | 2/2 | 2/2 | — | `armadura de poder` → Power armor; `onde encontrar um núcleo de fusão` → Fusion core |
| terraria | cirrus | 0/2 | 2/2 | — | suggestion `coin water uma fornalha` |
| grounded | cirrus | 0/1 | 1/1 | — | suggestion `node encontrar pulgões` |
| warframe | cirrus | 0/1 | 1/1 | — | suggestion `coda conseguir o excalibur`; bare `Excalibur` 3,030 hits |
| poe | cirrus | 0/1 | 1/1 | — | suggestion `pack que serve o orb caos` |
| **non-Fandom** | default + cirrus | **1/18** | **18/18** | 6/13 | |
| **Fandom** | unified | **11/11** | **11/11** | 6/6 | |

Not in the experiment: poe2, abioticfactor, davethediver, skyrim, grounded2
(engine tags from the eval fixture) and gw2 (WAF). On the 19 full-ladder
sentences: 7 hit, 5 relevant, 12 dead ends; 4 sentences skipped the title
index outright via the 4-word cap (`TITLE_INDEX_MAX_WORDS`,
`commands.rs:1445`). Stripped proper-noun + verb follow-ups: `casa Krobus` 1
irrelevant hit (Farmhouse), `conseguir Excalibur` 0, `derrotar dragão` 0,
`Abigail gosta` 0.

## Appendix B — Portuguese-wiki availability

40 GETs over 21 hosts, 2026-09-09, registry order. Edits = mainspace
`recentchanges` rows since 2026-08-10 (`rclimit=500`; ">500" means the
window saturated).

| Game | pt host | Verdict | Articles | Edits 30 d | Notes |
|---|---|---|---|---|---|
| stardew | pt.stardewvalleywiki.com (`/mediawiki/api.php`, pages `/`) | thin, alive | 1,983 | >500 (9 d, 2 users) | lang pt; `activeusers` stat reads 0 (stale on MW 1.35); Abacaxi → 11 langlinks incl. en:Pineapple |
| corekeeper | core-keeper.fandom.com/pt-br | none | — | — | HTTP 410 Gone (closed community); `/pt` 404 |
| conanexiles | conanexiles.fandom.com/pt-br | stub | 318 | 0 | Ferreiro → en:Blacksmith |
| warframe | warframe.fandom.com/pt-br | stale | 2,412 | 1 | top Excalibur pages last edited 2018-07-13 / 2019-02-18; wiki.warframe.com `pt`/`pt-br` interwikis point at itself; 0 langlinks |
| gw2 | — | not probed | — | — | wiki.guildwars2.com WAF-blocks this network |
| poe | — | none | — | — | pathofexile.fandom.com `/pt-br`, `/pt` 404; poewiki.net interwikimap has no language sibling |
| poe2 | — | none | — | — | pathofexile2.fandom.com `/pt-br` 404; poe2wiki.net interwikimap likewise |
| abioticfactor | — | none | — | — | abioticfactor.wiki.gg `/pt/api.php` redirects to the EN root; Fandom `/pt-br` 404 |
| davethediver | — | none | — | — | dave-the-diver.fandom.com and davethediver.fandom.com `/pt-br` 404 |
| skyrim | pt.uesp.net (`/w/api.php`, pages `/wiki/`, ns 134) | stale, partial | 11,411 (all ES games) | 0 (ns 0 and 134) | Portuguese prose, English titles (`Skyrim:Whiterun` exists, `Skyrim:Rio Branco` does not); 0 langlinks; elderscrolls.fandom.com/pt-br 404 |
| fallout4 | fallout.fandom.com/pt-br | stale | 3,176 | 0 | all Fallout games in one namespace; Pip-Boy → 7 langlinks incl. en:Pip-Boy |
| grounded | — | none | — | — | grounded.wiki.gg `/pt/api.php` redirects to the EN root; grounded.fandom.com `/pt-br`, `/pt` 404 |
| grounded2 | — | none | — | — | shares grounded.fandom.com |
| terraria | terraria.wiki.gg/pt (`/pt/api.php`, `/pt/wiki/$1`) | thin, alive | 2,666 | 56 (7 users) | lang pt; Guia → 13 langlinks incl. en:Guide; terraria.fandom.com/pt 1,599 articles / 2 edits — skip |
| minecraft | pt.minecraft.wiki (`/api.php`, pages `/w/`) | healthy | 8,667 | >500 (13 d, 30 contributors) | lang pt-br; 103 active users per siteinfo; Fornalha → 16 langlinks incl. en:Furnace |

Discriminator observed: a nonexistent Fandom language edition returns 404
(410 when closed) with an empty `text/html` body; an existing one returns
siteinfo with `scriptpath` `/pt-br` (or `/pt`). Only the wiki.gg hosts
redirected `/pt` to the English root. Caveat: the Fandom slugs for poe, poe2
and abioticfactor were not verified against a working EN `api.php`, so those
"none" verdicts rest on the official wikis' interwikimaps and the wiki.gg
redirect.

## Status log

- 2026-09-09 — created from the assessment session (six readers + critic
  at `0780619`, 176 live GETs, 0 LLM calls); `commit:` is set by the
  follow-up commit in the same PR.
