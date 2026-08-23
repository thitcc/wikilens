// Merge-policy what-ifs — replay a retrieval run's recorded searches (raw hits,
// per-candidate rewrite hits) through alternative merge rules and score each
// one against the same gold. Same searches, same rewrite, same wiki answers:
// only the combination rule changes, so the comparison is fair even though
// the rewrite itself is non-deterministic.
//
//   node eval/merge-policies.mjs --run eval/out/<run-name> [--round 1]
//
// Policies (all cap at 4 titles, case-insensitive dedupe):
//   production   commands.rs merge_hits — consensus, then rewrite hits (cap 3
//                while raw[0] is unplaced), then raw
//   raw-only     the raw search alone
//   rewrite-only the candidate hits alone, in candidate order
//   no-consensus production without the consensus step
//   entity-first cand1[0] (the bare-entity candidate's top hit) gets a reserved
//                slot, then production order
//   interleave   round-robin raw[0], cand1[0], cand2[0], raw[1], cand1[1], …
//   raw-first    raw[0] then round-robin over (cand1, cand2, raw[1..])
//   entity+raw-first  cand1[0] and raw[0] reserved, then round-robin over the rest
//   consensus-guard   production, but consensus counts only agreement with cand1
// Records need `rewriteHitsByCandidate` (runs after 2026-08-22); older records
// are split heuristically (one candidate → all hits; 8 hits → 4+4) and the
// ambiguous remainder is reported, not guessed.

import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { readJsonl, mergeHits, ciEq, SEARCH_LIMIT } from './lib.mjs';

const args = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => (a.startsWith('--') ? [a.slice(2), arr[i + 1] ?? true] : null)).filter(Boolean));
if (!args.run) { console.error('usage: --run <dir> [--round N]'); process.exit(2); }
const files = readdirSync(args.run).filter((f) => /^round-\d+\.jsonl$/.test(f)).sort();
const recs = (args.round ? [`round-${args.round}.jsonl`] : files).flatMap((f) => readJsonl(join(args.run, f)));

function pushUnique(out, t, cap) { if (out.length < cap && !out.some((e) => ciEq(e, t))) out.push(t); }
function dedupe(list, cap = SEARCH_LIMIT) { const out = []; for (const t of list) pushUnique(out, t, cap); return out; }
function roundRobin(lists, cap = SEARCH_LIMIT) {
  const out = []; const max = Math.max(0, ...lists.map((l) => l.length));
  for (let i = 0; i < max && out.length < cap; i++) for (const l of lists) if (i < l.length) pushUnique(out, l[i], cap);
  return out;
}
const POLICIES = {
  production: ({ raw, cands }) => mergeHits(raw, cands.flat(), SEARCH_LIMIT),
  'raw-only': ({ raw }) => dedupe(raw),
  'rewrite-only': ({ cands }) => dedupe(cands.flat()),
  'no-consensus': ({ raw, cands }) => {
    const out = []; const rewrite = cands.flat();
    const reserve = raw.length > 0 && !rewrite.some((t) => ciEq(t, raw[0]));
    for (const t of rewrite) pushUnique(out, t, reserve ? SEARCH_LIMIT - 1 : SEARCH_LIMIT);
    for (const t of raw) pushUnique(out, t, SEARCH_LIMIT);
    return out;
  },
  'entity-first': ({ raw, cands }) => {
    const out = []; if (cands[0]?.[0]) pushUnique(out, cands[0][0], SEARCH_LIMIT);
    for (const t of mergeHits(raw, cands.flat(), SEARCH_LIMIT)) pushUnique(out, t, SEARCH_LIMIT);
    return out;
  },
  interleave: ({ raw, cands }) => roundRobin([raw, ...cands]),
  // cand1[0] and raw[0] both reserved, then round-robin over the remainders.
  'entity+raw-first': ({ raw, cands }) => { const out = []; if (cands[0]?.[0]) pushUnique(out, cands[0][0], SEARCH_LIMIT); if (raw[0]) pushUnique(out, raw[0], SEARCH_LIMIT); for (const t of roundRobin([(cands[0] ?? []).slice(1), ...cands.slice(1), raw.slice(1)])) pushUnique(out, t, SEARCH_LIMIT); return out; },
  // Production, but consensus only counts agreement with the entity candidate (cand1), so a paraphrase cand2 can't inflate it.
  'consensus-guard': ({ raw, cands }) => { const out = []; const rewrite = cands.flat(); const c1 = cands[0] ?? [];
    for (const t of c1) { const canonical = raw.find((r) => ciEq(r, t)); if (canonical !== undefined) pushUnique(out, canonical, SEARCH_LIMIT); }
    const reserve = raw.length > 0 && !out.some((e) => ciEq(e, raw[0]));
    for (const t of rewrite) pushUnique(out, t, reserve ? SEARCH_LIMIT - 1 : SEARCH_LIMIT);
    for (const t of raw) pushUnique(out, t, SEARCH_LIMIT);
    return out; },
  'raw-first': ({ raw, cands }) => { const out = []; if (raw[0]) pushUnique(out, raw[0], SEARCH_LIMIT); for (const t of roundRobin([...cands, raw.slice(1)])) pushUnique(out, t, SEARCH_LIMIT); return out; },
};

const tally = Object.fromEntries(Object.keys(POLICIES).map((k) => [k, { hit: 0, n: 0 }]));
let judged = 0; let ambiguous = 0; let skipped = 0;
const changed = { gained: {}, lost: {} };
for (const r of recs) {
  if (!['hit', 'wrong_nonzero', 'zero_hit'].includes(r.status) || !r.rawTitles) { skipped++; continue; }
  let cands = r.rewriteHitsByCandidate;
  if (!cands) {
    const n = (r.candidatesSearched ?? []).length;
    if (n <= 1) cands = n === 0 ? [] : [r.rewriteHits ?? []];
    else if ((r.rewriteHits ?? []).length === 8) cands = [r.rewriteHits.slice(0, 4), r.rewriteHits.slice(4)];
    else { ambiguous++; continue; }
  }
  judged++;
  const gold = [...(r.goldResolved ?? []), ...(r.gold ?? [])];
  const resolve = (t) => r.resolvedMap?.[t] ?? t;
  const isHit = (list) => list.some((t) => gold.some((g) => ciEq(resolve(t), g) || ciEq(t, g)));
  const prod = isHit(POLICIES.production({ raw: r.rawTitles, cands }));
  for (const [name, fn] of Object.entries(POLICIES)) {
    const h = isHit(fn({ raw: r.rawTitles, cands }));
    tally[name].n++; if (h) tally[name].hit++;
    if (name !== 'production' && h !== prod) { const b = h ? changed.gained : changed.lost; (b[name] ??= []).push(r.id); }
  }
}
const pct = (a, b) => (b ? Math.round((1000 * a) / b) / 10 : null);
console.log(`merge-policy replay over ${judged} judged records (${ambiguous} ambiguous old-format records skipped, ${skipped} non-judged)`);
for (const [name, t] of Object.entries(tally).sort((a, b) => b[1].hit - a[1].hit)) {
  const g = changed.gained[name]?.length ?? 0; const l = changed.lost[name]?.length ?? 0;
  console.log(`  ${name.padEnd(13)} ${String(t.hit).padStart(4)}/${t.n}  ${pct(t.hit, t.n)}%${name !== 'production' ? `   vs production: +${g} −${l}` : ''}`);
}
for (const name of Object.keys(POLICIES)) {
  if (name === 'production') continue;
  if (changed.gained[name]?.length) console.log(`  ${name} gains: ${[...new Set(changed.gained[name])].join(' ')}`);
  if (changed.lost[name]?.length) console.log(`  ${name} loses: ${[...new Set(changed.lost[name])].join(' ')}`);
}
