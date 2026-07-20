#!/usr/bin/env node
// vault-lint — validate WikiLens planning-vault docs against their conventions.
//
// Zero-dependency. Resolves the vault relative to this file, so it runs from any cwd:
//   node .claude/skills/vault-lint/lint.mjs
//
// Source of truth: the tag registry is parsed live from vault/index.md so this script
// never drifts from it; the type/status vocabularies are fixed by CLAUDE.md §7.
// Exit code is non-zero iff there are errors (warnings never gate).

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, resolve, dirname, basename, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
// Defaulted, not hardcoded: tests point runLint() at a throwaway vault instead.
const DEFAULT_ROOT = resolve(__dirname, '..', '..', '..'); // .claude/skills/vault-lint -> repo root

export class LintError extends Error {}

const TYPES = ['plan', 'decision', 'research', 'fix', 'retro', 'note'];
const STATUSES = ['idea', 'todo', 'active', 'blocked', 'done', 'dropped'];
const REQUIRED = ['title', 'type', 'status', 'created', 'updated'];
const ISO = /^\d{4}-\d{2}-\d{2}$/;
const FILENAME = /^\d{4}-\d{2}-\d{2}_[a-z0-9-]+\.md$/;
const EXEMPT = new Set(['index.md']); // exempt from filename + index-reference rules

// ---------- helpers ----------

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name === 'templates' || name.startsWith('.')) continue; // placeholders / .obsidian
      out.push(...walk(p));
    } else if (name.endsWith('.md')) {
      out.push(p);
    }
  }
  return out;
}

function parseFrontmatter(text) {
  const lines = text.split(/\r?\n/);
  if (lines[0]?.trim() !== '---') return null;
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i].trim() === '---') { end = i; break; }
  }
  if (end === -1) return null;
  const data = {};
  const fm = lines.slice(1, end);
  for (let i = 0; i < fm.length; i++) {
    const line = fm[i];
    if (!line.trim() || line.trimStart().startsWith('#')) continue;
    const m = line.match(/^([A-Za-z_][\w-]*):\s?(.*)$/);
    if (!m) continue;
    let value = m[2];
    // A key with no inline value may be a YAML block sequence. Absorb the
    // following "  - item" lines into the inline form, quotes intact, so
    // scalar()/inlineList() and the raw wikilink checks all see one shape.
    if (value.trim() === '') {
      const items = [];
      while (i + 1 < fm.length && /^\s+-\s+/.test(fm[i + 1])) {
        items.push(fm[++i].replace(/^\s+-\s+/, '').trim());
      }
      if (items.length) value = `[${items.join(', ')}]`;
    }
    data[m[1]] = value;
  }
  return data;
}

function unquote(s) {
  s = s.trim();
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    return s.slice(1, -1);
  }
  return s;
}

function scalar(raw) {
  return raw === undefined ? '' : unquote(raw);
}

function inlineList(raw) {
  if (raw === undefined) return [];
  let v = raw.trim();
  if (v === '' || v === '[]') return [];
  if (v.startsWith('[') && v.endsWith(']')) v = v.slice(1, -1);
  if (!v.trim()) return [];
  return v.split(',').map((s) => unquote(s)).filter(Boolean);
}

function isISODate(v) {
  if (!ISO.test(v)) return false;
  const [y, m, d] = v.split('-').map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === m - 1 && dt.getUTCDate() === d;
}

function lev(a, b) {
  const m = a.length, n = b.length;
  const row = Array.from({ length: n + 1 }, (_, i) => i);
  for (let i = 1; i <= m; i++) {
    let prev = row[0]; row[0] = i;
    for (let j = 1; j <= n; j++) {
      const tmp = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = tmp;
    }
  }
  return row[n];
}

function closest(word, list) {
  let best = null, bestD = Infinity;
  for (const w of list) {
    const d = lev(word, w);
    if (d < bestD) { bestD = d; best = w; }
  }
  return bestD <= 3 ? best : null;
}

function loadRegistry(indexText) {
  const lines = indexText.split(/\r?\n/);
  const i = lines.findIndex((l) => /^##\s+Tag registry/i.test(l));
  if (i === -1) return null;
  for (let j = i + 1; j < lines.length; j++) {
    const l = lines[j].trim();
    if (!l) continue;
    if (l.startsWith('#')) break;
    // registry line: a ·-separated list of `backtick` tokens, no prose ('=' marks the gloss)
    if (l.includes('`') && l.includes('·') && !l.includes('=')) {
      return [...l.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
    }
  }
  return null;
}

function relSlug(entry) {
  const m = entry.match(/^\[\[([^\]|]+)(?:\|[^\]]*)?\]\]$/);
  return m ? m[1].trim() : null;
}

// Exported for the unit tests (lint.test.mjs); the CLI does not use these directly.
export { walk, parseFrontmatter, unquote, scalar, inlineList, isISODate, closest, loadRegistry, relSlug };

// ---------- run ----------

export function runLint(root = DEFAULT_ROOT) {
  const VAULT = join(root, 'vault');
  const INDEX = join(VAULT, 'index.md');

  if (!existsSync(VAULT) || !existsSync(INDEX)) {
    throw new LintError(`could not find the vault at ${relative(process.cwd(), VAULT)} (run from the wikilens repo).`);
  }

  const indexText = readFileSync(INDEX, 'utf8');
  const registry = loadRegistry(indexText);
  const indexLinks = new Set(
    [...indexText.matchAll(/\[\[([^\]|]+)(?:\|[^\]]*)?\]\]/g)].map((m) => m[1].trim()),
  );

  const files = walk(VAULT);
  const existingSlugs = new Set(files.map((f) => basename(f, '.md')));

  const issues = []; // { file, level: 'ERROR'|'WARN', msg }
  const push = (file, level, msg) => issues.push({ file, level, msg });

  if (!registry) {
    push(relative(root, INDEX), 'ERROR', 'could not find the tag registry (## Tag registry) — every tag will be rejected');
  }

  for (const path of files) {
    const rel = relative(root, path).split('\\').join('/');
    const name = basename(path);
    const text = readFileSync(path, 'utf8');
    const fm = parseFrontmatter(text);

    if (!fm) {
      push(rel, 'ERROR', 'no YAML frontmatter block');
      continue;
    }

    // required fields
    for (const k of REQUIRED) {
      if (scalar(fm[k]) === '') push(rel, 'ERROR', `missing required field: ${k}`);
    }

    const type = scalar(fm.type);
    const status = scalar(fm.status);
    const created = scalar(fm.created);
    const updated = scalar(fm.updated);
    const tags = inlineList(fm.tags);
    const related = inlineList(fm.related);
    const commitRaw = fm.commit === undefined ? '' : fm.commit.trim();

    // vocab
    if (type && !TYPES.includes(type)) push(rel, 'ERROR', `type "${type}" not in {${TYPES.join(', ')}}`);
    if (status && !STATUSES.includes(status)) {
      const hint = closest(status, STATUSES);
      push(rel, 'ERROR', `status "${status}" not in {${STATUSES.join(', ')}}${hint ? ` — did you mean "${hint}"?` : ''}`);
    }

    // dates
    if (created && !isISODate(created)) push(rel, 'ERROR', `created "${created}" is not a valid ISO date (YYYY-MM-DD)`);
    if (updated && !isISODate(updated)) push(rel, 'ERROR', `updated "${updated}" is not a valid ISO date (YYYY-MM-DD)`);
    if (isISODate(created) && isISODate(updated) && updated < created) {
      push(rel, 'ERROR', `updated (${updated}) is before created (${created})`);
    }

    // filename
    if (!EXEMPT.has(name)) {
      if (!FILENAME.test(name)) {
        push(rel, 'ERROR', `filename must be YYYY-MM-DD_<kebab-slug>.md`);
      } else if (created && name.slice(0, 10) !== created) {
        push(rel, 'ERROR', `filename date (${name.slice(0, 10)}) doesn't match created (${created})`);
      }
    }

    // tags
    if (registry) {
      for (const t of tags) {
        if (!registry.includes(t)) {
          const hint = closest(t, registry);
          push(rel, 'ERROR', `tag "${t}" not in the registry${hint ? ` — did you mean "${hint}"?` : ''} (add it to index.md first)`);
        }
      }
    }

    // related: unquoted wikilinks (raw) + resolvable targets
    const relatedRaw = fm.related ?? '';
    for (const m of relatedRaw.matchAll(/\[\[[^\]]*\]\]/g)) {
      const before = relatedRaw[m.index - 1];
      const after = relatedRaw[m.index + m[0].length];
      const quoted = (before === '"' && after === '"') || (before === "'" && after === "'");
      if (!quoted) push(rel, 'ERROR', `wikilink ${m[0]} in related: must be quoted ("${m[0]}")`);
    }
    for (const entry of related) {
      const slug = relSlug(entry);
      if (!slug) push(rel, 'ERROR', `related entry "${entry}" is not a [[wikilink]]`);
      else if (!existingSlugs.has(slug)) push(rel, 'ERROR', `related [[${slug}]] has no matching file in the vault`);
    }

    // commit
    if (commitRaw && !commitRaw.startsWith('[') && /^\d+$/.test(commitRaw)) {
      push(rel, 'WARN', `commit ${commitRaw} is all digits — quote it ("${commitRaw}") so YAML keeps it a string`);
    }

    // "done means…"
    if (status === 'done') {
      if (!commitRaw) push(rel, 'WARN', 'status is done but commit is empty (set it if this doc tracks code)');
      if (tags.length === 0 && related.length === 0) push(rel, 'WARN', 'status is done but has no tags and no related links');
    }

    // index drift
    if (!EXEMPT.has(name) && !rel.includes('/archive/')) {
      const slug = basename(name, '.md');
      if (!indexLinks.has(slug)) push(rel, 'WARN', `not referenced anywhere in index.md ([[${slug}]])`);
    }
  }

  return {
    files: files.length,
    issues,
    errors: issues.filter((i) => i.level === 'ERROR').length,
    warns: issues.filter((i) => i.level === 'WARN').length,
  };
}

// ---------- report ----------

export function printReport({ files, issues, errors, warns }, log = console.log) {
  log(`vault-lint: ${files} docs checked — ${errors} error${errors === 1 ? '' : 's'}, ${warns} warning${warns === 1 ? '' : 's'}\n`);

  if (issues.length) {
    const byFile = new Map();
    for (const it of issues) {
      if (!byFile.has(it.file)) byFile.set(it.file, []);
      byFile.get(it.file).push(it);
    }
    for (const [file, list] of byFile) {
      log(file);
      for (const it of list) log(`  ${it.level.padEnd(5)}  ${it.msg}`);
      log('');
    }
  }

  log(errors ? `✖ ${errors} error${errors === 1 ? '' : 's'}` : '✔ clean');
}

// ---------- cli ----------

// Windows-aware main-guard (mirrors sync-agents/generate.mjs): side effects —
// printing and process.exit — happen only when run directly, never on import.
const argvPath = process.argv[1] ? resolve(process.argv[1]) : '';
const selfPath = fileURLToPath(import.meta.url);
const isMain =
  argvPath && (process.platform === 'win32' ? argvPath.toLowerCase() === selfPath.toLowerCase() : argvPath === selfPath);

if (isMain) {
  try {
    const result = runLint();
    printReport(result);
    process.exit(result.errors ? 1 : 0);
  } catch (err) {
    if (err instanceof LintError) {
      console.error(`vault-lint: ${err.message}`);
      process.exit(2);
    }
    throw err;
  }
}
