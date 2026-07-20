// Unit tests for the vault linter (node --test .claude/skills/vault-lint/).
// Each test builds a throwaway vault in the OS temp dir; nothing touches the
// real vault/ tree.

import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { LintError, inlineList, parseFrontmatter, runLint } from './lint.mjs';

// A minimal index.md: frontmatter (it is linted like any other doc) plus a tag
// registry in the shape loadRegistry() expects.
const INDEX = `---
title: Index
type: note
status: active
created: 2026-07-19
updated: 2026-07-19
tags: []
related: []
---

# Vault index

## Tag registry (closed — add a tag here before using it)

\`vault\` · \`testing\` · \`frontend\`
`;

// Frontmatter that satisfies every required field; callers override via extra.
const doc = (extra = '') =>
  `---\ntitle: A doc\ntype: plan\nstatus: done\ncreated: 2026-07-19\nupdated: 2026-07-19\n${extra}---\n\n# A doc\n`;

function makeVault(t, docs = {}, { index = INDEX } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'vault-lint-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, 'vault'), { recursive: true });
  writeFileSync(join(root, 'vault', 'index.md'), index);
  for (const [name, content] of Object.entries(docs)) {
    writeFileSync(join(root, 'vault', `${name}.md`), content);
  }
  return root;
}

const errorsFor = (result, needle) =>
  result.issues.filter((i) => i.level === 'ERROR' && i.msg.includes(needle));
const warnsFor = (result, needle) =>
  result.issues.filter((i) => i.level === 'WARN' && i.msg.includes(needle));

// ---------- parseFrontmatter: block sequences ----------

test('parses a YAML block sequence into the same shape as the inline form', () => {
  const block = parseFrontmatter('---\nrelated:\n  - "[[a]]"\n  - "[[b]]"\n---\n');
  const inline = parseFrontmatter('---\nrelated: ["[[a]]", "[[b]]"]\n---\n');
  assert.deepEqual(inlineList(block.related), ['[[a]]', '[[b]]']);
  assert.deepEqual(inlineList(block.related), inlineList(inline.related));
});

test('a block sequence keeps each item quoted so the quoting check still sees quotes', () => {
  const fm = parseFrontmatter('---\nrelated:\n  - "[[a]]"\n---\n');
  assert.match(fm.related, /"\[\[a\]\]"/);
});

test('a block sequence does not swallow the next top-level key', () => {
  const fm = parseFrontmatter('---\nrelated:\n  - "[[a]]"\ncommit: 9689f39\ntags: [vault]\n---\n');
  assert.deepEqual(inlineList(fm.related), ['[[a]]']);
  assert.equal(fm.commit, '9689f39');
  assert.deepEqual(inlineList(fm.tags), ['vault']);
});

test('a key with an empty value and no list items stays empty', () => {
  // Guards vault/2026-07-13_testing-audit.md, whose bare `commit:` must keep
  // producing the "commit is empty" warning rather than absorbing later keys.
  const fm = parseFrontmatter('---\ncommit:\nstatus: done\n---\n');
  assert.equal(fm.commit, '');
  assert.equal(fm.status, 'done');
});

test('inline form, comments and blank lines are unaffected', () => {
  const fm = parseFrontmatter('---\ntitle: T\n\n# a comment\ntags: [vault, testing]\n---\n');
  assert.equal(fm.title, 'T');
  assert.deepEqual(inlineList(fm.tags), ['vault', 'testing']);
  assert.equal(fm['# a comment'], undefined);
});

// ---------- the three checks the bug silently disabled ----------

test('a dangling wikilink in a block sequence is an ERROR', (t) => {
  const root = makeVault(t, {
    '2026-07-19_a': doc('tags: [vault]\nrelated:\n  - "[[2026-07-19_nope]]"\n'),
  });
  const result = runLint(root);
  assert.equal(errorsFor(result, 'has no matching file').length, 1);
});

test('a dangling wikilink in inline form is an ERROR too (unchanged behavior)', (t) => {
  const root = makeVault(t, {
    '2026-07-19_a': doc('tags: [vault]\nrelated: ["[[2026-07-19_nope]]"]\n'),
  });
  assert.equal(errorsFor(runLint(root), 'has no matching file').length, 1);
});

test('a resolvable wikilink in a block sequence is accepted', (t) => {
  const root = makeVault(t, {
    '2026-07-19_a': doc('tags: [vault]\nrelated:\n  - "[[2026-07-19_b]]"\n'),
    '2026-07-19_b': doc('tags: [vault]\n'),
  });
  assert.equal(runLint(root).errors, 0);
});

test('an unquoted wikilink in a block sequence is an ERROR', (t) => {
  const root = makeVault(t, {
    '2026-07-19_a': doc('tags: [vault]\nrelated:\n  - [[2026-07-19_b]]\n'),
    '2026-07-19_b': doc('tags: [vault]\n'),
  });
  assert.equal(errorsFor(runLint(root), 'must be quoted').length, 1);
});

test('a done doc with block-form related and no tags draws no "no tags and no related" warning', (t) => {
  // The false positive that surfaced the bug: three related links read as zero.
  const root = makeVault(t, {
    '2026-07-19_a': doc('commit: 9689f39\ntags: []\nrelated:\n  - "[[2026-07-19_b]]"\n'),
    '2026-07-19_b': doc('tags: [vault]\n'),
  });
  assert.deepEqual(warnsFor(runLint(root), 'no tags and no related links'), []);
});

test('a done doc with genuinely no tags and no related still warns', (t) => {
  const root = makeVault(t, {
    '2026-07-19_a': doc('commit: 9689f39\ntags: []\nrelated: []\n'),
  });
  assert.equal(warnsFor(runLint(root), 'no tags and no related links').length, 1);
});

// ---------- report shape + exit-code contract ----------

test('a clean vault reports zero errors', (t) => {
  const root = makeVault(t, { '2026-07-19_a': doc('commit: 9689f39\ntags: [vault]\n') });
  const result = runLint(root);
  assert.equal(result.errors, 0);
  assert.equal(result.files, 2); // the doc + index.md
});

test('warnings do not count as errors (they never gate)', (t) => {
  const root = makeVault(t, { '2026-07-19_a': doc('tags: [vault]\n') }); // done, no commit
  const result = runLint(root);
  assert.equal(result.errors, 0);
  assert.ok(result.warns > 0);
});

test('a missing vault throws LintError rather than exiting', () => {
  const root = mkdtempSync(join(tmpdir(), 'vault-lint-empty-'));
  try {
    assert.throws(() => runLint(root), LintError);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('an unknown tag is an ERROR with a did-you-mean hint', (t) => {
  const root = makeVault(t, { '2026-07-19_a': doc('commit: 9689f39\ntags: [vaults]\n') });
  const [issue] = errorsFor(runLint(root), 'not in the registry');
  assert.ok(issue, 'expected a registry error');
  assert.match(issue.msg, /did you mean "vault"/);
});
