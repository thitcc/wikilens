// Parity: titles.mjs against the unit vectors in src-tauri/src/wiki/titles.rs.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { TitleIndex, parseAllpages, normalize, stripNamespace, jaroWinkler } from './titles.mjs';

test('parses allpages titles and continue (titles.rs)', () => {
  const body = '{"continue":{"apcontinue":"Beach","continue":"-||"},"query":{"allpages":[{"pageid":1,"ns":0,"title":"Abigail"},{"pageid":2,"ns":0,"title":"Amethyst"}]}}';
  assert.deepEqual(parseAllpages(body), { titles: ['Abigail', 'Amethyst'], apcontinue: 'Beach' });
  assert.deepEqual(parseAllpages('{"query":{"allpages":[{"title":"Zenith"}]}}'), { titles: ['Zenith'], apcontinue: null });
  assert.throws(() => parseAllpages('{"query":{}}'));
});

test('normalize lowercases and collapses punctuation (titles.rs)', () => {
  assert.equal(normalize('Arcane Persistence!'), 'arcane persistence');
  assert.equal(normalize('  Wood-Chipper  '), 'wood chipper');
  assert.equal(normalize('Skyrim:Whiterun'), 'skyrim whiterun');
});

test('strip_namespace drops a leading prefix (titles.rs)', () => {
  assert.equal(stripNamespace('Skyrim:Whiterun'), 'Whiterun');
  assert.equal(stripNamespace('Whiterun'), 'Whiterun');
  assert.equal(stripNamespace('A:'), 'A:');
});

test('jaro-winkler matches known values (titles.rs)', () => {
  assert.ok(Math.abs(jaroWinkler('martha', 'marhta') - 0.961) < 0.01);
  assert.equal(jaroWinkler('same', 'same'), 1);
  assert.equal(jaroWinkler('', ''), 1);
  assert.equal(jaroWinkler('abc', ''), 0);
  assert.ok(jaroWinkler('cat', 'dog') < 0.5);
});

test('best_match resolves a typo to a real title (titles.rs)', () => {
  const index = new TitleIndex(['Excalibur', 'Persistence', 'Mag'], false);
  assert.equal(index.bestMatch('Excalibar'), 'Excalibur');
  assert.equal(index.bestMatch('persistance'), 'Persistence');
  assert.equal(index.bestMatch('zzzzzzzz'), null);
  assert.equal(index.bestMatch(''), null);
});

test('short query does not match a longer prefix title (titles.rs)', () => {
  const index = new TitleIndex(['Magic', 'Iron Ore'], false);
  assert.equal(index.bestMatch('mag'), null);
  assert.equal(index.bestMatch('iron'), null);
});

test('namespace stripping is gated on the wiki using one (titles.rs)', () => {
  assert.equal(new TitleIndex(['Skyrim:Whiterun'], true).bestMatch('whiterun'), 'Skyrim:Whiterun');
  assert.equal(new TitleIndex(['Skyrim:Whiterun'], false).bestMatch('whiterun'), null);
});
