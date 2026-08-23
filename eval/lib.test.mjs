// Parity tests: the mirror's pure functions against the Rust crate's own test
// vectors (search.rs, commands.rs merge_tests, llm.rs). If a vector here
// drifts from the Rust one, the eval stops measuring what the app does.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  preprocessQuery, simplifyQuery, buildSearchParams, mergeHits, parseRewriteQueries,
  truncateText, buildPageUrl, buildUserMessage, MAX_PAGE_CHARS, reasoningFromId,
} from './lib.mjs';

test('preprocess drops stopwords and punctuation (search.rs)', () => {
  assert.equal(preprocessQuery('How do I make Abigail like me?'), 'Abigail');
  assert.equal(preprocessQuery('best crops for winter'), 'crops winter');
  assert.equal(preprocessQuery('what does Abigail like as a gift?'), 'Abigail gift');
});

test('preprocess drops intent words (search.rs)', () => {
  assert.equal(preprocessQuery('best strategy to get archon shards'), 'archon shards');
  assert.equal(preprocessQuery('how do I obtain a fusion core?'), 'fusion core');
  assert.equal(preprocessQuery('strategies for the Eidolon fight'), 'Eidolon fight');
});

test('preprocess keeps casing and falls back on all-filler (search.rs)', () => {
  assert.equal(preprocessQuery('Where is Krobus?'), 'Krobus');
  assert.equal(preprocessQuery('  how do you do  '), 'how do you do');
  assert.equal(preprocessQuery(''), '');
});

test('simplify keeps long and Capitalized tokens (search.rs)', () => {
  assert.equal(simplifyQuery('what does Leo like?'), 'Leo');
  assert.equal(simplifyQuery('how to get an iron bar id'), 'iron');
  assert.equal(simplifyQuery('how to get it'), '');
});

test('search params: srwhat=text only for multi-word queries, namespace when set', () => {
  const multi = buildSearchParams('best crops', 4, null);
  assert.equal(multi.get('srwhat'), 'text');
  assert.equal(multi.get('srnamespace'), null);
  const single = buildSearchParams('wood', 4, '134');
  assert.equal(single.get('srwhat'), null);
  assert.equal(single.get('srnamespace'), '134');
  assert.equal(single.get('srlimit'), '4');
});

test('merge: genuine consensus first, then round-robin (commands.rs)', () => {
  const raw = ['Trinity', 'Sirius & Orion', 'Mag'];
  const cands = [['Sirius & Orion', 'Wisp']];
  assert.deepEqual(mergeHits(raw, cands, 4), ['Sirius & Orion', 'Trinity', 'Wisp', 'Mag']);
});

test('merge: injects the entity when raw is junk (commands.rs)', () => {
  assert.deepEqual(mergeHits(['Version History'], [['Wine']], 4), ['Wine', 'Version History']);
});

test('merge: dedupes case-insensitively, keeps canonical, truncates (commands.rs)', () => {
  assert.deepEqual(mergeHits(['Wood', 'Stone'], [['wood', 'Clay']], 2), ['Wood', 'Clay']);
});

test('merge: empty inputs (commands.rs)', () => {
  assert.deepEqual(mergeHits(['A'], [], 4), ['A']);
  assert.deepEqual(mergeHits([], [['A']], 4), ['A']);
  assert.deepEqual(mergeHits([], [], 4), []);
  assert.deepEqual(mergeHits(['A'], [[]], 4), ['A']);
});

test('merge: round-robin shares the cap between all three lists (commands.rs)', () => {
  const cands = [['A', 'B', 'C', 'D'], ['E', 'F', 'G', 'H']];
  assert.deepEqual(mergeHits(['R1', 'R2'], cands, 4), ['A', 'E', 'R1', 'B']);
});

test('merge: pseudo-consensus echo does not count (commands.rs)', () => {
  const raw = ['R1', 'R2', 'R3', 'R4'];
  const cands = [['E1'], ['R1', 'R2', 'R3', 'R4']];
  assert.deepEqual(mergeHits(raw, cands, 4), ['E1', 'R1', 'R2', 'R3']);
});

test('merge: limit one seats the lead candidate (commands.rs)', () => {
  assert.deepEqual(mergeHits(['R1'], [['A']], 1), ['A']);
});

test('merge: copy threshold is three shared titles (commands.rs)', () => {
  const raw = ['R1', 'R2', 'R3', 'R4'];
  assert.deepEqual(mergeHits(raw, [['X', 'R1', 'R2', 'R3']], 4), ['X', 'R1', 'R2', 'R3']);
  assert.deepEqual(mergeHits(raw, [['X', 'R1', 'R2']], 4), ['R1', 'R2', 'X', 'R3']);
});

test('merge: raw top hit is no longer guaranteed a seat (commands.rs)', () => {
  const raw = ['R0', 'R1', 'R2', 'R3'];
  const cands = [['X', 'R1', 'R2'], ['Y']];
  assert.deepEqual(mergeHits(raw, cands, 4), ['R1', 'R2', 'X', 'Y']);
});

test('merge: a third-rank candidate hit can lose its seat (commands.rs)', () => {
  const raw = ['R1', 'R2', 'R3', 'R4'];
  assert.deepEqual(mergeHits(raw, [['A', 'B', 'G']], 4), ['A', 'R1', 'B', 'R2']);
});

test('merge: a failed first candidate keeps the second in its turn (commands.rs)', () => {
  assert.deepEqual(mergeHits(['R1'], [[], ['P']], 4), ['P', 'R1']);
});

test('rewrite parser: outermost object, trimmed, ci-deduped, non-strings dropped (llm.rs)', () => {
  assert.deepEqual(parseRewriteQueries('```json\n{"queries":[" Abigail ","abigail","Villagers"]}\n```'), ['Abigail', 'Villagers']);
  assert.deepEqual(parseRewriteQueries('{"queries":["A", 3, "", "B"]}'), ['A', 'B']);
  assert.deepEqual(parseRewriteQueries('not json'), []);
  assert.deepEqual(parseRewriteQueries('{"q":["A"]}'), []);
});

test('truncate marks overflow on a code-point boundary (fetch.rs)', () => {
  assert.equal(truncateText('hello'), 'hello');
  const long = 'é'.repeat(MAX_PAGE_CHARS + 50);
  const out = truncateText(long);
  assert.ok(out.endsWith('…[truncated]'));
  assert.equal([...out].length, MAX_PAGE_CHARS + [...'…[truncated]'].length);
});

test('page url: underscores, safe punctuation kept, unsafe percent-encoded (fetch.rs)', () => {
  const stardew = { page_url: 'https://stardewvalleywiki.com/' };
  assert.equal(buildPageUrl(stardew, 'Prismatic Shard'), 'https://stardewvalleywiki.com/Prismatic_Shard');
  assert.equal(buildPageUrl(stardew, 'Category:Crops'), 'https://stardewvalleywiki.com/Category:Crops');
  assert.equal(buildPageUrl(stardew, 'Bream (Fish)'), 'https://stardewvalleywiki.com/Bream_(Fish)');
  assert.equal(buildPageUrl(stardew, 'Bob & Alice?'), 'https://stardewvalleywiki.com/Bob_%26_Alice%3F');
});

test('user message: fenced excerpts then the question (llm.rs)', () => {
  const msg = buildUserMessage('q?', [{ title: 'T', text: 'body' }]);
  assert.equal(msg, '<wiki_excerpt title="T">\nbody\n</wiki_excerpt>\n\nPlayer question: q?');
});

test('reasoning tag is the :thinking suffix only (models.rs)', () => {
  assert.equal(reasoningFromId('deepseek-reasoner:thinking'), true);
  assert.equal(reasoningFromId('claude-haiku-4-5-20251001'), false);
});
