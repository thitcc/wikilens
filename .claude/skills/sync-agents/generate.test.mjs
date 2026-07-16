// Unit tests for the sync-agents generator (node --test .claude/skills/sync-agents/).
// Each test builds a throwaway fixture repo in the OS temp dir; nothing touches
// the real .claude/ or .agents/ trees.

import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';

import { runSync } from './generate.mjs';

const command = (description, body, extra = '') => `---\ndescription: ${description}\n${extra}---\n${body}`;
const skill = (name, description, body, extra = '') =>
  `---\nname: ${name}\ndescription: ${description}\n${extra}---\n${body}`;

function makeRoot(t, { commands = {}, skills = {}, files = {}, claudeMd = '# CLAUDE.md\n', codexConfig = true } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'sync-agents-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  if (claudeMd !== null) writeFileSync(join(root, 'CLAUDE.md'), claudeMd);
  if (codexConfig) {
    mkdirSync(join(root, '.codex'));
    writeFileSync(join(root, '.codex', 'config.toml'), 'project_doc_fallback_filenames = ["CLAUDE.md"]\n');
  }
  mkdirSync(join(root, '.claude', 'commands'), { recursive: true });
  for (const [name, content] of Object.entries(commands)) {
    writeFileSync(join(root, '.claude', 'commands', `${name}.md`), content);
  }
  for (const [name, content] of Object.entries(skills)) {
    mkdirSync(join(root, '.claude', 'skills', name), { recursive: true });
    writeFileSync(join(root, '.claude', 'skills', name, 'SKILL.md'), content);
  }
  for (const [rel, content] of Object.entries(files)) {
    const path = join(root, ...rel.split('/'));
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content);
  }
  return root;
}

const adapterPath = (root, name) => join(root, '.agents', 'skills', name, 'SKILL.md');
const manifestPath = (root) => join(root, '.agents', 'skills', '.sync-manifest.json');

test('converts a command into a Codex adapter, stripping Claude-only metadata', (t) => {
  const root = makeRoot(t, {
    commands: {
      foo: command('Do foo things', 'Run foo, then `/bar`. Args: "$ARGUMENTS".\n', 'allowed-tools: Read\nargument-hint: <x>\n'),
      bar: command('Do bar things', 'Bar body.\n'),
    },
  });
  const report = runSync(root);
  assert.deepEqual(report.generated, ['bar', 'foo']);
  assert.deepEqual(report.warnings, []);
  assert.deepEqual(report.stripped, [{ name: 'foo', fields: ['allowed-tools', 'argument-hint'] }]);
  const emitted = readFileSync(adapterPath(root, 'foo'), 'utf8');
  assert.match(emitted, /^---\nname: foo\ndescription: Do foo things\n---\n/);
  assert.ok(emitted.includes('Generated from .claude/commands/foo.md'));
  assert.ok(!emitted.includes('allowed-tools'));
  assert.ok(!emitted.includes('argument-hint'));
  assert.ok(emitted.includes('Run foo, then `$bar`.'));
  assert.ok(emitted.includes('Args: the text accompanying the skill mention.'));
});

test('leaves path segments that merely contain a workflow name untouched', (t) => {
  const root = makeRoot(t, {
    commands: {
      foo: command('Foo', 'See src/foo/mod.rs, commands/foo.md and vault/2026-01-01_foo.md.\n'),
    },
  });
  runSync(root);
  const emitted = readFileSync(adapterPath(root, 'foo'), 'utf8');
  assert.ok(emitted.includes('See src/foo/mod.rs, commands/foo.md and vault/2026-01-01_foo.md.'));
});

test('fails on an ambiguous dotted-relative workflow reference', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Read ./foo for details.\n') },
  });
  assert.throws(() => runSync(root), /unresolved Claude command reference/);
});

test('fails when a source skill name does not match its directory', (t) => {
  const root = makeRoot(t, { skills: { foo: skill('bar', 'Mismatch', 'Body.\n') } });
  assert.throws(() => runSync(root), /must match its directory/);
});

test('fails on a source skill without a description', (t) => {
  const root = makeRoot(t, { skills: { foo: `---\nname: foo\n---\nBody.\n` } });
  assert.throws(() => runSync(root), /non-empty description/);
});

test('fails on unknown frontmatter metadata', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Body.\n', 'model: opus\n') },
  });
  assert.throws(() => runSync(root), /unknown frontmatter key "model"/);
});

test('fails on multi-line frontmatter values', (t) => {
  const root = makeRoot(t, {
    commands: { foo: `---\ndescription: >-\n  wrapped\n---\nBody.\n` },
  });
  assert.throws(() => runSync(root), /unsupported frontmatter line/);
});

test('the declared vault-lint collision resolves to the source skill and is reported', (t) => {
  const root = makeRoot(t, {
    commands: { 'vault-lint': command('Lint via command', 'Command body.\n') },
    skills: { 'vault-lint': skill('vault-lint', 'Lint via skill', 'Skill body.\n') },
  });
  const report = runSync(root);
  assert.deepEqual(report.collisions, [
    { name: 'vault-lint', winner: '.claude/skills/vault-lint/SKILL.md', loser: '.claude/commands/vault-lint.md' },
  ]);
  const emitted = readFileSync(adapterPath(root, 'vault-lint'), 'utf8');
  assert.ok(emitted.includes('Skill body.'));
  assert.ok(!emitted.includes('Command body.'));
});

test('fails on an undeclared name collision', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo cmd', 'Body.\n') },
    skills: { foo: skill('foo', 'Foo skill', 'Body.\n') },
  });
  assert.throws(() => runSync(root), /undeclared name collision: "foo"/);
});

test('removes owned adapters whose source disappeared', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Foo body.\n'), bar: command('Bar', 'Bar body.\n') },
  });
  runSync(root);
  rmSync(join(root, '.claude', 'commands', 'bar.md'));
  const report = runSync(root);
  assert.deepEqual(report.removed, ['bar']);
  assert.deepEqual(report.unchanged, ['foo']);
  assert.ok(!existsSync(join(root, '.agents', 'skills', 'bar')));
});

test('refuses to overwrite an unowned file at an output path', (t) => {
  const root = makeRoot(t, { commands: { foo: command('Foo', 'Foo body.\n') } });
  mkdirSync(join(root, '.agents', 'skills', 'foo'), { recursive: true });
  writeFileSync(adapterPath(root, 'foo'), 'hand-written\n');
  assert.throws(() => runSync(root), /refusing to overwrite/);
  assert.equal(readFileSync(adapterPath(root, 'foo'), 'utf8'), 'hand-written\n');
});

test('adopts a byte-identical unowned adapter without rewriting it', (t) => {
  const root = makeRoot(t, { commands: { foo: command('Foo', 'Foo body.\n') } });
  runSync(root);
  rmSync(manifestPath(root)); // simulate a lost manifest
  const epoch = new Date('2020-01-01T00:00:00Z');
  utimesSync(adapterPath(root, 'foo'), epoch, epoch);
  const report = runSync(root);
  assert.deepEqual(report.unchanged, ['foo']);
  assert.equal(statSync(adapterPath(root, 'foo')).mtimeMs, epoch.getTime());
  assert.ok(existsSync(manifestPath(root)));
});

test('rejects a manifest entry that escapes .agents/skills instead of deleting through it', (t) => {
  const root = makeRoot(t, { commands: { foo: command('Foo', 'Foo body.\n') } });
  runSync(root);
  writeFileSync(join(root, 'victim.txt'), 'precious\n');
  writeFileSync(manifestPath(root), JSON.stringify({ version: 1, owned: ['../../victim.txt'] }));
  assert.throws(() => runSync(root), /corrupt .*\.sync-manifest\.json/);
  assert.equal(readFileSync(join(root, 'victim.txt'), 'utf8'), 'precious\n');
});

test('rejects a manifest entry with backslash separators instead of deleting an adapter', (t) => {
  const root = makeRoot(t, { commands: { foo: command('Foo', 'Foo body.\n') } });
  runSync(root);
  writeFileSync(manifestPath(root), JSON.stringify({ version: 1, owned: ['foo\\SKILL.md'] }));
  assert.throws(() => runSync(root), /corrupt .*\.sync-manifest\.json/);
  assert.ok(existsSync(adapterPath(root, 'foo')));
});

test('fails when an emitted body references a missing .claude engine path', (t) => {
  const root = makeRoot(t, {
    skills: { foo: skill('foo', 'Foo', 'Run node .claude/skills/foo/missing.mjs\n') },
  });
  assert.throws(() => runSync(root), /\.claude\/skills\/foo\/missing\.mjs, which does not exist/);
});

test('accepts an existing .claude engine path, even at the end of a sentence', (t) => {
  const root = makeRoot(t, {
    skills: { foo: skill('foo', 'Foo', 'See .claude/skills/foo/SKILL.md.\n') },
  });
  const report = runSync(root);
  assert.deepEqual(report.generated, ['foo']);
});

test('a second run is a byte- and mtime-preserving no-op', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Foo body.\n'), bar: command('Bar', 'Bar body.\n') },
    skills: { baz: skill('baz', 'Baz', 'Baz body.\n') },
  });
  const first = runSync(root);
  assert.equal(first.upToDate, false);
  assert.deepEqual(first.generated, ['bar', 'baz', 'foo']);
  const epoch = new Date('2020-01-01T00:00:00Z');
  for (const name of ['foo', 'bar', 'baz']) utimesSync(adapterPath(root, name), epoch, epoch);
  utimesSync(manifestPath(root), epoch, epoch);
  const second = runSync(root);
  assert.equal(second.upToDate, true);
  assert.deepEqual(second.generated, []);
  assert.deepEqual(second.updated, []);
  assert.deepEqual(second.removed, []);
  assert.deepEqual(second.unchanged, ['bar', 'baz', 'foo']);
  for (const name of ['foo', 'bar', 'baz']) {
    assert.equal(statSync(adapterPath(root, name)).mtimeMs, epoch.getTime());
  }
  assert.equal(statSync(manifestPath(root)).mtimeMs, epoch.getTime());
});

test('rewrites an owned adapter when its source changes', (t) => {
  const root = makeRoot(t, { commands: { foo: command('Foo', 'Old body.\n') } });
  runSync(root);
  writeFileSync(join(root, '.claude', 'commands', 'foo.md'), command('Foo', 'New body.\n'));
  const report = runSync(root);
  assert.deepEqual(report.updated, ['foo']);
  assert.ok(readFileSync(adapterPath(root, 'foo'), 'utf8').includes('New body.'));
});

test('fails when a root AGENTS.md would shadow CLAUDE.md discovery', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Body.\n') },
    files: { 'AGENTS.md': 'stale import\n' },
  });
  assert.throws(() => runSync(root), /AGENTS\.md found — it would shadow CLAUDE\.md/);
});

test('fails on a corrupted .Codex path in an emitted body', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Run node .Codex/skills/lint.mjs\n') },
  });
  assert.throws(() => runSync(root), /corrupted "\.Codex" path/);
});

test('fails on an argument placeholder the translation cannot resolve', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Weird $ARGUMENTSX token.\n') },
  });
  assert.throws(() => runSync(root), /leftover \$ARGUMENTS placeholder/);
});

test('warns when CLAUDE.md approaches the Codex instruction-size cap', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Body.\n') },
    claudeMd: 'x'.repeat(29 * 1024),
  });
  const report = runSync(root);
  assert.equal(report.warnings.length, 1);
  assert.match(report.warnings[0], /combined instruction chain/);
});

test('warns when .codex/config.toml does not wire up the CLAUDE.md fallback', (t) => {
  const root = makeRoot(t, {
    commands: { foo: command('Foo', 'Body.\n') },
    codexConfig: false,
  });
  const report = runSync(root);
  assert.equal(report.warnings.length, 1);
  assert.match(report.warnings[0], /project_doc_fallback_filenames/);
});
