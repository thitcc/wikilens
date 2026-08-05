#!/usr/bin/env node
// sync-agents — generate local Codex skill adapters from the committed Claude
// workflow sources (.claude/commands/*.md and .claude/skills/*/SKILL.md).
//
// Zero-dependency. Resolves the repo relative to this file, so it runs from any cwd:
//   node .claude/skills/sync-agents/generate.mjs
//
// `.claude/` stays the only hand-edited workflow source; adapters land in the
// gitignored .agents/skills/ and are never committed. Deterministic and
// fail-loud: sources are read in sorted order, only allowlisted frontmatter is
// understood, and anything surprising — a root AGENTS.md, unknown metadata, an
// undeclared name collision, an unowned file at an output path, a broken
// .claude/ engine reference, an untranslated Claude-ism — aborts the run
// instead of guessing. Exit codes: 0 = success (including "up to date"),
// 1 = any failure.

import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = resolve(__dirname, '..', '..', '..'); // .claude/skills/sync-agents -> repo root

// A name shared by a command and a source skill must be declared here with the
// kind that wins; any undeclared collision aborts the run.
const DECLARED_COLLISIONS = { 'vault-lint': 'skill' };

// Frontmatter allowlists. Claude-only fields are stripped from adapters (and
// reported); a key outside its allowlist aborts — new metadata must be handled
// deliberately, never dropped in silence. Codex's sandbox and approval policy
// are the real permission boundary, so dropping allowed-tools loses nothing.
const COMMAND_KEYS = new Set(['description', 'allowed-tools', 'argument-hint']);
const SKILL_KEYS = new Set(['name', 'description', 'allowed-tools']);
const STRIPPED_KEYS = ['allowed-tools', 'argument-hint'];

const ARGUMENTS_PROSE = 'the text accompanying the skill mention';
const DEFAULT_PROJECT_DOC_MAX_BYTES = 32 * 1024; // Codex's default when unset
const SIZE_WARN_RATIO = 0.875;
const NAME_RE = /^[a-z0-9][a-z0-9-]*$/;

class SyncError extends Error {}
const fail = (message) => {
  throw new SyncError(message);
};
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const alternation = (names) =>
  [...names].sort((a, b) => b.length - a.length).map(escapeRe).join('|');

// The size warning must track the cap the repo actually configures — a hardcoded
// copy of Codex's default stops matching the moment .codex/config.toml raises it,
// and a warning that never fires is worse than none. Zero-dep on purpose: a TOML
// parser buys nothing for one integer. Commented-out keys can't match (line-start
// anchored), and anything unparseable falls back to Codex's own default.
function readProjectDocMaxBytes(configText) {
  const m = /^[ \t]*project_doc_max_bytes[ \t]*=[ \t]*(\d[\d_]*)/m.exec(configText);
  if (!m) return DEFAULT_PROJECT_DOC_MAX_BYTES;
  const value = Number(m[1].replace(/_/g, ''));
  return Number.isSafeInteger(value) && value > 0 ? value : DEFAULT_PROJECT_DOC_MAX_BYTES;
}

function parseFrontmatter(raw, label) {
  const text = raw.replace(/\r\n/g, '\n');
  if (!text.startsWith('---\n')) fail(`${label}: missing frontmatter block`);
  const end = text.indexOf('\n---\n', 3);
  if (end === -1) fail(`${label}: unterminated frontmatter block`);
  const fields = {};
  for (const line of text.slice(4, end).split('\n')) {
    if (!line.trim()) continue;
    const m = /^([A-Za-z][A-Za-z0-9-]*):\s*(.*)$/.exec(line);
    if (!m) {
      fail(
        `${label}: unsupported frontmatter line ${JSON.stringify(line)} — the generator only understands single-line "key: value" fields`,
      );
    }
    if (m[1] in fields) fail(`${label}: duplicate frontmatter key "${m[1]}"`);
    fields[m[1]] = m[2].trim();
  }
  return { fields, body: text.slice(end + 5) };
}

function collectSources(root) {
  const sources = [];
  const commandsDir = join(root, '.claude', 'commands');
  if (existsSync(commandsDir)) {
    for (const entry of readdirSync(commandsDir).sort()) {
      if (!entry.endsWith('.md')) continue;
      const path = join(commandsDir, entry);
      if (!statSync(path).isFile()) continue;
      const label = `.claude/commands/${entry}`;
      const { fields, body } = parseFrontmatter(readFileSync(path, 'utf8'), label);
      for (const key of Object.keys(fields)) {
        if (!COMMAND_KEYS.has(key)) {
          fail(`${label}: unknown frontmatter key "${key}" — extend the generator's allowlist deliberately if this is a new Claude field`);
        }
      }
      if (!fields.description) fail(`${label}: a command needs a non-empty description to become a Codex skill`);
      const name = entry.slice(0, -3);
      if (!NAME_RE.test(name)) fail(`${label}: command filename must be a kebab-case skill name`);
      sources.push({
        name,
        kind: 'command',
        label,
        description: fields.description,
        stripped: STRIPPED_KEYS.filter((k) => k in fields),
        body,
      });
    }
  }
  const skillsDir = join(root, '.claude', 'skills');
  if (existsSync(skillsDir)) {
    for (const entry of readdirSync(skillsDir).sort()) {
      const path = join(skillsDir, entry, 'SKILL.md');
      if (!existsSync(path)) continue; // not a skill directory; Claude ignores it too
      const label = `.claude/skills/${entry}/SKILL.md`;
      const { fields, body } = parseFrontmatter(readFileSync(path, 'utf8'), label);
      for (const key of Object.keys(fields)) {
        if (!SKILL_KEYS.has(key)) {
          fail(`${label}: unknown frontmatter key "${key}" — extend the generator's allowlist deliberately if this is a new Claude field`);
        }
      }
      if (!fields.name) fail(`${label}: a source skill needs a name`);
      if (fields.name !== entry) fail(`${label}: frontmatter name "${fields.name}" must match its directory "${entry}"`);
      if (!NAME_RE.test(fields.name)) fail(`${label}: skill name must be kebab-case`);
      if (!fields.description) fail(`${label}: a source skill needs a non-empty description`);
      sources.push({
        name: fields.name,
        kind: 'skill',
        label,
        description: fields.description,
        stripped: STRIPPED_KEYS.filter((k) => k in fields),
        body,
      });
    }
  }
  return sources;
}

function resolveCollisions(sources) {
  const byName = new Map();
  const collisions = [];
  for (const source of sources) {
    const existing = byName.get(source.name);
    if (!existing) {
      byName.set(source.name, source);
      continue;
    }
    const winnerKind = DECLARED_COLLISIONS[source.name];
    if (!winnerKind || existing.kind === source.kind) {
      fail(`undeclared name collision: "${source.name}" is both ${existing.label} and ${source.label} — declare a winner in DECLARED_COLLISIONS or rename one`);
    }
    const winner = existing.kind === winnerKind ? existing : source;
    const loser = winner === existing ? source : existing;
    byName.set(source.name, winner);
    collisions.push({ name: source.name, winner: winner.label, loser: loser.label });
  }
  const winners = [...byName.values()].sort((a, b) => (a.name < b.name ? -1 : 1));
  return { winners, collisions };
}

function transformBody(body, names) {
  // Quoted or bare argument placeholder -> prose. Claude /name workflow
  // references -> Codex $name mentions; the lookarounds keep path segments
  // (".claude/commands/check.md") and dotted relatives ("./check") untouched.
  let out = body.replace(/"\$ARGUMENTS"|\$ARGUMENTS\b/g, ARGUMENTS_PROSE);
  out = out.replace(new RegExp(`(?<![\\w/.])/(${alternation(names)})(?![\\w./-])`, 'g'), '$$$1');
  return out;
}

function checkEmitted(name, content, sourceText, names, root) {
  if (content.includes('.Codex')) {
    fail(`adapter "${name}": emitted content contains a corrupted ".Codex" path`);
  }
  if (content.includes('$ARGUMENTS')) {
    fail(`adapter "${name}": a leftover $ARGUMENTS placeholder survived translation`);
  }
  // Looser than the transform on purpose: anything it finds is a known
  // workflow name the transform deliberately skipped (e.g. "./check") —
  // ambiguous enough to need a human, so abort.
  const detector = new RegExp(`(?<![\\w/])/(${alternation(names)})(?![\\w-])`);
  const leftover = detector.exec(content);
  if (leftover) {
    fail(`adapter "${name}": unresolved Claude command reference "${leftover[0]}" — reword the source so the reference is unambiguous`);
  }
  // Only the hand-written source text is scanned for engine paths — the
  // generated-by banner references this generator itself, not a source engine.
  for (const match of sourceText.matchAll(/\.claude\/[A-Za-z0-9._/-]+/g)) {
    const ref = match[0].replace(/[.,;:]+$/, '');
    if (!existsSync(join(root, ...ref.split('/')))) {
      fail(`adapter "${name}": references ${ref}, which does not exist — adapters must point at committed engines`);
    }
  }
}

// Owned entries are the only paths the generator ever deletes, so they must
// match the exact shape it writes — anything else (traversal segments,
// backslashes, foreign files) aborts before any destructive step.
const OWNED_ENTRY_RE = /^[a-z0-9][a-z0-9-]*\/SKILL\.md$/;

function loadManifest(path) {
  if (!existsSync(path)) return { raw: null, owned: [] };
  const raw = readFileSync(path, 'utf8');
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    parsed = null;
  }
  if (
    !parsed ||
    parsed.version !== 1 ||
    !Array.isArray(parsed.owned) ||
    !parsed.owned.every((o) => typeof o === 'string' && OWNED_ENTRY_RE.test(o))
  ) {
    fail('corrupt .agents/skills/.sync-manifest.json (owned entries must be "<kebab-name>/SKILL.md") — delete .agents/skills/ entirely and re-run to regenerate from scratch');
  }
  return { raw, owned: parsed.owned };
}

export function runSync(root = DEFAULT_ROOT) {
  for (const shadow of ['AGENTS.override.md', 'AGENTS.md']) {
    if (existsSync(join(root, shadow))) {
      fail(`root ${shadow} found — it would shadow CLAUDE.md in Codex's instruction discovery; CLAUDE.md is canonical, so remove the file (see CLAUDE.md §4)`);
    }
  }

  const sources = collectSources(root);
  if (!sources.length) fail('no workflow sources found under .claude/commands or .claude/skills');
  const { winners, collisions } = resolveCollisions(sources);
  const names = winners.map((w) => w.name);

  const report = {
    generated: [],
    updated: [],
    unchanged: [],
    removed: [],
    collisions,
    stripped: [],
    warnings: [],
    upToDate: false,
  };

  const codexConfig = join(root, '.codex', 'config.toml');
  const codexConfigText = existsSync(codexConfig) ? readFileSync(codexConfig, 'utf8') : '';
  const maxBytes = readProjectDocMaxBytes(codexConfigText);

  const claudeMd = join(root, 'CLAUDE.md');
  if (!existsSync(claudeMd)) {
    report.warnings.push('CLAUDE.md not found — Codex has no project instructions to fall back to');
  } else {
    const size = statSync(claudeMd).size;
    if (size >= Math.floor(maxBytes * SIZE_WARN_RATIO)) {
      report.warnings.push(`CLAUDE.md is ${size} bytes — approaching Codex's ${maxBytes}-byte project_doc_max_bytes; the cap applies to the combined instruction chain (global + project docs), so truncation can start even earlier`);
    }
  }
  if (!codexConfigText.includes('project_doc_fallback_filenames')) {
    report.warnings.push('.codex/config.toml does not point Codex at CLAUDE.md (project_doc_fallback_filenames) — Codex will load no project instructions');
  }

  const skillsRoot = join(root, '.agents', 'skills');
  const manifestPath = join(skillsRoot, '.sync-manifest.json');
  const manifest = loadManifest(manifestPath);
  const ownedSet = new Set(manifest.owned);

  const desired = new Map(); // manifest-relative path -> emitted content
  for (const source of winners) {
    if (source.stripped.length) report.stripped.push({ name: source.name, fields: source.stripped });
    const body = transformBody(source.body, names).replace(/^\n+/, '');
    const emitted = `---\nname: ${source.name}\ndescription: ${source.description}\n---\n\n<!-- Generated from ${source.label} by sync-agents — do not edit; regenerate with: node .claude/skills/sync-agents/generate.mjs -->\n\n${body}`;
    checkEmitted(source.name, emitted, `${source.description}\n${body}`, names, root);
    desired.set(`${source.name}/SKILL.md`, emitted);
  }

  for (const [relPath, content] of desired) {
    const target = join(skillsRoot, ...relPath.split('/'));
    const name = relPath.split('/')[0];
    if (existsSync(target)) {
      if (readFileSync(target, 'utf8') === content) {
        report.unchanged.push(name);
        continue;
      }
      if (!ownedSet.has(relPath)) {
        fail(`refusing to overwrite .agents/skills/${relPath}: it differs from the generated adapter and is not owned by sync-agents — move it aside or delete it, then re-run`);
      }
      writeFileSync(target, content);
      report.updated.push(name);
    } else {
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
      report.generated.push(name);
    }
  }

  for (const relPath of manifest.owned) {
    if (desired.has(relPath)) continue;
    const target = join(skillsRoot, ...relPath.split('/'));
    if (!existsSync(target)) continue;
    rmSync(target);
    try {
      rmdirSync(dirname(target));
    } catch {
      // leave non-empty directories alone
    }
    report.removed.push(relPath.split('/')[0]);
  }

  const nextManifest = `${JSON.stringify({ version: 1, owned: [...desired.keys()].sort() }, null, 2)}\n`;
  const manifestChanged = manifest.raw !== nextManifest;
  if (manifestChanged) {
    mkdirSync(skillsRoot, { recursive: true });
    writeFileSync(manifestPath, nextManifest);
  }

  report.upToDate =
    !report.generated.length && !report.updated.length && !report.removed.length && !manifestChanged;
  return report;
}

export function printReport(report, log = console.log) {
  for (const c of report.collisions) log(`collision (declared): ${c.name} — ${c.winner} wins over ${c.loser}`);
  for (const s of report.stripped) log(`stripped Claude-only metadata from ${s.name}: ${s.fields.join(', ')}`);
  for (const w of report.warnings) log(`warning: ${w}`);
  for (const [verb, list] of [
    ['generated', report.generated],
    ['updated', report.updated],
    ['removed', report.removed],
  ]) {
    if (list.length) log(`${verb}: ${list.join(', ')}`);
  }
  const total = report.generated.length + report.updated.length + report.unchanged.length;
  log(
    report.upToDate
      ? `sync-agents: up to date (${total} adapters)`
      : `sync-agents: ${report.generated.length} generated, ${report.updated.length} updated, ${report.removed.length} removed (${total} adapters)`,
  );
}

const argvPath = process.argv[1] ? resolve(process.argv[1]) : '';
const selfPath = fileURLToPath(import.meta.url);
const isMain =
  argvPath && (process.platform === 'win32' ? argvPath.toLowerCase() === selfPath.toLowerCase() : argvPath === selfPath);
if (isMain) {
  try {
    printReport(runSync());
  } catch (err) {
    if (err instanceof SyncError) {
      console.error(`sync-agents: ERROR — ${err.message}`);
      process.exit(1);
    }
    throw err;
  }
}
