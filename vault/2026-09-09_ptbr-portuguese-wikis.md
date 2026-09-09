---
title: Portuguese-language wikis as first-class sources
type: plan
status: idea
created: 2026-09-09
updated: 2026-09-09
tags: [i18n, wiki, rag]
related:
  - "[[2026-09-09_one-language-setting]]"
  - "[[2026-09-09_ptbr-feasibility-assessment]]"
  - "[[2026-09-09_ptbr-question-retrieval]]"
  - "[[2026-07-04_golden-query-retrieval-tests]]"
  - "[[2026-08-04_game-auto-detection]]"
commit:
---

# Portuguese-language wikis as first-class sources

## Context / problem
The fourth pt-BR layer is a supply problem before it is a code problem. The
2026-09-09 probe ([[2026-09-09_ptbr-feasibility-assessment]], 40 GETs over
21 hosts) found, for the 15 built-ins: **1 healthy** — pt.minecraft.wiki
(8,667 articles, 103 active users, >500 mainspace edits in 13 days, EN↔PT
langlinks); **2 thin-alive** — pt.stardewvalleywiki.com (1,983 articles) and
terraria.wiki.gg/pt (2,666); **3 stale** — warframe.fandom.com/pt-br (top
pages last touched 2018-07/2019-02), fallout.fandom.com/pt-br and pt.uesp.net
for Skyrim (0–1 edits in 30 days each); **1 stub** —
conanexiles.fandom.com/pt-br (318 articles); **7 none** — poe, poe2,
corekeeper, grounded, grounded2, davethediver, abioticfactor; **1 unprobed**
— gw2 (WAF). Two facts shape the idea: Fandom pt editions vanish
(core-keeper.fandom.com/pt-br returns 410 Gone), and a live-service game
answered from 2018 Portuguese pages is worse than an English answer from the
current wiki.

Today no Portuguese wiki can even be added cleanly. `normalize_base_url`
(`wiki/probe.rs`) keeps scheme + host and drops any path unless it ends in
`api.php`, so a pasted `warframe.fandom.com/pt-br/wiki/…` probes the root
`api.php` and silently saves the **English** wiki under the Portuguese name
(pt editions carry the prefix in both `api.php` and `articlepath`).
`candidate_domains` probes only `{slug}.wiki.gg` / `{slug}.fandom.com` roots,
so `suggest_wikis` never offers an edition — the pasted URL is the only way
in, and it is the wrong one. `parse_siteinfo` discards `general.lang`, so no
candidate row can name its language. `add_game` mints `id = slugify(name)`
and refuses a built-in collision, so a Portuguese Minecraft cannot keep its
natural name. The `html.rs` reducer is language-neutral, but the
`wikitext.rs` fallback hardcodes four English namespace prefixes
(`render_link_inner`) and 14 canonical `MAGIC_WORDS`, so a degraded pt fetch
leaks `Categoria:` and `__SEMTDC__`.

## Goal / non-goals
- Goal: where a healthy Portuguese wiki exists, the player can pick it and
  gets Portuguese excerpts — the wiki's own client names, no translation;
  pasting any language-prefixed MediaWiki URL saves that edition, not its
  English parent.
- Non-goal: making Portuguese wikis "the" pt-BR story — plans 1–2
  ([[2026-09-09_ptbr-question-retrieval]]) serve every game from its English
  wiki; registering stale or stub editions to fill the table.

## Approach
1. ADR on registry shape (gates 3–6): a second `GameWiki` per game
   (`minecraft-pt`, zero type change, `search_namespace` as the additive-field
   precedent, duplicate tiles in the menu) vs a `lang` field grouping both
   under one tile with a wiki-language preference beside the
   [[2026-09-09_one-language-setting]] value — a per-wiki attribute either way.
2. The probe fix — independently shippable as a `type: fix` doc before
   anything else: a wrong-wiki trap any language-prefixed MediaWiki hits
   today, not a Portuguese feature. `normalize_base_url` keeps a leading `xx`
   / `xx-yy` path segment and `probe_base` tries `{base}/{seg}/api.php` before
   `CANDIDATE_API_PATHS`; `suggest` also probes
   `{slug}.fandom.com/{lang}/api.php`; `parse_siteinfo` keeps `general.lang`
   and `WikiCandidate` shows it on the row; id minting appends `-{lang}` on a
   built-in collision instead of refusing.
3. Register pt.minecraft.wiki and, if the coverage bar is accepted,
   pt.stardewvalleywiki.com and terraria.wiki.gg/pt — `page_url` from each
   wiki's real `articlepath`, one pt golden case each in `GOLDEN_CASES`,
   inserted before the gw2 row: `golden_queries_hit_expected_pages` collects
   misses, but a search *error* aborts it via `.expect(…)` and gw2 sits at
   row 10 of 21, so a case behind it never reports on a WAF-refused run
   ([[2026-07-04_golden-query-retrieval-tests]]) — or make it error-tolerant.
4. `wikitext.rs` fallback: pt namespace prefixes (`arquivo:`, `ficheiro:`,
   `imagem:`, `categoria:`) and pt magic-word aliases, or namespaces read from
   siteinfo; one pt HTML fixture + insta snapshot + `eval/html.test.mjs` row.
5. Detection: a `DetectRule` names one `game_id` (31 rules, language-neutral);
   `match_game`'s result resolves to the preferred-language variant before
   the suggestion chip renders ([[2026-08-04_game-auto-detection]]).
6. Eval: a pt wiki block in `eval/questions.json`, 12–20 Portuguese questions
   per wiki with Portuguese gold titles; `gen-synthetic.mjs` and the judge in
   `run-answer.mjs` parameterised by wiki language.
7. Docs: README "Add your own" paragraph + a Games-table Language column;
   CLAUDE.md §4 recipe; `CONTRIBUTING.md`; `game_request.yml`; smoke rows.
8. Optional langlinks bridge: `prop=langlinks` to fall back to the EN twin
   page when the pt page is missing — five probed hosts expose them
   (pt.minecraft.wiki, pt.stardewvalleywiki.com, terraria.wiki.gg/pt, two
   Fandom pt-br editions); Warframe pt-br and UESP-pt do not.

## Decisions & trade-offs
- **Freshness over language for stale editions**: Warframe, Fallout 4 and
  Skyrim keep their English wikis; a 2018 Portuguese page about a
  live-service game misleads in the player's own language.
- **wiki.gg over Fandom for Terraria**: the existing official-over-Fandom
  curation rule (`candidate_domains`, CLAUDE.md §4); 2,666 articles and 56
  edits in 30 days against Fandom's 1,599 and 2.
- **Not started until plans 1–2 show demand**: they serve all 15 games; this
  serves 1–3 and adds a freshness watch per edition.
- **Registry shape is its own ADR**: duplicate entries are cheap now and ugly
  in the menu; a `lang` field is the right model but touches the picker,
  history and the stored `wikis.json`.

## Status log
- 2026-09-09 — created as an idea from the assessment; item 2 ships on its own.
