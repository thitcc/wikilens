// Parity: the html.mjs port against the crate's insta snapshots — the same
// captured pages `html.rs` freezes. Byte-equal output or the test fails.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { toPlaintext } from './html.mjs';
import { REPO_ROOT } from './lib.mjs';

const WIKI_DIR = join(REPO_ROOT, 'src-tauri', 'src', 'wiki');
const SNAP_PREFIX = 'wikilens_lib__wiki__html__tests__';

// insta files carry a `---…---` YAML header before the snapshot body.
function snapshotBody(name) {
  const raw = readFileSync(join(WIKI_DIR, 'snapshots', `${SNAP_PREFIX}${name}.snap`), 'utf8').replace(/\r\n/g, '\n');
  const m = raw.match(/^---\n[\s\S]*?\n---\n([\s\S]*)$/);
  assert.ok(m, `snapshot ${name} has no header`);
  return m[1].replace(/\n+$/, '');
}

const fixture = (file) => readFileSync(join(WIKI_DIR, 'fixtures', file), 'utf8').replace(/\r\n/g, '\n');

for (const [file, snap] of [
  ['stardew_parsnip.html', 'stardew_parsnip'],
  ['fandom_portable_infobox.html', 'fandom_portable_infobox'],
  ['uesp_page.html', 'uesp_skyrim_iron'],
]) {
  test(`html port matches insta snapshot ${snap}`, () => {
    assert.equal(toPlaintext(fixture(file)), snapshotBody(snap));
  });
}

// The unit vectors from html.rs tests.
test('wikitable rows become pipe lines', () => {
  const html = '<table class="wikitable roundedborder"><tr><th>Stage 1</th><th>Harvest</th></tr>\n<tr><td>1 Day</td><td>Total: 7 Days</td></tr></table>';
  assert.equal(toPlaintext(html), 'Stage 1 | Harvest\n1 Day | Total: 7 Days');
});

test('drops navbox, toc, style, images, svg', () => {
  const html = '<style>.x{}</style><div id="toc">1 Contents</div>\n<table class="wikitable" id="navbox"><tr><td>Crops</td><td>Spring</td></tr></table>\n<p>Keep <img src="x.png" alt="icon" /> me</p><svg><path d="M0"/></svg>';
  assert.equal(toPlaintext(html), 'Keep me');
});

test('portable infobox fields become label: value lines', () => {
  const html = '<aside class="portable-infobox pi-background pi-theme-wikia">\n<h2 class="pi-item pi-title">Copper Ore</h2>\n<figure class="pi-item pi-image"><a href="x"><img src="y.png"/></a></figure>\n<div class="pi-item pi-data"><h3 class="pi-data-label pi-secondary-font">Rarity</h3><div class="pi-data-value pi-font">Common</div></div>\n<div class="pi-item pi-data"><h3 class="pi-data-label">Stackable</h3><div class="pi-data-value">&#10004;&#160;Yes</div></div>\n</aside>';
  const text = toPlaintext(html);
  assert.ok(text.includes('Copper Ore'));
  assert.ok(text.includes('Rarity: Common'));
  assert.ok(text.includes('Stackable: ✔ Yes'));
});

test('drops fandom navbox and checkbox cells', () => {
  const html = '<table class="fandom-table navbox"><tr><td>nav junk</td></tr></table>\n<table class="table-progress-tracking sortable fandom-table"><tr>\n<td class="table-progress-checkbox-cell"><input type="checkbox"/><label>tick</label></td>\n<td>Giant Mushroom</td><td>+25</td></tr></table>';
  assert.equal(toPlaintext(html), 'Giant Mushroom | +25');
});

test('decodes numeric entities and strips comments', () => {
  assert.equal(toPlaintext('<p>A &#8211; B &#x2192; C &amp; D<!-- hidden --></p>'), 'A – B → C & D');
});

test('br splits lines inside cells', () => {
  assert.equal(toPlaintext('<p>+19 food<br />+4.2 health</p>'), '+19 food\n+4.2 health');
});

test('nested tables produce their own rows', () => {
  const html = '<table id="outer"><tr><td id="infoboxsection" colspan="2">Sell Prices</td></tr>\n<tr><td><table class="no-wrap"><tr><td>60g</td><td>75g</td></tr></table></td></tr></table>';
  const text = toPlaintext(html);
  assert.ok(text.includes('Sell Prices'));
  assert.ok(text.includes('60g | 75g'));
});

test('empty and separator-only lines are dropped', () => {
  assert.equal(toPlaintext('<table><tr><td></td><td></td></tr><tr><td>real</td></tr></table>'), 'real');
});
