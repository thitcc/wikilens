// Aggregate a retrieval run (and, if present, an answer run) into summary.json
// plus a console digest.
//
//   node eval/aggregate.mjs --run eval/out/<run-name>
//
// Dimensions: overall, per leg (raw-only / rewrite-only / merge), rescues,
// ranks, ladder fires, per source / style / game / engine, Tier-1 skip-gate
// counterfactual (≤2-word queries that only the rewrite saved), cross-round
// stability (always-hit / always-miss / flaky, rewrite-candidate variance),
// timings (median / p90 per phase).

import { readdirSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { readJsonl, loadFixture } from './lib.mjs';

const args = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => (a.startsWith('--') ? [a.slice(2), arr[i + 1] ?? true] : null)).filter(Boolean));
if (!args.run) { console.error('usage: --run <dir>'); process.exit(2); }
const RUN = args.run;
const fixture = loadFixture();

const roundFiles = readdirSync(RUN).filter((f) => /^round-\d+\.jsonl$/.test(f)).sort((a, b) => num(a) - num(b));
if (roundFiles.length === 0) { console.error('no round-*.jsonl in', RUN); process.exit(1); }
const rounds = roundFiles.map((f) => ({ n: num(f), recs: readJsonl(join(RUN, f)) }));
function num(f) { return Number(f.match(/\d+/)[0]); }

const pct = (a, b) => (b === 0 ? null : Math.round((1000 * a) / b) / 10);
const median = (xs) => { const s = xs.filter((x) => typeof x === 'number').sort((a, b) => a - b); return s.length ? s[Math.floor(s.length / 2)] : null; };
const p90 = (xs) => { const s = xs.filter((x) => typeof x === 'number').sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * 0.9))] : null; };

// ── Per-round core stats ───────────────────────────────────────────────
function roundStats(recs) {
  const judged = recs.filter((r) => ['hit', 'wrong_nonzero', 'zero_hit'].includes(r.status));
  const n = judged.length;
  const hit = judged.filter((r) => r.status === 'hit');
  const rawOnly = judged.filter((r) => r.goldInRawTop4);
  const rewriteOnly = judged.filter((r) => r.goldInRewriteHits);
  const rewriteRan = judged.filter((r) => !r.rewriteError);
  return {
    total: recs.length, judged: n,
    hit: hit.length, wrong_nonzero: judged.filter((r) => r.status === 'wrong_nonzero').length,
    zero_hit: judged.filter((r) => r.status === 'zero_hit').length,
    raw_error: recs.filter((r) => r.status === 'raw_error').length,
    gold_invalid: recs.filter((r) => r.status === 'gold_invalid').length,
    hitRate: pct(hit.length, n),
    legs: {
      rawOnly: { hits: rawOnly.length, rate: pct(rawOnly.length, n) },
      rewriteOnly: { hits: rewriteOnly.length, rate: pct(rewriteOnly.length, n) },
      merge: { hits: hit.length, rate: pct(hit.length, n) },
    },
    rescuedByRewrite: judged.filter((r) => r.status === 'hit' && !r.goldInRawTop4).length,
    savedByRaw: judged.filter((r) => r.status === 'hit' && !r.goldInRewriteHits).length,
    bothLegs: judged.filter((r) => r.status === 'hit' && r.goldInRawTop4 && r.goldInRewriteHits).length,
    evictedByMerge: judged.filter((r) => r.status !== 'hit' && (r.goldInRawTop4 || r.goldInRewriteHits)).length,
    rewriteErrors: judged.filter((r) => r.rewriteError).length,
    rewriteRan: rewriteRan.length,
    rankHistogram: [0, 1, 2, 3].map((k) => hit.filter((r) => r.goldRankInMerge === k).length),
    top2: hit.filter((r) => r.goldRankInMerge <= 1).length,
    ladderFires: judged.filter((r) => (r.ladder ?? []).some((l) => l.stage !== 'title-index')).length,
    ladderRescues: judged.filter((r) => r.status === 'hit' && (r.mergedBeforeLadder ?? []).length === 0).length,
    candidatesSearchedMean: Math.round((10 * judged.reduce((s, r) => s + (r.candidatesSearched?.length ?? 0), 0)) / Math.max(1, n)) / 10,
    timings: {
      rawMs: { median: median(judged.map((r) => r.rawMs)), p90: p90(judged.map((r) => r.rawMs)) },
      rewriteMs: { median: median(rewriteRan.map((r) => r.rewriteMs)), p90: p90(rewriteRan.map((r) => r.rewriteMs)) },
      candMs: { median: median(judged.filter((r) => r.candMs > 0).map((r) => r.candMs)), p90: p90(judged.filter((r) => r.candMs > 0).map((r) => r.candMs)) },
      joinMs: { median: median(judged.map((r) => Math.max(r.rawMs ?? 0, r.rewriteMs ?? 0))) },
    },
    skipGate: skipGate(judged),
  };
}

// Tier-1 #7 counterfactual: skip the rewrite when the preprocessed query has
// ≤ N words. "wouldLose" = hits that only the rewrite leg produced.
function skipGate(judged) {
  const out = {};
  for (const maxWords of [1, 2, 3]) {
    const gated = judged.filter((r) => r.queryWords <= maxWords);
    const wouldLose = gated.filter((r) => r.status === 'hit' && !r.goldInRawTop4);
    out[`le${maxWords}`] = { gated: gated.length, wouldLose: wouldLose.length, wouldLoseIds: wouldLose.map((r) => r.id), savedMs: median(gated.map((r) => Math.max(0, (r.rewriteMs ?? 0) - (r.rawMs ?? 0)) + (r.candMs ?? 0))) };
  }
  return out;
}

// ── Grouped rates, averaged over rounds ────────────────────────────────
function grouped(keyFn) {
  const acc = new Map();
  for (const { recs } of rounds) {
    for (const r of recs) {
      if (!['hit', 'wrong_nonzero', 'zero_hit'].includes(r.status)) continue;
      const k = keyFn(r);
      if (!acc.has(k)) acc.set(k, { n: 0, hit: 0, raw: 0, rw: 0, ids: new Set() });
      const a = acc.get(k);
      a.n++; a.ids.add(r.id);
      if (r.status === 'hit') a.hit++;
      if (r.goldInRawTop4) a.raw++;
      if (r.goldInRewriteHits) a.rw++;
    }
  }
  const out = {};
  for (const [k, a] of [...acc.entries()].sort((x, y) => y[1].n - x[1].n)) {
    out[k] = { questions: a.ids.size, judged: a.n, hit: a.hit, hitRate: pct(a.hit, a.n), rawOnlyRate: pct(a.raw, a.n), rewriteOnlyRate: pct(a.rw, a.n) };
  }
  return out;
}

// ── Stability across rounds ────────────────────────────────────────────
function stability() {
  if (rounds.length < 2) return null;
  const byId = new Map();
  for (const { n, recs } of rounds) for (const r of recs) {
    if (!byId.has(r.id)) byId.set(r.id, []);
    byId.get(r.id).push({ round: n, status: r.status, cands: JSON.stringify(r.rewriteCandidates ?? []), merged: JSON.stringify(r.mergedResolved ?? []) });
  }
  let always = 0; let never = 0; let flaky = 0; const flakyIds = []; let candVar = 0; let mergeVar = 0; let judgedAll = 0;
  for (const [id, v] of byId) {
    const judged = v.filter((x) => ['hit', 'wrong_nonzero', 'zero_hit'].includes(x.status));
    if (judged.length < 2) continue;
    judgedAll++;
    const hits = judged.filter((x) => x.status === 'hit').length;
    if (hits === judged.length) always++;
    else if (hits === 0) never++;
    else { flaky++; flakyIds.push(`${id}(${hits}/${judged.length})`); }
    if (new Set(judged.map((x) => x.cands)).size > 1) candVar++;
    if (new Set(judged.map((x) => x.merged)).size > 1) mergeVar++;
  }
  return { questionsInAllRounds: judgedAll, alwaysHit: always, neverHit: never, flaky, flakyIds, rewriteCandidatesVaried: candVar, mergedSetVaried: mergeVar };
}

const perRound = rounds.map(({ n, recs }) => ({ round: n, ...roundStats(recs) }));
const rates = perRound.map((r) => r.hitRate).filter((x) => x !== null);
const summary = {
  run: RUN,
  rounds: rounds.length,
  questions: new Set(rounds.flatMap((r) => r.recs.map((x) => x.id))).size,
  hitRate: { mean: Math.round((10 * rates.reduce((a, b) => a + b, 0)) / Math.max(1, rates.length)) / 10, min: Math.min(...rates), max: Math.max(...rates) },
  perRound,
  bySource: grouped((r) => r.source),
  byStyle: grouped((r) => r.style),
  byGame: grouped((r) => r.game),
  byEngine: grouped((r) => r.engine ?? fixture.wikis[r.game]?.engine ?? '?'),
  stability: stability(),
  misses: rounds.flatMap(({ n, recs }) => recs.filter((r) => ['wrong_nonzero', 'zero_hit'].includes(r.status)).map((r) => ({ round: n, id: r.id, game: r.game, status: r.status, query: r.query, rewriteCandidates: r.rewriteCandidates, rawTitles: r.rawTitles, merged: r.mergedResolved, goldInRawTop4: r.goldInRawTop4, goldInRewriteHits: r.goldInRewriteHits }))),
};

// Answer run (optional)
const answersFile = join(RUN, 'answers.jsonl');
if (existsSync(answersFile)) {
  const a = readJsonl(answersFile);
  const judged = a.filter((r) => r.judge?.label);
  const byLabel = {};
  for (const r of judged) byLabel[r.judge.label] = (byLabel[r.judge.label] ?? 0) + 1;
  summary.answers = {
    total: a.length, judged: judged.length, byLabel,
    grounded: judged.filter((r) => r.factInContext === true).length,
    correctUngrounded: judged.filter((r) => r.judge.label === 'correct' && r.factInContext === false).length,
    correctGrounded: judged.filter((r) => r.judge.label === 'correct' && r.factInContext === true).length,
    wrongWithFactInContext: judged.filter((r) => r.judge.label === 'wrong' && r.factInContext === true).length,
    abstainWithFactInContext: judged.filter((r) => r.judge.label === 'abstain' && r.factInContext === true).length,
    factBeyondCap: judged.filter((r) => r.factBeyondCap === true).length,
    stopMaxTokens: a.filter((r) => ['max_tokens', 'length'].includes(r.answer?.stopReason)).length,
    degradedFetch: a.filter((r) => r.degraded).length,
    timings: { fetchMs: { median: median(a.map((r) => r.fetchMs)), p90: p90(a.map((r) => r.fetchMs)) }, answerMs: { median: median(a.map((r) => r.answer?.ms)), p90: p90(a.map((r) => r.answer?.ms)) } },
    contextChars: { median: median(a.map((r) => r.contextChars)), p90: p90(a.map((r) => r.contextChars)) },
    outputTokens: { median: median(a.map((r) => r.answer?.usage?.output_tokens ?? r.answer?.usage?.completion_tokens)) },
    rows: a.map((r) => ({ id: r.id, game: r.game, retrieval: r.retrievalStatus, label: r.judge?.label ?? null, factInContext: r.factInContext, factBeyondCap: r.factBeyondCap, stopReason: r.answer?.stopReason, judgeNote: r.judge?.note ?? null })),
  };
}

writeFileSync(join(RUN, 'summary.json'), JSON.stringify(summary, null, 2));

// ── Console digest ─────────────────────────────────────────────────────
console.log(`run ${RUN}: ${summary.questions} questions × ${summary.rounds} rounds`);
for (const r of perRound) {
  console.log(`  round ${r.round}: hit ${r.hit}/${r.judged} (${r.hitRate}%) | raw-only ${r.legs.rawOnly.rate}% | rewrite-only ${r.legs.rewriteOnly.rate}% | rescued ${r.rescuedByRewrite} savedByRaw ${r.savedByRaw} evicted ${r.evictedByMerge} | zero ${r.zero_hit} wrong ${r.wrong_nonzero} rawErr ${r.raw_error} rwErr ${r.rewriteErrors} | ladder ${r.ladderFires} | med raw/rw/cand ${r.timings.rawMs.median}/${r.timings.rewriteMs.median}/${r.timings.candMs.median}ms`);
}
console.log(`  mean hit ${summary.hitRate.mean}% (${summary.hitRate.min}–${summary.hitRate.max})`);
if (summary.stability) console.log(`  stability: always ${summary.stability.alwaysHit} never ${summary.stability.neverHit} flaky ${summary.stability.flaky} [${summary.stability.flakyIds.join(' ')}] candVar ${summary.stability.rewriteCandidatesVaried} mergeVar ${summary.stability.mergedSetVaried}`);
for (const [name, g] of [['source', summary.bySource], ['style', summary.byStyle], ['engine', summary.byEngine], ['game', summary.byGame]]) {
  console.log(`  by ${name}: ${Object.entries(g).map(([k, v]) => `${k} ${v.hitRate}% (${v.hit}/${v.judged}; raw ${v.rawOnlyRate}% rw ${v.rewriteOnlyRate}%)`).join(' | ')}`);
}
const sg = perRound[0].skipGate;
console.log(`  skip-gate (round 1): ≤1w gated ${sg.le1.gated} lose ${sg.le1.wouldLose} | ≤2w gated ${sg.le2.gated} lose ${sg.le2.wouldLose} [${sg.le2.wouldLoseIds.join(',')}] | ≤3w gated ${sg.le3.gated} lose ${sg.le3.wouldLose}`);
if (summary.answers) {
  const A = summary.answers;
  console.log(`  answers: ${A.judged} judged ${JSON.stringify(A.byLabel)} | correct&grounded ${A.correctGrounded} correct&ungrounded ${A.correctUngrounded} wrong-with-fact-in-context ${A.wrongWithFactInContext} abstain-with-fact ${A.abstainWithFactInContext} | fact beyond cap ${A.factBeyondCap} | max_tokens ${A.stopMaxTokens} | degraded ${A.degradedFetch}`);
}
console.log(`summary → ${join(RUN, 'summary.json')}`);
