// Validate the fixture's `fact.evidence` strings against the live gold pages:
// each evidence must be a literal (case/whitespace-insensitive) substring of
// the gold page's reduced text. Prints the offset and whether it sits beyond
// the 8000-char cap — so curators know which facts probe truncation.
//
//   node eval/check-facts.mjs [--only ids]

import { loadFixture, fetchRenderedPage, sleep, MAX_PAGE_CHARS } from './lib.mjs';
import { toPlaintext } from './html.mjs';

const args = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => (a.startsWith('--') ? [a.slice(2), arr[i + 1] ?? true] : null)).filter(Boolean));
const fixture = loadFixture();
let qs = fixture.questions.filter((q) => q.fact);
if (args.only) { const ids = new Set(String(args.only).split(',')); qs = qs.filter((q) => ids.has(q.id)); }
const norm = (s) => s.toLowerCase().replace(/\s+/g, ' ');

let bad = 0;
for (const q of qs) {
  const wiki = fixture.wikis[q.game];
  let found = null;
  for (const g of q.gold) {
    const p = await fetchRenderedPage(wiki, g, toPlaintext);
    await sleep(300);
    if (p.error || p.empty) continue;
    const idx = norm(p.fullText).indexOf(norm(q.fact.evidence));
    if (idx !== -1) { found = { page: p.title, idx, chars: p.rawChars }; break; }
    if (!found) found = { page: p.title, idx: -1, chars: p.rawChars };
  }
  if (!found || found.idx === -1) { bad++; console.log(`✗ ${q.id.padEnd(16)} evidence NOT FOUND in ${found?.page ?? q.gold.join('|')} — "${q.fact.evidence}"`); continue; }
  console.log(`✓ ${q.id.padEnd(16)} ${found.page} @${found.idx}/${found.chars}${found.idx >= MAX_PAGE_CHARS ? '  ⚠ BEYOND CAP' : ''}`);
}
console.log(`${qs.length - bad}/${qs.length} facts verified${bad ? ` — ${bad} to fix` : ''}`);
process.exit(bad ? 1 : 0);
