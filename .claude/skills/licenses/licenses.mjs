#!/usr/bin/env node
// licenses — regenerate THIRD-PARTY-LICENSES.txt: the license text of every
// Rust crate linked into wikilens.exe (cargo-about, x86_64-pc-windows-msvc,
// normal dependencies only — src-tauri/about.toml) and of every production
// npm package bundled into the frontend (a walk of package-lock.json +
// node_modules), as one plain-text file that ships next to the exe
// (tauri.conf.json `bundle.resources`).
//
// Zero-dependency. Resolves the repo relative to this file, so it runs from any cwd:
//   npm run licenses              regenerate (writes only when the content changed)
//   npm run licenses -- --check   regenerate in memory; exit 1 if the committed file is stale (CI's gate)
//
// Deterministic by construction, so the CI check is a byte compare: the crate
// graph is Cargo.lock + the configured target (host-independent), the npm
// walk is sorted here with a code-point comparator (node_modules readdir
// order differs per filesystem), every text is normalized to LF, and the
// header carries no date, version or machine path. The app crate is excluded
// (Cargo.toml `publish = false` + about.toml `private.ignore`) so a release
// bump never stales the file — the release PR stays bump-only.
//
// Fail-loud: a missing/old cargo-about, a crate under an unaccepted license
// (cargo-about's own error), a production package without a license text,
// or the app crate leaking into the inventory all abort before anything is
// written. Exit codes: 0 = success, 1 = any failure.

import { existsSync, readdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = resolve(__dirname, '..', '..', '..'); // .claude/skills/licenses -> repo root

export const OUTPUT_REL = 'THIRD-PARTY-LICENSES.txt';
export const CARGO_ABOUT_VERSION = '0.9';
export const CARGO_ABOUT_INSTALL = `cargo install cargo-about --locked --version ${CARGO_ABOUT_VERSION}.2 --features cli`;

/** The app crate: its presence in the inventory means the exclusion broke. */
const APP_CRATE = 'wikilens';

/** cargo-about scans `src/` too, and a source file carrying a license header
 * can outrank the real LICENSE file (encoding_rs and schemars_derive did). A
 * "license text" with Rust items in it is that bug, never a license. */
const LOOKS_LIKE_SOURCE_RE = /^\s*(#!\[|pub(\(crate\))? (fn|static|const|struct|enum|mod|use|trait) |use [a-z_:]+;|impl[ <]|fn [a-z_]+\()/m;

/** Production packages whose license file the readdir rule can't find, mapped
 * to the file to use (relative to the repo root). plugin-opener ships only a
 * `LICENSE.spdx` document; its holder and terms are @tauri-apps/api's. */
export const NPM_LICENSE_OVERRIDES = {
  '@tauri-apps/plugin-opener': 'node_modules/@tauri-apps/api/LICENSE_MIT',
};

export class LicensesError extends Error {}
const fail = (message) => {
  throw new LicensesError(message);
};

/** Code-point order — `localeCompare` is locale- and ICU-build-dependent. */
const byCodePoint = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
const compareBy = (...keys) => (a, b) => {
  for (const key of keys) {
    const c = byCodePoint(key(a), key(b));
    if (c !== 0) return c;
  }
  return 0;
};

/** LF only, no trailing whitespace — the two ecosystems ship both endings. */
export const normalizeText = (text) => text.replace(/\r\n?/g, '\n').replace(/[ \t]+$/gm, '').replace(/\n+$/, '');

const readText = (path, rel) => {
  if (!existsSync(path)) fail(`${rel}: not found — is this a WikiLens checkout?`);
  return readFileSync(path, 'utf8');
};

// ---------- subprocess ----------

// Injectable so unit tests never spawn cargo. stdout is captured (the version
// probe reads it); stderr inherits so cargo-about's own errors (an unaccepted
// license names the crate) stay visible. One command string through the shell (see
// bump.mjs: npm/cargo shims need it on Windows, and the args are constants
// plus a quoted temp path).
function execCommand(tool, args, cwd) {
  const { status, stdout, error } = spawnSync([tool, ...args].join(' '), {
    cwd,
    encoding: 'utf8',
    shell: true,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  return { status: error ? 1 : (status ?? 1), stdout: stdout ?? '', error: error?.message };
}

export function checkTool(exec, cwd) {
  const { status, stdout, error } = exec('cargo', ['about', '--version'], cwd) ?? {};
  const version = (stdout ?? '').trim();
  if (error || status !== 0 || !version.startsWith('cargo-about ')) {
    fail(`cargo-about is not installed (\`cargo about --version\` ${error ? `failed to start: ${error}` : `exited with status ${status}`}) — install it once: ${CARGO_ABOUT_INSTALL}`);
  }
  if (!version.startsWith(`cargo-about ${CARGO_ABOUT_VERSION}.`)) {
    fail(`${version} found, but the inventory is generated with ${CARGO_ABOUT_VERSION}.x (canonical texts differ across releases) — ${CARGO_ABOUT_INSTALL}`);
  }
  return version;
}

// ---------- Rust: cargo-about ----------

/** Run cargo-about and reduce its JSON to what the file prints: one entry per
 * distinct (license, exact text), each with the crates under it. */
export function collectRust(root, exec) {
  const cwd = join(root, 'src-tauri');
  const out = join(tmpdir(), `wikilens-cargo-about-${process.pid}.json`);
  const args = ['about', 'generate', '--format', 'json', '--locked', '--fail', '-o', `"${out}"`];
  try {
    const { status, error } = exec('cargo', args, cwd) ?? {};
    if (error || status !== 0) {
      fail(`\`cargo about generate\` ${error ? `failed to start: ${error}` : `exited with status ${status}`} — see its error above: an unaccepted license, a stale clarify checksum, or a config error in src-tauri/about.toml`);
    }
    if (!existsSync(out)) fail('`cargo about generate` exited 0 but wrote no output');
    return reduceCargoAbout(JSON.parse(readFileSync(out, 'utf8')));
  } finally {
    if (existsSync(out)) unlinkSync(out);
  }
}

export function reduceCargoAbout(json) {
  const licenses = json?.licenses;
  if (!Array.isArray(licenses) || licenses.length === 0) fail('cargo-about reported no licenses — empty crate graph?');
  const entries = licenses.map((l) => ({
    id: String(l.id ?? ''),
    name: String(l.name ?? l.id ?? ''),
    text: normalizeText(String(l.text ?? '')),
    usedBy: (l.used_by ?? [])
      .map((u) => ({
        name: String(u.crate?.name ?? ''),
        version: String(u.crate?.version ?? ''),
        repository: u.crate?.repository ? String(u.crate.repository) : '',
      }))
      .sort(compareBy((c) => c.name, (c) => c.version)),
  }));
  for (const e of entries) {
    if (!e.id || !e.text) fail(`cargo-about entry "${e.name || e.id}" has no id or text`);
    if (LOOKS_LIKE_SOURCE_RE.test(e.text)) {
      fail(`cargo-about picked a source file as the ${e.id} text for ${e.usedBy.map((c) => c.name).join(', ')} — pin the real file with a \`[<name>.clarify]\` table in src-tauri/about.toml`);
    }
    if (e.usedBy.some((c) => c.name === APP_CRATE)) {
      fail(`the app crate "${APP_CRATE}" leaked into the inventory — keep \`publish = false\` in src-tauri/Cargo.toml and \`private = { ignore = true }\` in src-tauri/about.toml`);
    }
  }
  return entries.sort(compareBy((e) => e.id, (e) => e.text));
}

// ---------- npm: package-lock walk ----------

const LICENSE_FILE_RE = /^(licen[cs]e|copying)([._-].*)?$/i;
const NOT_A_LICENSE_TEXT_RE = /\.(js|cjs|mjs|ts|sh|ps1|spdx|json|yml|yaml)$/i;

/** The license file shipped in a package dir, or null. Prefers the MIT
 * variant when a package ships one file per alternative (the same
 * first-accepted-alternative rule the Rust side applies). */
export function findLicenseFile(dir) {
  const candidates = readdirSync(dir)
    .filter((f) => LICENSE_FILE_RE.test(f) && !NOT_A_LICENSE_TEXT_RE.test(f))
    .sort(byCodePoint);
  if (candidates.length === 0) return null;
  return candidates.find((f) => /mit/i.test(f)) ?? candidates[0];
}

/** Production packages from package-lock.json (v2/v3 `packages` map): every
 * entry not reachable only through dev dependencies. Grouped by exact text. */
export function collectNpm(root, { warn = (m) => console.error(m) } = {}) {
  const lockRel = 'package-lock.json';
  const lock = JSON.parse(readText(join(root, lockRel), lockRel));
  if (!(lock.lockfileVersion >= 2) || !lock.packages) fail(`${lockRel}: lockfileVersion 2 or 3 with a "packages" map expected`);

  const byText = new Map();
  for (const [key, entry] of Object.entries(lock.packages)) {
    if (key === '' || entry.link || entry.dev || entry.devOptional) continue;
    const name = entry.name ?? key.slice(key.lastIndexOf('node_modules/') + 'node_modules/'.length);
    const version = String(entry.version ?? '');
    const dir = join(root, ...key.split('/'));
    if (!existsSync(dir)) {
      if (entry.optional) {
        warn(`licenses: ${name}@${version} is optional and not installed — skipped`);
        continue;
      }
      fail(`${key} is missing — run npm install first`);
    }
    let file;
    if (Object.hasOwn(NPM_LICENSE_OVERRIDES, name)) {
      file = join(root, ...NPM_LICENSE_OVERRIDES[name].split('/'));
      if (!existsSync(file)) fail(`${name}: override ${NPM_LICENSE_OVERRIDES[name]} not found`);
    } else {
      const found = findLicenseFile(dir);
      if (!found) fail(`${name}@${version} (${key}) ships no license text — add it to NPM_LICENSE_OVERRIDES with a reviewed file`);
      file = join(dir, found);
    }
    const text = normalizeText(readFileSync(file, 'utf8'));
    if (!text) fail(`${name}@${version}: license file is empty (${file})`);
    const id = String(entry.license ?? readPackageLicense(dir) ?? 'UNKNOWN');
    const group = byText.get(text) ?? { ids: new Set(), text, usedBy: [] };
    group.ids.add(id);
    group.usedBy.push({ name, version });
    byText.set(text, group);
  }
  if (byText.size === 0) fail('no production npm packages found — run npm install first');

  return [...byText.values()]
    .map((g) => ({
      id: [...g.ids].sort(byCodePoint).join(' / '),
      text: g.text,
      usedBy: g.usedBy.sort(compareBy((p) => p.name, (p) => p.version)),
    }))
    .sort(compareBy((e) => e.id, (e) => e.text));
}

function readPackageLicense(dir) {
  try {
    const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
    return typeof pkg.license === 'string' ? pkg.license : pkg.license?.type;
  } catch {
    return undefined;
  }
}

// ---------- render ----------

const RULE = '-'.repeat(80);

const componentKey = (u) => `${u.name}@${u.version}`;

/** Distinct components across entries — a crate listed under two texts (an
 * `AND` expression, or two MIT variants) is still one crate. */
export const distinctComponents = (entries) => new Set(entries.flatMap((e) => e.usedBy.map(componentKey))).size;

function countByIdLine(entries) {
  const byId = new Map();
  for (const e of entries) {
    const set = byId.get(e.id) ?? new Set();
    for (const u of e.usedBy) set.add(componentKey(u));
    byId.set(e.id, set);
  }
  return [...byId.entries()]
    .sort((a, b) => b[1].size - a[1].size || byCodePoint(a[0], b[0]))
    .map(([id, set]) => `${id} (${set.size})`)
    .join(', ');
}

function renderEntry(e, describe) {
  const lines = [RULE, e.name && e.name !== e.id ? `${e.name} (${e.id})` : e.id, 'Used by:'];
  for (const u of e.usedBy) lines.push(`  - ${describe(u)}`);
  lines.push('', e.text, '');
  return lines.join('\n');
}

export function render({ rust, npm }) {
  const crates = distinctComponents(rust);
  const packages = distinctComponents(npm);
  const head = [
    'WikiLens — third-party licenses',
    '===============================',
    '',
    'WikiLens is MIT-licensed (LICENSE in the source repository,',
    'https://github.com/thitcc/wikilens). It is built from the open-source',
    'components below, each under its own license, reproduced here as their',
    'authors require. This file is generated by `npm run licenses` from the',
    "project's two lockfiles and ships next to WikiLens.exe — edit the",
    'generator (.claude/skills/licenses/licenses.mjs), not this file.',
    '',
    `Part 1 — Rust crates linked into wikilens.exe (x86_64-pc-windows-msvc,`,
    `normal dependencies only): ${crates} crates under ${rust.length} license texts.`,
    `  ${countByIdLine(rust)}`,
    '',
    `Part 2 — npm packages bundled into the frontend (production dependencies`,
    `only): ${packages} packages under ${npm.length} license texts.`,
    `  ${countByIdLine(npm)}`,
    '',
    'Components under the Mozilla Public License 2.0 are used unmodified;',
    'their source is available at the repository listed with each one and on',
    'crates.io / npmjs.com.',
    '',
    '',
    'PART 1 — RUST CRATES',
    '====================',
    '',
  ];
  const part1 = rust.map((e) => renderEntry(e, (u) => `${u.name} ${u.version}${u.repository ? ` — ${u.repository}` : ''}`));
  const part2 = npm.map((e) => renderEntry(e, (u) => `${u.name}@${u.version}`));
  // Every entry already ends with one newline (renderEntry), so the join
  // leaves a blank line between entries and exactly one newline at the end.
  return [...head, ...part1, '', 'PART 2 — NPM PACKAGES', '=====================', '', ...part2].join('\n');
}

// ---------- run ----------

export function runLicenses({ root = DEFAULT_ROOT, check = false, exec = execCommand, warn } = {}) {
  checkTool(exec, join(root, 'src-tauri'));
  const rust = collectRust(root, exec);
  const npm = collectNpm(root, { warn });
  const content = render({ rust, npm });
  const path = join(root, OUTPUT_REL);
  const existing = existsSync(path) ? readFileSync(path, 'utf8') : null;
  const counts = {
    crates: distinctComponents(rust),
    packages: distinctComponents(npm),
    kb: Math.round(Buffer.byteLength(content, 'utf8') / 1024),
  };
  if (existing === content) return { status: 'unchanged', counts };
  if (check) return { status: 'stale', counts, firstDiffLine: firstDifferingLine(existing ?? '', content) };
  writeFileSync(path, content);
  return { status: existing === null ? 'created' : 'updated', counts };
}

function firstDifferingLine(a, b) {
  const la = a.split('\n');
  const lb = b.split('\n');
  const n = Math.max(la.length, lb.length);
  for (let i = 0; i < n; i++) if (la[i] !== lb[i]) return i + 1;
  return 0;
}

// ---------- cli ----------

// Windows-aware main-guard (mirrors bump.mjs): side effects — printing and
// process.exit — happen only when run directly, never on import.
const argvPath = process.argv[1] ? resolve(process.argv[1]) : '';
const selfPath = fileURLToPath(import.meta.url);
const isMain =
  argvPath && (process.platform === 'win32' ? argvPath.toLowerCase() === selfPath.toLowerCase() : argvPath === selfPath);
if (isMain) {
  try {
    const flags = process.argv.slice(2);
    const check = flags.includes('--check');
    const unknown = flags.filter((f) => f !== '--check');
    if (unknown.length) fail(`unknown argument ${unknown.join(' ')} — usage: npm run licenses [-- --check]`);
    const { status, counts, firstDiffLine } = runLicenses({ check });
    const summary = `${counts.crates} crates, ${counts.packages} packages, ${counts.kb} KB`;
    if (status === 'stale') {
      fail(`${OUTPUT_REL} is stale (first difference at line ${firstDiffLine}; regenerated: ${summary}) — run \`npm run licenses\` and commit the result`);
    }
    console.log(`licenses: ${OUTPUT_REL} ${status} (${summary})`);
  } catch (err) {
    if (err instanceof LicensesError) {
      console.error(`licenses: ${err.message}`);
      process.exit(1);
    }
    throw err;
  }
}
