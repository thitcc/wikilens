// Retrieval eval runner — N rounds over the fixture, per-leg counterfactuals.
//
//   node eval/run-retrieval.mjs --out eval/out/<run-name> [--rounds 3] [--only S1,M2]
//        [--game stardew] [--source hand] [--no-rewrite] [--pace 300]
//
// Per question (mirrors commands.rs run_ask up to the fetch phase):
//   preprocess → join(raw search ‖ LLM rewrite) → ≤2 candidate searches →
//   merge_hits → zero-hit ladder (suggestion, simplify, title index — the
//   latter fetched once per wiki per run) — then the eval judges three
//   pipelines for the price of one:
//   raw-only (gold in raw top-4), rewrite-only (gold in any candidate hit),
//   and the real merge (gold rank). Titles are redirect-resolved before
//   judging. Sequential per question, paced (MediaWiki etiquette); raw search
//   and rewrite run concurrently (different hosts, like production).
//
// Output: <out>/meta.json, gold.json, round-<n>.jsonl (durable mid-run).
// Transient raw-search errors retry once at end of round; still-failing
// questions stay `raw_error` so network noise is separable from misses.

import { mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import {
  preprocessQuery, simplifyQuery, searchFull, mergeHits, rewriteQuery, resolveTitles,
  loadDefaultTarget, loadFixture, sleep, appendJsonl, ciEq, SEARCH_LIMIT, REWRITE_SEARCH_LIMIT,
} from './lib.mjs';

import { TitleIndex, fetchAllTitles } from './titles.mjs';

const TITLE_INDEX_MAX_WORDS = 4; // commands.rs TITLE_INDEX_MAX_WORDS
const TITLE_INDEX_ON = !['0', 'false', 'off', 'no'].includes(String(process.env.WIKILENS_TITLE_INDEX ?? '1').trim().toLowerCase());
const titleIndexCache = new Map(); // wiki id -> TitleIndex | null (failed)
async function getTitleIndex(wiki) {
  if (titleIndexCache.has(wiki.id)) return titleIndexCache.get(wiki.id);
  let index = null;
  try { index = new TitleIndex(await fetchAllTitles(wiki, 150), wiki.search_namespace != null); } catch { index = null; }
  titleIndexCache.set(wiki.id, index);
  return index;
}

const args = parseArgs(process.argv.slice(2));
if (!args.out) { console.error('usage: --out <dir> [--rounds N] [--only ids] [--game id] [--source s] [--no-rewrite] [--pace ms]'); process.exit(2); }
const ROUNDS = Number(args.rounds ?? 1);
const PACE = Number(args.pace ?? 300);
const OUT = args.out;
mkdirSync(OUT, { recursive: true });

const fixture = loadFixture();
const WIKIS = fixture.wikis;
let questions = fixture.questions;
if (args.only) { const ids = new Set(String(args.only).split(',')); questions = questions.filter((q) => ids.has(q.id)); }
if (args.game) questions = questions.filter((q) => q.game === args.game);
if (args.source) questions = questions.filter((q) => q.source === args.source);
for (const q of questions) if (!WIKIS[q.game]) throw new Error(`${q.id}: unknown wiki ${q.game}`);

const target = args['no-rewrite'] ? null : loadDefaultTarget().rewrite;
const meta = {
  startedAt: new Date().toISOString(), rounds: ROUNDS, pace: PACE,
  filters: { only: args.only ?? null, game: args.game ?? null, source: args.source ?? null },
  rewrite: target ? { kind: target.kind, model: target.model, skipReasoning: target.skipReasoning } : 'disabled',
  questionCount: questions.length, wikiCount: new Set(questions.map((q) => q.game)).size,
  fixtureVersion: fixture.version,
};
writeFileSync(join(OUT, 'meta.json'), JSON.stringify(meta, null, 2));
console.log(`run: ${questions.length} questions × ${ROUNDS} rounds; rewrite=${target ? `${target.kind}/${target.model}` : 'disabled'}; out=${OUT}`);

// ── Gold pre-pass: resolve every gold title once per wiki (batches of 50) ─
async function goldPrePass() {
  const perWiki = new Map();
  for (const q of questions) {
    if (!perWiki.has(q.game)) perWiki.set(q.game, new Set());
    for (const g of q.gold) perWiki.get(q.game).add(g);
  }
  const valid = {};
  for (const [game, titles] of perWiki) {
    const wiki = WIKIS[game];
    valid[game] = {};
    const list = [...titles];
    for (let i = 0; i < list.length; i += 50) {
      const batch = list.slice(i, i + 50);
      let res;
      try { res = await resolveTitles(wiki, batch); } catch (e) {
        await sleep(2000);
        res = await resolveTitles(wiki, batch); // one retry, then let it throw
      }
      for (const t of batch) {
        const resolved = res.map.get(t) ?? t;
        valid[game][t] = res.missing.has(resolved) ? null : resolved;
      }
      await sleep(PACE);
    }
  }
  writeFileSync(join(OUT, 'gold.json'), JSON.stringify(valid, null, 2));
  return valid;
}

// ── One question, one round ───────────────────────────────────────────
async function runQuestion(q, goldResolved, round) {
  const wiki = WIKIS[q.game];
  const rec = {
    id: q.id, game: q.game, engine: wiki.engine, style: q.style, source: q.source,
    round, question: q.question, gold: q.gold, goldResolved,
  };

  // commands.rs — the wiki search gets the preprocessed query; the rewrite
  // receives the original question.
  const query = preprocessQuery(q.question);
  rec.query = query;
  rec.queryWords = query.split(/\s+/u).filter(Boolean).length;

  const [raw, rw] = await Promise.all([
    searchFull(wiki, query),
    !target
      ? Promise.resolve({ queries: [], ms: null, error: 'disabled' })
      : target.skipReasoning
        ? Promise.resolve({ queries: [], ms: null, error: 'skipped (reasoning model)' })
        : rewriteQuery(target, wiki.name, q.question),
  ]);
  rec.rawMs = raw.ms; rec.rawError = raw.error; rec.rawTitles = raw.titles; rec.suggestion = raw.suggestion;
  rec.rewriteMs = rw.ms; rec.rewriteError = rw.error; rec.rewriteCandidates = rw.queries;

  if (raw.error) { rec.status = 'raw_error'; return rec; } // a raw-search error kills the ask

  // commands.rs — drop candidates echoing the preprocessed query (ci), take ≤2.
  const toSearch = rw.queries.filter((rq) => !ciEq(rq, query)).slice(0, REWRITE_SEARCH_LIMIT);
  rec.candidatesSearched = toSearch;
  const rewriteHits = [];
  let candMs = 0;
  const candErrors = [];
  if (toSearch.length > 0) {
    const t0 = Date.now();
    const results = await Promise.all(toSearch.map((rq) => searchFull(wiki, rq)));
    candMs = Date.now() - t0;
    for (const r of results) { if (r.error) candErrors.push(r.error); rewriteHits.push(...(r.titles ?? [])); }
  }
  rec.candMs = candMs; rec.candErrors = candErrors; rec.rewriteHits = rewriteHits;

  let merged = mergeHits(raw.titles, rewriteHits, SEARCH_LIMIT);
  rec.mergedBeforeLadder = merged;

  // Zero-hit ladder (commands.rs): suggestion → simplify → title index.
  const ladder = [];
  if (merged.length === 0) {
    const sugg = raw.suggestion ?? '';
    if (sugg !== '' && sugg !== query) {
      await sleep(PACE);
      const r = await searchFull(wiki, sugg);
      ladder.push({ stage: 'suggestion', query: sugg, titles: r.titles, error: r.error, ms: r.ms });
      if (r.error) { rec.ladder = ladder; rec.status = 'raw_error'; rec.rawError = `ladder:${r.error}`; return rec; }
      merged = r.titles;
    }
  }
  if (merged.length === 0) {
    const simplified = simplifyQuery(q.question);
    if (simplified !== '' && simplified !== query) {
      await sleep(PACE);
      const r = await searchFull(wiki, simplified);
      ladder.push({ stage: 'simplify', query: simplified, titles: r.titles, error: r.error, ms: r.ms });
      if (r.error) { rec.ladder = ladder; rec.status = 'raw_error'; rec.rawError = `ladder:${r.error}`; return rec; }
      merged = r.titles;
    }
  }
  // commands.rs — the title index fires only while still empty, only for ≤4-word
  // queries, and only when WIKILENS_TITLE_INDEX isn't "0". Fetched once per
  // wiki per run (production: once per session).
  if (merged.length === 0 && TITLE_INDEX_ON && rec.queryWords <= TITLE_INDEX_MAX_WORDS) {
    const t0 = Date.now();
    const index = await getTitleIndex(wiki);
    if (!index) {
      ladder.push({ stage: 'title-index', query, titles: null, note: 'index unavailable', ms: Date.now() - t0 });
    } else {
      const matched = index.bestMatch(query);
      ladder.push({ stage: 'title-index', query, titles: matched ? [matched] : [], note: matched ? null : '(no match)', ms: Date.now() - t0, indexSize: index.titles.length });
      if (matched) merged = [matched];
    }
  }
  rec.ladder = ladder;
  rec.merged = merged;

  // Redirect-normalize everything we judge (one GET over the union).
  const union = [...new Set([...raw.titles, ...rewriteHits, ...merged])];
  let map = new Map();
  await sleep(PACE);
  try { ({ map } = await resolveTitles(wiki, union)); } catch { /* judge on unresolved titles */ }
  const res = (t) => map.get(t) ?? t;
  rec.mergedResolved = merged.map(res);

  const inList = (list) => {
    for (const g of goldResolved) {
      const idx = list.findIndex((t) => ciEq(t, g));
      if (idx !== -1) return idx;
    }
    return -1;
  };
  rec.goldRankInRaw = inList(raw.titles.map(res));
  rec.goldInRawTop4 = rec.goldRankInRaw !== -1;
  rec.goldInRewriteHits = inList(rewriteHits.map(res)) !== -1;
  const rank = inList(rec.mergedResolved);
  rec.goldRankInMerge = rank;
  rec.status = merged.length === 0 ? 'zero_hit' : rank !== -1 ? 'hit' : 'wrong_nonzero';
  return rec;
}

// ── Main ──────────────────────────────────────────────────────────────
async function main() {
  const goldValid = await goldPrePass();
  const invalid = questions.filter((q) => !q.gold.some((g) => goldValid[q.game]?.[g]));
  console.log(`gold pre-pass: ${questions.length - invalid.length} valid, ${invalid.length} invalid${invalid.length ? ` (${invalid.map((q) => q.id).join(', ')})` : ''}`);

  for (let round = 1; round <= ROUNDS; round++) {
    const file = join(OUT, `round-${round}.jsonl`);
    if (existsSync(file)) writeFileSync(file, '');
    const tally = { hit: 0, wrong_nonzero: 0, zero_hit: 0, raw_error: 0, gold_invalid: 0 };
    const retry = [];
    const t0 = Date.now();
    const pass = async (q, retried) => {
      const goldResolved = q.gold.map((g) => goldValid[q.game]?.[g]).filter((g) => g);
      let rec;
      if (goldResolved.length === 0) {
        rec = { id: q.id, game: q.game, style: q.style, source: q.source, round, question: q.question, gold: q.gold, goldResolved, status: 'gold_invalid' };
      } else {
        rec = await runQuestion(q, goldResolved, round);
        rec.retried = retried;
      }
      if (rec.status === 'raw_error' && !retried) { retry.push(q); return; }
      appendJsonl(file, rec);
      tally[rec.status] = (tally[rec.status] ?? 0) + 1;
      console.log(`r${round} ${q.id.padEnd(5)} ${rec.status.padEnd(13)} rank=${rec.goldRankInMerge ?? '-'} raw=${rec.goldInRawTop4 ?? '-'} rw=${rec.goldInRewriteHits ?? '-'} rwErr=${rec.rewriteError ?? '-'} (${rec.rawMs ?? '-'}/${rec.rewriteMs ?? '-'}/${rec.candMs ?? '-'}ms)${retried ? ' [retry]' : ''}`);
    };
    for (const q of questions) { await pass(q, false); await sleep(PACE); }
    if (retry.length > 0) {
      console.log(`r${round}: retrying ${retry.length} raw_error question(s) after backoff`);
      await sleep(3000);
      for (const q of retry) { await pass(q, true); await sleep(PACE * 2); }
    }
    console.log(`ROUND ${round} DONE in ${Math.round((Date.now() - t0) / 1000)}s: ${JSON.stringify(tally)}`);
    if (round < ROUNDS) await sleep(5000);
  }
  console.log('RUN COMPLETE');
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) continue;
    const key = a.slice(2);
    const next = argv[i + 1];
    if (next === undefined || next.startsWith('--')) out[key] = true;
    else { out[key] = next; i++; }
  }
  return out;
}

main().catch((e) => { console.error('FATAL', e); process.exit(1); });
