// Merge curated question candidates into questions.json and rewrite it in
// the canonical compact layout (one question per line), so diffs stay
// readable. Rejects duplicate ids; strips generator-only `meta` fields.
//
//   node eval/fixture-merge.mjs <candidates.json> [more.json …]
//   node eval/fixture-merge.mjs --reformat            # just rewrite the layout
//
// A candidates file is either a JSON array of questions or an object with a
// `questions` array (the shape gen-synthetic.mjs and the drafting briefs use).

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { EVAL_DIR, loadFixture } from './lib.mjs';

const FIXTURE = join(EVAL_DIR, 'questions.json');
const FIELD_ORDER = ['id', 'game', 'style', 'source', 'question', 'gold', 'trunc', 'fact', 'note'];

export function formatFixture(fixture) {
  const head = { ...fixture };
  delete head.questions;
  const headJson = JSON.stringify(head, null, 2).replace(/\n}$/, '');
  const lines = fixture.questions.map((q) => {
    const ordered = {};
    for (const k of FIELD_ORDER) if (q[k] !== undefined) ordered[k] = q[k];
    for (const k of Object.keys(q)) if (!(k in ordered) && k !== 'meta') ordered[k] = q[k];
    return `    ${JSON.stringify(ordered)}`;
  });
  return `${headJson},\n  "questions": [\n${lines.join(',\n')}\n  ]\n}\n`;
}

const args = process.argv.slice(2);
const fixture = loadFixture();
if (!args.includes('--reformat')) {
  const ids = new Set(fixture.questions.map((q) => q.id));
  let added = 0;
  for (const file of args) {
    const raw = JSON.parse(readFileSync(file, 'utf8'));
    const list = Array.isArray(raw) ? raw : raw.questions;
    for (const q of list) {
      if (ids.has(q.id)) { console.error(`skip duplicate id ${q.id} (${file})`); continue; }
      if (!fixture.wikis[q.game]) throw new Error(`${q.id}: unknown wiki ${q.game}`);
      ids.add(q.id);
      fixture.questions.push(q);
      added++;
    }
  }
  console.log(`added ${added} → ${fixture.questions.length} questions`);
}
writeFileSync(FIXTURE, formatFixture(fixture));
