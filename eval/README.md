# Retrieval eval suite

A re-runnable measurement of the ask pipeline's retrieval phase (and,
optionally, its answers) over a fixed question set. It answers two questions
the golden suite can't: *how often does the right page reach the model?* and
*how much of that is the LLM rewrite vs. the raw search?* — with per-wiki,
per-style and per-source breakdowns, and cross-round stability.

Born from the core-pipeline analysis' Tier 0
(`vault/research/2026-08-17_core-pipeline-analysis.md`); the plan and the
round-2 results live in `vault/2026-08-22_retrieval-eval-suite.md` and
`vault/research/2026-08-22_retrieval-eval-round-2.md`.

## What's here

| File | Role |
| --- | --- |
| `questions.json` | The fixture: wikis (endpoint, engine family, `builtin` flag) + questions (id, game, style, source, gold titles, optional `trunc` / `fact`). Pinned against the Rust game registry by `config_guardrails.rs`. |
| `lib.mjs`, `html.mjs`, `titles.mjs` | Node mirrors of the Rust pipeline: `preprocess_query`, `simplify_query`, `build_search_params`, `merge_hits`, `parse_rewrite_queries`, the rewrite request, `fetch_rendered_page` + `truncate_text`, `build_user_message` + the answer request, `html::to_plaintext`, the title index (`allpages` walk + Jaro-Winkler `best_match`). |
| `lib.test.mjs`, `html.test.mjs`, `titles.test.mjs` | Parity tests against the crate's own test vectors and insta snapshots — run by `npm run test:node` (CI). A mirror that drifts fails the build. |
| `run-retrieval.mjs` | The runner: N rounds, per-leg counterfactuals, zero-hit ladder, JSONL per round. |
| `run-answer.mjs` | Answer-level eval on a retrieval run: fetch the merged pages like `fetch.rs`, call the eval answer model with the production prompt, check the expected fact against the context actually sent, judge the answer. |
| `aggregate.mjs` | Summaries: overall / per leg / per wiki / per engine / per style / per source / stability / skip-gate counterfactual / timings → `summary.json` + console digest. |
| `merge-policies.mjs` | Replays a run's recorded searches (raw + per-candidate rewrite hits) through alternative merge rules and scores each against the same gold — a fair what-if for merge-rule changes. `production` is the shipped gc-rr rule; `old-production` keeps the pre-2026-08-23 reserved-slot rule as the legacy baseline, and `gc-rr` must always show +0 −0 vs production (identity tripwire). |
| `gen-synthetic.mjs` | Drafts synthetic candidates from sampled page content (curator reviews, then `fixture-merge.mjs`). |
| `fixture-merge.mjs` | Merges candidate files into the fixture and keeps its one-line-per-question layout. |
| `check-facts.mjs` | Verifies every `fact.evidence` is a literal substring of its gold page (and reports whether it sits beyond the 8000-char cap). |

Run output goes to `eval/out/` (gitignored). Summaries and conclusions go in
the vault, never here.

## Running

```
npm run eval:retrieval -- --out eval/out/2026-08-22 --rounds 3
npm run eval:answer    -- --run eval/out/2026-08-22        # questions with `fact`
npm run eval:aggregate -- --run eval/out/2026-08-22
```

Then `node eval/merge-policies.mjs --run eval/out/2026-08-22` for the merge what-ifs.

Flags: `--only S1,M2`, `--game stardew`, `--source hand|history|synthetic`,
`--no-rewrite` (raw-only pipeline), `--pace 300` (ms between requests).

The LLM legs (rewrite, answer, judge, synthetic drafting) use the **eval-owned
target from the repo `.env`** — the `WIKILENS_EVAL_*` family (`_API_PROVIDER`
= `anthropic` | `openai`, `_API_URL`, `_API_KEY`, `_ANSWER_MODEL`, optional
`_REWRITE_MODEL`; commented template in `.env.example`). Eval-only by design:
the app's modes read nothing from env anymore, and a stable cloud target keeps
measurements reproducible instead of silently following the app's Settings.
The key is used for the request header only; no record, log, or summary ever
contains it. Cost is small (Haiku-class: ~200 questions × 3 rounds ≈ a few
tenths of a dollar; the answer subset ≈ the same again).

Etiquette: sequential per question, ≥300 ms between requests, the raw search
and the rewrite concurrently (different hosts, like production), ≤2 candidate
searches concurrently (like production). Transient raw-search errors retry
once at the end of the round; what still fails stays `raw_error` so network
noise never counts as a miss.

## What a record means

Per question and round: `status` ∈ `hit` (a gold title is in the merged top-4,
after redirect resolution) | `wrong_nonzero` (titles, none gold) | `zero_hit`
| `raw_error` | `gold_invalid`. Plus the counterfactual legs —
`goldInRawTop4` (raw-only pipeline), `goldInRewriteHits` (rewrite-only) — and
`goldRankInMerge`. "Rescued by rewrite" = hit ∧ ¬raw; "saved by raw" =
hit ∧ ¬rewrite; "evicted" = a leg had the gold but the merge dropped it (R7).

Answer records: `factInContext` (the evidence string occurs in the truncated
context sent), `factBeyondCap` (in a fetched page's full text but past the
8000-char cut — the R10 class), `judge.label` ∈ correct | partial | wrong |
abstain, `answer.stopReason` (`max_tokens` = cut answer); the aggregator adds
`declaredGap` — the answer text itself says the excerpts lack the fact (a
programmatic cross-check on the judge's wrong/abstain split).

## Mirror fidelity notes

- The **title index** (`titles.rs`, last ladder rung) is mirrored in
  `titles.mjs` and fetched once per wiki per run (production: once per
  session); its allpages walk is paced at 150 ms here.
- The **wikitext fallback** (`fetch_pages_wikitext`) is not mirrored — a failed
  `action=parse` marks the answer record `degraded` and skips the page.
- The answer call is non-streaming (same model, prompt, cap and content; only
  transport differs), so per-token timings aren't measured.
- Hit@4 is retrieval only: the right page in the context does not mean the
  right fact survived truncation or reached the answer — that is what the
  answer subset measures, on a smaller set.

## Growing the set

Keep ids stable (the stability analysis joins on them). Styles and sources
are closed vocabularies (pinned by the Rust test). Every gold title must
resolve live (`run-retrieval.mjs --rounds 0` runs only the gold pre-pass);
every `fact.evidence` must pass `check-facts.mjs`. Synthetic questions stay
flagged `synthetic` — they inflate hit rates (the generator knows the page),
so reports quarantine them; the `history` + `hand` buckets are the headline.
Shared wikis need care: Grounded 2 pages carry the `(Grounded 2)` suffix on
Grounded's Fandom wiki, UESP Skyrim lives in namespace 134 (`Skyrim:` titles),
Nukapedia mixes every Fallout game.
