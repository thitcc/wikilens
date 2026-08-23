// Answer-level eval — run the answer phase on a retrieval run's results and
// judge each answer against the fixture's expected fact.
//
//   node eval/run-answer.mjs --run eval/out/<run-name> [--round 1] [--only ids] [--pace 300]
//
// For every fixture question that carries `fact` (or the --only ids): take
// the titles the retrieval round actually merged, fetch them the way
// fetch.rs does (sequential `action=parse`, 12s, 8000-char truncation,
// relevance order), build the exact user message (llm.rs) and call the
// Default-mode answer model with the production system prompt and
// max_tokens (non-streaming). Then:
//   factInContext  — the fact's evidence string occurs in the context SENT
//                    (truncated pages) — programmatic, case-insensitive;
//   factBeyondCap  — it occurs in a fetched page's full text but not in the
//                    truncated part (the R10 class);
//   judge          — a second, constrained call on the same model: given
//                    question + expected fact + answer, label
//                    correct | partial | wrong | abstain (+ one-line note).
// Non-mirrors (recorded, not hidden): the wikitext fallback — a failed parse
// marks the record `degraded` and the page is skipped; the zero-hit canned
// answer — a retrieval zero_hit is recorded with no model call.
// Output: <run>/answers.jsonl (answers stay local; summaries go in the vault).

import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  loadDefaultTarget, loadFixture, readJsonl, appendJsonl, fetchRenderedPage, sortByRelevance,
  answerCompletion, sleep,
} from './lib.mjs';
import { toPlaintext } from './html.mjs';

const args = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => (a.startsWith('--') ? [a.slice(2), arr[i + 1] ?? true] : null)).filter(Boolean));
if (!args.run) { console.error('usage: --run <dir> [--round 1] [--only ids] [--pace ms]'); process.exit(2); }
const RUN = args.run;
const ROUND = Number(args.round ?? 1);
const PACE = Number(args.pace ?? 300);
const fixture = loadFixture();
const WIKIS = fixture.wikis;
const targets = loadDefaultTarget();
const answerTarget = targets.answer;

const roundRecs = readJsonl(join(RUN, `round-${ROUND}.jsonl`));
const byId = new Map(roundRecs.map((r) => [r.id, r]));
let questions = fixture.questions.filter((q) => q.fact);
if (args.only) { const ids = new Set(String(args.only).split(',')); questions = fixture.questions.filter((q) => ids.has(q.id)); }
const OUT = join(RUN, 'answers.jsonl');
writeFileSync(OUT, '');
console.log(`answer eval: ${questions.length} questions from ${RUN} round ${ROUND}; model=${answerTarget.kind}/${answerTarget.model}`);

const JUDGE_SYSTEM = 'You grade an in-game wiki assistant\'s answer against a known expected fact. Labels: "correct" — the answer states the expected fact (wording may differ; extra correct detail is fine); "partial" — it states part of the fact or hedges it with wrong extras; "wrong" — it states something that contradicts the fact, or confidently answers with a different fact; "abstain" — it says it cannot find / does not know / suggests searching elsewhere instead of answering. Judge only against the expected fact given; do not use outside knowledge. Reply ONLY with compact JSON: {"label":"correct|partial|wrong|abstain","note":"<≤20 words>"}';

async function judge(question, fact, answer) {
  const user = `Question: ${question}\nExpected fact: ${fact.expected}\nEvidence from the wiki page: ${fact.evidence}\n\nAssistant answer:\n${answer}`;
  const headers = answerTarget.kind === 'anthropic'
    ? { 'x-api-key': answerTarget.apiKey, 'anthropic-version': '2023-06-01', 'content-type': 'application/json' }
    : { Authorization: `Bearer ${answerTarget.apiKey}`, 'content-type': 'application/json' };
  const body = answerTarget.kind === 'anthropic'
    ? { model: answerTarget.model, max_tokens: 120, system: JUDGE_SYSTEM, messages: [{ role: 'user', content: user }] }
    : { model: answerTarget.model, max_tokens: 120, messages: [{ role: 'system', content: JUDGE_SYSTEM }, { role: 'user', content: user }] };
  const t0 = Date.now();
  try {
    const r = await fetch(answerTarget.endpoint, { method: 'POST', headers, body: JSON.stringify(body), signal: AbortSignal.timeout(30000) });
    if (!r.ok) return { label: null, error: `HTTP ${r.status}`, ms: Date.now() - t0 };
    const j = await r.json();
    const txt = answerTarget.kind === 'anthropic' ? (j?.content ?? []).find((b) => b.type === 'text')?.text : j?.choices?.[0]?.message?.content;
    const m = typeof txt === 'string' ? txt.match(/\{[\s\S]*\}/) : null;
    const parsed = m ? JSON.parse(m[0]) : null;
    const label = ['correct', 'partial', 'wrong', 'abstain'].includes(parsed?.label) ? parsed.label : null;
    return { label, note: parsed?.note ?? null, raw: label ? undefined : txt, ms: Date.now() - t0 };
  } catch (e) {
    return { label: null, error: String(e?.name ?? e), ms: Date.now() - t0 };
  }
}

const norm = (s) => s.toLowerCase().replace(/\s+/g, ' ');

async function main() {
  let k = 0;
  for (const q of questions) {
    const wiki = WIKIS[q.game];
    const ret = byId.get(q.id);
    const rec = { id: q.id, game: q.game, style: q.style, source: q.source, question: q.question, gold: q.gold, fact: q.fact ?? null, round: ROUND, retrievalStatus: ret?.status ?? 'missing' };
    if (!ret || !['hit', 'wrong_nonzero'].includes(ret.status)) {
      rec.skipped = ret ? `retrieval ${ret.status}` : 'no retrieval record';
      appendJsonl(OUT, rec);
      console.log(`${q.id} skipped (${rec.skipped})`);
      continue;
    }
    // fetch.rs fetch_pages — sequential, relevance order restored.
    const titles = ret.merged;
    const pages = [];
    let degraded = false;
    const t0 = Date.now();
    for (const t of titles) {
      const p = await fetchRenderedPage(wiki, t, toPlaintext);
      if (p.error) degraded = true; else if (!p.empty) pages.push(p);
      await sleep(PACE);
    }
    rec.fetchMs = Date.now() - t0;
    rec.degraded = degraded;
    const ordered = sortByRelevance(pages, titles);
    rec.pages = ordered.map((p) => ({ title: p.title, chars: [...p.text].length, rawChars: p.rawChars, truncated: p.truncated }));
    rec.contextChars = ordered.reduce((s, p) => s + p.text.length, 0);
    if (ordered.length === 0) {
      rec.skipped = 'pages unreadable'; // commands.rs: "I found matching pages but couldn't read their contents."
      appendJsonl(OUT, rec);
      console.log(`${q.id} skipped (pages unreadable)`);
      continue;
    }
    if (q.fact?.evidence) {
      const ev = norm(q.fact.evidence);
      rec.factInContext = ordered.some((p) => norm(p.text).includes(ev));
      rec.factInFullPages = ordered.some((p) => norm(p.fullText).includes(ev));
      rec.factBeyondCap = !rec.factInContext && rec.factInFullPages;
      rec.factGoldPageInContext = ordered.some((p) => q.gold.some((g) => g.toLowerCase() === p.title.toLowerCase()));
    }
    // llm.rs answer — same prompt, cap, content; non-streaming.
    const a = await answerCompletion(answerTarget, q.question, ordered.map((p) => ({ title: p.title, text: p.text })));
    rec.answer = { text: a.text, stopReason: a.stopReason, usage: a.usage, ms: a.ms, error: a.error, userChars: a.userChars };
    if (a.text && q.fact) {
      await sleep(PACE);
      rec.judge = await judge(q.question, q.fact, a.text);
    }
    appendJsonl(OUT, rec);
    k++;
    console.log(`${q.id.padEnd(5)} ${rec.retrievalStatus.padEnd(13)} ctx=${rec.contextChars} fact:${rec.factInContext === undefined ? '-' : rec.factInContext ? 'in' : rec.factBeyondCap ? 'BEYOND-CAP' : 'absent'} stop=${a.stopReason} judge=${rec.judge?.label ?? '-'} (${rec.fetchMs}/${a.ms}ms)${degraded ? ' [degraded]' : ''}${a.error ? ` ERR ${a.error}` : ''}`);
    await sleep(PACE);
  }
  console.log(`ANSWER EVAL COMPLETE: ${k} answered → ${OUT}`);
}

main().catch((e) => { console.error('FATAL', e); process.exit(1); });
