// Synthetic question generator — draft candidates for the curator, never
// written into questions.json directly.
//
//   node eval/gen-synthetic.mjs --game stardew --count 5 --out eval/out/synthetic-stardew.json
//   --search-seed "mount,armor,boss"   # sample via list=search instead of list=random
//                                     # (shared wikis where random rarely lands on the game, e.g. Grounded 2)
//
// Samples random main-namespace pages (filters: ≥ --min-bytes, no
// disambiguation/list pages; shared-wiki guards: Grounded 2 suffix, UESP
// namespace, Fallout 4 category), reads each page through the real reducer,
// and asks the Default-mode model for ONE player-voice question per page in a
// rotating style. 1 in 5 gets a programmatic typo in the subject word. The
// curator reviews the candidates (drop nonsense, fix gold alternates) before
// merging them as `source: "synthetic"` — reports quarantine that bucket.

import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { loadDefaultTarget, loadFixture, fetchRenderedPage, sleep, UA } from './lib.mjs';
import { toPlaintext } from './html.mjs';

const args = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => (a.startsWith('--') ? [a.slice(2), arr[i + 1] ?? true] : null)).filter(Boolean));
if (!args.game || !args.out) { console.error('usage: --game <id> --out <file> [--count 5] [--min-bytes 4000] [--seed-prefix Y]'); process.exit(2); }
const COUNT = Number(args.count ?? 5);
const MIN_BYTES = Number(args['min-bytes'] ?? 4000);
const fixture = loadFixture();
const wiki = fixture.wikis[args.game];
if (!wiki) throw new Error(`unknown wiki ${args.game}`);
const target = loadDefaultTarget().rewrite;

const STYLES = ['howto', 'stat', 'negation', 'compare', 'entity'];
const PACE = 400;

const GUIDE = {
  howto: 'a how-to question with intent words (how do i get / make / unlock / beat / farm …)',
  stat: 'a question about one specific number or list on the page (damage, price, drops, ingredients, duration, requirement)',
  negation: 'a "why can\'t i …" / "why isn\'t … working" / "does X not work with Y" question answerable from the page',
  compare: 'a comparison question between the page subject and one other thing the page itself mentions',
  entity: 'a "what is X" / "where do i find X" question about the page subject',
};

async function api(params) {
  const p = new URLSearchParams({ ...params, format: 'json' });
  const r = await fetch(`${wiki.api_url}?${p}`, { headers: { 'User-Agent': UA }, signal: AbortSignal.timeout(12000) });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
}

const ns = wiki.search_namespace ?? '0';
function acceptTitle(t) {
  if (/\((disambiguation)\)|^List of|\/|:/.test(t) && !(args.game === 'skyrim' && t.startsWith('Skyrim:'))) return false;
  if (args.game === 'grounded2' && !t.endsWith('(Grounded 2)')) return false;
  if (args.game === 'grounded' && t.endsWith('(Grounded 2)')) return false;
  return true;
}

async function samplePages() {
  const picked = [];
  const seen = new Set();
  const seeds = args['search-seed'] ? String(args['search-seed']).split(',').map((x) => x.trim()).filter(Boolean) : null;
  for (let attempt = 0; attempt < (seeds ? seeds.length : 8) && picked.length < COUNT * 2; attempt++) {
    const rnd = seeds
      ? await api({ action: 'query', list: 'search', srsearch: seeds[attempt], srnamespace: ns, srlimit: '50' })
      : await api({ action: 'query', list: 'random', rnnamespace: ns, rnlimit: '50', rnfilterredir: 'nonredirects' });
    await sleep(PACE);
    const titles = (seeds ? rnd?.query?.search ?? [] : rnd?.query?.random ?? []).map((r) => r.title).filter((t) => acceptTitle(t) && !seen.has(t));
    titles.forEach((t) => seen.add(t));
    if (titles.length === 0) continue;
    const info = await api({ action: 'query', prop: args.game === 'fallout4' ? 'info|categories' : 'info', titles: titles.slice(0, 50).join('|'), cllimit: 'max' });
    await sleep(PACE);
    for (const p of Object.values(info?.query?.pages ?? {})) {
      if (!p.title || (p.length ?? 0) < MIN_BYTES) continue;
      if (args.game === 'fallout4') {
        const cats = (p.categories ?? []).map((c) => c.title);
        if (!cats.some((c) => /Fallout 4/.test(c))) continue;
      }
      picked.push({ title: p.title, bytes: p.length });
    }
  }
  // Shuffle deterministically enough: sort by a hash of the title.
  picked.sort((a, b) => hash(a.title) - hash(b.title));
  return picked;
}
function hash(s) { let h = 2166136261; for (const c of s) { h ^= c.charCodeAt(0); h = Math.imul(h, 16777619) >>> 0; } return h; }

async function generate(page, style, text) {
  const system = 'You write ONE question a player might type into an in-game wiki assistant while playing. You are given a wiki page (title + text). Write the question about the PAGE SUBJECT ITSELF, answerable from the given text. Player voice: short (≤ 14 words), lowercase, casual, may include filler or intent words. Refer to the subject the way a player would say it — if the title is longer than two words, use a short natural name, never paste the full title verbatim. No quotes, no markdown. Reply ONLY with compact JSON: {"question":"..."}';
  const user = `Game: ${wiki.name}\nPage title: ${page.title}\nStyle: ${GUIDE[style]}\n\nPage text (excerpt):\n${text}`;
  const headers = target.kind === 'anthropic'
    ? { 'x-api-key': target.apiKey, 'anthropic-version': '2023-06-01', 'content-type': 'application/json' }
    : { Authorization: `Bearer ${target.apiKey}`, 'content-type': 'application/json' };
  const body = target.kind === 'anthropic'
    ? { model: target.model, max_tokens: 120, system, messages: [{ role: 'user', content: user }] }
    : { model: target.model, max_tokens: 120, messages: [{ role: 'system', content: system }, { role: 'user', content: user }] };
  const r = await fetch(target.endpoint, { method: 'POST', headers, body: JSON.stringify(body), signal: AbortSignal.timeout(20000) });
  if (!r.ok) return null;
  const j = await r.json();
  const txt = target.kind === 'anthropic' ? (j?.content ?? []).find((b) => b.type === 'text')?.text : j?.choices?.[0]?.message?.content;
  const m = typeof txt === 'string' ? txt.match(/\{[\s\S]*\}/) : null;
  if (!m) return null;
  try { const q = JSON.parse(m[0])?.question; return typeof q === 'string' ? q.trim() : null; } catch { return null; }
}

// Programmatic typo: mutate the longest word (≥5 letters) the question shares with the title.
function injectTypo(question, title) {
  const titleWords = new Set(title.toLowerCase().replace(/\(.*?\)/g, '').split(/[^a-z]+/).filter((w) => w.length >= 5));
  const words = question.split(' ');
  const idx = words.map((w, i) => [w.toLowerCase().replace(/[^a-z]/g, ''), i]).filter(([w]) => titleWords.has(w)).sort((a, b) => b[0].length - a[0].length)[0]?.[1];
  if (idx === undefined) return null;
  const w = words[idx];
  const k = 1 + (hash(w) % (w.length - 2));
  const kind = hash(question) % 3;
  let mutated;
  if (kind === 0) mutated = w.slice(0, k) + w.slice(k + 1); // drop a letter
  else if (kind === 1) mutated = w.slice(0, k) + w[k + 1] + w[k] + w.slice(k + 2); // swap adjacent
  else mutated = w.slice(0, k) + w[k] + w.slice(k); // double a letter
  if (!mutated || mutated === w) return null;
  const out = [...words]; out[idx] = mutated;
  return { question: out.join(' '), typoOf: w, typo: mutated };
}

async function main() {
  const pages = await samplePages();
  console.log(`${args.game}: ${pages.length} candidate pages (≥${MIN_BYTES} bytes)`);
  const out = [];
  let n = 0;
  for (const page of pages) {
    if (out.length >= COUNT) break;
    const fetched = await fetchRenderedPage(wiki, page.title, toPlaintext);
    await sleep(PACE);
    if (fetched.error || fetched.empty) continue;
    const full = fetched.fullText;
    // Lede + a middle window, so questions can target details past the lede.
    const excerpt = full.length > 4500 ? `${full.slice(0, 3000)}\n…\n${full.slice(Math.floor(full.length / 2), Math.floor(full.length / 2) + 1500)}` : full;
    const style = STYLES[n % STYLES.length];
    const q = await generate(page, style, excerpt);
    await sleep(PACE);
    if (!q) continue;
    const rec = { id: `${args['seed-prefix'] ?? 'Y'}${args.game}-${out.length + 1}`, game: args.game, style, source: 'synthetic', question: q, gold: [fetched.title], meta: { pageBytes: page.bytes, pageChars: fetched.rawChars } };
    if (n % 5 === 4) {
      const t = injectTypo(q, fetched.title);
      if (t) { rec.question = t.question; rec.style = 'typo'; rec.meta.baseStyle = style; rec.meta.typoOf = t.typoOf; rec.meta.typo = t.typo; }
    }
    out.push(rec);
    n++;
    console.log(`  ${rec.id} [${rec.style}] ${rec.question}  → ${fetched.title}`);
  }
  mkdirSync(dirname(args.out), { recursive: true });
  writeFileSync(args.out, JSON.stringify(out, null, 2));
  console.log(`wrote ${out.length} candidates → ${args.out}`);
}

main().catch((e) => { console.error('FATAL', e); process.exit(1); });
