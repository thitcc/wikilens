// Unit tests for licenses.mjs — Node's built-in runner (`npm run test:node`).
// No real cargo or npm: cargo-about is a fake `exec` that writes canned JSON
// to the `-o` path, and the npm side walks a fixture lockfile + node_modules
// tree under a temp dir.

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import {
  LicensesError,
  OUTPUT_REL,
  checkTool,
  collectNpm,
  findLicenseFile,
  distinctComponents,
  normalizeText,
  reduceCargoAbout,
  render,
  runLicenses,
} from './licenses.mjs';

const MIT = 'MIT License\n\nCopyright (c) Example\n\nPermission is hereby granted...';
const APACHE = 'Apache License\nVersion 2.0, January 2004\n...';
const ISC = 'ISC License\n\nCopyright (c) ISC Example\n\nPermission to use...';

const CARGO_ABOUT_JSON = {
  licenses: [
    {
      name: 'MIT License',
      id: 'MIT',
      text: MIT,
      used_by: [
        { crate: { name: 'zeta', version: '2.0.0', repository: 'https://example.com/zeta' } },
        { crate: { name: 'alpha', version: '1.0.0', repository: 'https://example.com/alpha' } },
      ],
    },
    {
      name: 'Apache License 2.0',
      id: 'Apache-2.0',
      text: APACHE + '\r\n',
      used_by: [{ crate: { name: 'beta', version: '0.3.1', repository: null } }],
    },
    // alpha is `MIT AND Unicode-3.0`: listed under two texts, still one crate.
    {
      name: 'Unicode License v3',
      id: 'Unicode-3.0',
      text: 'UNICODE LICENSE V3\n\nPermission is hereby granted...',
      used_by: [{ crate: { name: 'alpha', version: '1.0.0', repository: 'https://example.com/alpha' } }],
    },
  ],
};

let root;
before(() => {
  root = mkdtempSync(join(tmpdir(), 'wikilens-licenses-test-'));
  mkdirSync(join(root, 'src-tauri'));
  const pkg = (rel, files) => {
    const dir = join(root, ...rel.split('/'));
    mkdirSync(dir, { recursive: true });
    for (const [name, content] of Object.entries(files)) writeFileSync(join(dir, name), content);
  };
  pkg('node_modules/react', { LICENSE: MIT, 'package.json': '{"license":"MIT"}' });
  pkg('node_modules/react-dom', { LICENSE: MIT });
  // One file per alternative — the MIT one must win.
  pkg('node_modules/@tauri-apps/api', { LICENSE_MIT: MIT, 'LICENSE_APACHE-2.0': APACHE });
  // Ships only an SPDX document; covered by NPM_LICENSE_OVERRIDES.
  pkg('node_modules/@tauri-apps/plugin-opener', { 'LICENSE.spdx': 'SPDXVersion: SPDX-2.2' });
  // Nested install with CRLF text and a decoy script named like a license.
  pkg('node_modules/react-markdown/node_modules/nested', { 'LICENSE.md': ISC.replace(/\n/g, '\r\n'), 'license.js': 'module.exports = 1' });
  pkg('node_modules/react-markdown', { LICENSE: MIT });
  // Dev-only: present on disk, must not be listed.
  pkg('node_modules/vitest', { 'LICENSE.md': MIT });
  writeFileSync(
    join(root, 'package-lock.json'),
    JSON.stringify({
      name: 'wikilens',
      lockfileVersion: 3,
      packages: {
        '': { name: 'wikilens', version: '0.0.0' },
        'node_modules/react': { version: '19.0.0', license: 'MIT' },
        'node_modules/react-dom': { version: '19.0.0', license: 'MIT' },
        'node_modules/@tauri-apps/api': { version: '2.0.0', license: 'Apache-2.0 OR MIT' },
        'node_modules/@tauri-apps/plugin-opener': { version: '2.0.0', license: 'MIT OR Apache-2.0' },
        'node_modules/react-markdown': { version: '9.0.0', license: 'MIT' },
        'node_modules/react-markdown/node_modules/nested': { version: '1.2.3', license: 'ISC' },
        'node_modules/vitest': { version: '3.0.0', license: 'MIT', dev: true },
        'node_modules/fsevents': { version: '2.3.3', license: 'MIT', optional: true },
      },
    }),
  );
});
after(() => rmSync(root, { recursive: true, force: true }));

/** cargo-about stand-in: answers the version probe and writes JSON to `-o`. */
function fakeExec({ version = 'cargo-about 0.9.2', json = CARGO_ABOUT_JSON, status = 0 } = {}) {
  const calls = [];
  const exec = (tool, args, cwd) => {
    calls.push({ tool, args, cwd });
    if (args[0] === 'about' && args[1] === '--version') return { status: 0, stdout: `${version}\n` };
    if (args[1] === 'generate') {
      if (status !== 0) return { status, stdout: '' };
      const out = args[args.indexOf('-o') + 1].replace(/^"|"$/g, '');
      writeFileSync(out, JSON.stringify(json));
      return { status: 0, stdout: '' };
    }
    throw new Error(`unexpected command ${tool} ${args.join(' ')}`);
  };
  exec.calls = calls;
  return exec;
}

test('normalizeText: LF only, no trailing whitespace or newlines', () => {
  assert.equal(normalizeText('a \r\nb\r\n\r\n'), 'a\nb');
  assert.equal(normalizeText('x\ry\n'), 'x\ny');
});

test('findLicenseFile: prefers the MIT variant, ignores scripts and SPDX documents', () => {
  assert.equal(findLicenseFile(join(root, 'node_modules/@tauri-apps/api')), 'LICENSE_MIT');
  assert.equal(findLicenseFile(join(root, 'node_modules/react-markdown/node_modules/nested')), 'LICENSE.md');
  assert.equal(findLicenseFile(join(root, 'node_modules/@tauri-apps/plugin-opener')), null);
});

test('collectNpm: production packages only, grouped by text, sorted, CRLF normalized', () => {
  const warnings = [];
  const npm = collectNpm(root, { warn: (m) => warnings.push(m) });
  const listed = npm.flatMap((e) => e.usedBy.map((u) => `${u.name}@${u.version}`));
  assert.deepEqual(listed, [
    // Apache-2.0 OR MIT / ISC / MIT... — entries sort by id, then packages by name.
    '@tauri-apps/api@2.0.0',
    '@tauri-apps/plugin-opener@2.0.0',
    'react@19.0.0',
    'react-dom@19.0.0',
    'react-markdown@9.0.0',
    'nested@1.2.3',
  ]);
  assert.ok(!listed.some((p) => p.startsWith('vitest')), 'dev-only package must be excluded');
  assert.equal(npm.length, 2, 'identical MIT texts merge into one entry; ISC is its own');
  const mit = npm.find((e) => e.usedBy.some((u) => u.name === 'react'));
  assert.equal(mit.id, 'Apache-2.0 OR MIT / MIT / MIT OR Apache-2.0');
  assert.equal(mit.text, MIT);
  const isc = npm.find((e) => e.id === 'ISC');
  assert.equal(isc.text, ISC, 'CRLF license file normalized to LF');
  assert.deepEqual(warnings, ['licenses: fsevents@2.3.3 is optional and not installed — skipped']);
});

test('collectNpm: a production package without a license text fails naming it', () => {
  const bare = mkdtempSync(join(tmpdir(), 'wikilens-licenses-bare-'));
  try {
    mkdirSync(join(bare, 'node_modules', 'naked'), { recursive: true });
    writeFileSync(join(bare, 'node_modules', 'naked', 'index.js'), '');
    writeFileSync(
      join(bare, 'package-lock.json'),
      JSON.stringify({ lockfileVersion: 3, packages: { '': {}, 'node_modules/naked': { version: '1.0.0' } } }),
    );
    assert.throws(() => collectNpm(bare), (e) => e instanceof LicensesError && /naked@1\.0\.0 .*ships no license text/.test(e.message));
  } finally {
    rmSync(bare, { recursive: true, force: true });
  }
});

test('reduceCargoAbout: sorts entries and crates, normalizes text, drops machine fields', () => {
  const rust = reduceCargoAbout(CARGO_ABOUT_JSON);
  assert.deepEqual(
    rust.map((e) => e.id),
    ['Apache-2.0', 'MIT', 'Unicode-3.0'],
  );
  assert.deepEqual(
    rust[1].usedBy.map((c) => c.name),
    ['alpha', 'zeta'],
  );
  assert.equal(rust[0].text, APACHE.replace(/\n$/, ''));
  assert.equal(rust[0].usedBy[0].repository, '');
  assert.ok(!('used_by' in rust[0]));
});

test('reduceCargoAbout: a source file mistaken for a license text is an error', () => {
  const source = { licenses: [{ ...CARGO_ABOUT_JSON.licenses[0], text: '#![allow(clippy::all)]\n// Copied from regex_syntax\n// MIT License\npub fn escape(text: &str) -> String {}' }] };
  assert.throws(() => reduceCargoAbout(source), (e) => e instanceof LicensesError && /source file as the MIT text for alpha, zeta/.test(e.message) && /clarify/.test(e.message));
  const header = { licenses: [{ ...CARGO_ABOUT_JSON.licenses[0], text: '// Copyright (c) Example\n// Permission is hereby granted...\npub(crate) static TABLE: [u16; 2] = [1, 2];' }] };
  assert.throws(() => reduceCargoAbout(header), LicensesError);
  // Prose that merely mentions "use" or "fn" is still a license.
  assert.doesNotThrow(() => reduceCargoAbout({ licenses: [{ ...CARGO_ABOUT_JSON.licenses[0], text: 'Permission to use, copy, modify... fn is not a keyword here.' }] }));
});

test('reduceCargoAbout: the app crate leaking in is an error', () => {
  const leaked = { licenses: [{ ...CARGO_ABOUT_JSON.licenses[0], used_by: [{ crate: { name: 'wikilens', version: '0.1.1' } }] }] };
  assert.throws(() => reduceCargoAbout(leaked), (e) => e instanceof LicensesError && /publish = false/.test(e.message));
  assert.throws(() => reduceCargoAbout({ licenses: [] }), (e) => e instanceof LicensesError && /no licenses/.test(e.message));
});

test('render: header counts, both parts, one trailing newline, no CR, order-independent', () => {
  const rust = reduceCargoAbout(CARGO_ABOUT_JSON);
  const npm = collectNpm(root, { warn: () => {} });
  const text = render({ rust, npm });
  assert.match(text, /^WikiLens — third-party licenses\n/);
  // alpha sits under MIT and Unicode-3.0: 4 memberships, 3 distinct crates.
  assert.match(text, /3 crates under 3 license texts\.\n  MIT \(2\), Apache-2\.0 \(1\), Unicode-3\.0 \(1\)\n/);
  assert.equal(distinctComponents(rust), 3);
  assert.match(text, /6 packages under 2 license texts\./);
  assert.match(text, /PART 1 — RUST CRATES\n/);
  assert.match(text, /PART 2 — NPM PACKAGES\n/);
  assert.match(text, /\n  - alpha 1\.0\.0 — https:\/\/example\.com\/alpha\n/);
  assert.match(text, /\n  - beta 0\.3\.1\n/);
  assert.match(text, /\n  - react@19\.0\.0\n/);
  assert.ok(!text.includes('\r'));
  assert.ok(text.endsWith('\n') && !text.endsWith('\n\n'));
  // Shuffled input renders byte-identically.
  const shuffled = { licenses: [...CARGO_ABOUT_JSON.licenses].reverse().map((l) => ({ ...l, used_by: [...l.used_by].reverse() })) };
  assert.equal(render({ rust: reduceCargoAbout(shuffled), npm }), text);
});

test('checkTool: gates on cargo-about 0.9.x and names the install command', () => {
  assert.equal(checkTool(fakeExec(), root), 'cargo-about 0.9.2');
  assert.throws(
    () => checkTool(fakeExec({ version: 'cargo-about 0.8.1' }), root),
    (e) => e instanceof LicensesError && /0\.8\.1 found/.test(e.message) && /cargo install cargo-about/.test(e.message),
  );
  const missing = () => ({ status: 1, stdout: '', error: 'spawn cargo ENOENT' });
  assert.throws(() => checkTool(missing, root), (e) => e instanceof LicensesError && /not installed/.test(e.message) && /cargo install cargo-about --locked --version 0\.9\.2/.test(e.message));
});

test('runLicenses: creates, then reports unchanged; --check flags a stale file with its first differing line', () => {
  const exec = fakeExec();
  const first = runLicenses({ root, exec, warn: () => {} });
  assert.equal(first.status, 'created');
  assert.deepEqual({ crates: first.counts.crates, packages: first.counts.packages }, { crates: 3, packages: 6 });
  assert.equal(exec.calls[1].cwd, join(root, 'src-tauri'), 'cargo-about runs in src-tauri (about.toml lives there)');
  assert.ok(exec.calls[1].args.includes('--locked') && exec.calls[1].args.includes('--fail'));

  const before = readFileSync(join(root, OUTPUT_REL), 'utf8');
  assert.equal(runLicenses({ root, exec, warn: () => {} }).status, 'unchanged');
  assert.equal(runLicenses({ root, exec, check: true, warn: () => {} }).status, 'unchanged');
  assert.equal(readFileSync(join(root, OUTPUT_REL), 'utf8'), before, 'unchanged runs never rewrite');

  const lines = before.split('\n');
  lines[4] = 'edited by hand';
  writeFileSync(join(root, OUTPUT_REL), lines.join('\n'));
  const stale = runLicenses({ root, exec, check: true, warn: () => {} });
  assert.equal(stale.status, 'stale');
  assert.equal(stale.firstDiffLine, 5);
  assert.equal(readFileSync(join(root, OUTPUT_REL), 'utf8'), lines.join('\n'), '--check never writes');

  assert.equal(runLicenses({ root, exec, warn: () => {} }).status, 'updated');
  assert.equal(readFileSync(join(root, OUTPUT_REL), 'utf8'), before);
});

test('runLicenses: a failing cargo-about run propagates with the command named', () => {
  assert.throws(
    () => runLicenses({ root, exec: fakeExec({ status: 101 }), warn: () => {} }),
    (e) => e instanceof LicensesError && /`cargo about generate` exited with status 101/.test(e.message),
  );
});
