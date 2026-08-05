#!/usr/bin/env node
// bump — one-command release version bump: rewrite the version in the three
// manifests (package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json),
// then refresh both lockfiles (npm install --package-lock-only + cargo check).
//
// Zero-dependency. Resolves the repo relative to this file, so it runs from any cwd:
//   npm run bump 0.2.0        (= node .claude/skills/bump/bump.mjs 0.2.0)
//
// Fail-loud: a malformed version, manifests that disagree on the current
// version, or a rewrite target that doesn't look exactly as expected all
// abort before anything is written. A failed lock refresh restores the
// manifests, so a fixed-environment retry starts clean. Versions bump only
// in release PRs (CLAUDE.md §3). Exit codes: 0 = success, 1 = any failure.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = resolve(__dirname, '..', '..', '..'); // .claude/skills/bump -> repo root

// Plain major.minor.patch, no leading zeros — release-scoped SemVer here never
// uses a v prefix or prerelease/build suffixes, and Tauri's Windows bundlers
// want the bare triple.
const VERSION_RE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

class BumpError extends Error {}
const fail = (message) => {
  throw new BumpError(message);
};
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const readText = (path, rel) => {
  if (!existsSync(path)) fail(`${rel}: not found — is this a WikiLens checkout?`);
  return readFileSync(path, 'utf8');
};

// ---------- version locators ----------
// Each returns the exact character span of the version string, so the rewrite
// is a byte splice — never JSON.parse/re-serialize, which would flatten each
// file's newline style (package.json is CRLF; the src-tauri manifests are
// pinned eol=lf — see vault/2026-07-15_cargo-manifest-eol-churn.md).

// Top level in the 2-space-indented JSON manifests is exactly two spaces, so
// nested "version" keys (lockfile package entries, plugin configs) can't match.
function locateJsonVersion(text, rel) {
  const matches = [...text.matchAll(/^(  "version": ")(\d+\.\d+\.\d+)(?=")/gm)];
  if (matches.length !== 1) {
    fail(`${rel}: expected exactly one top-level "version" line, found ${matches.length}`);
  }
  const m = matches[0];
  const start = m.index + m[1].length;
  return { current: m[2], start, end: start + m[2].length };
}

function packageSection(text, rel) {
  const header = /^\[package\]\r?\n/m.exec(text);
  if (!header) fail(`${rel}: no [package] section`);
  const bodyStart = header.index + header[0].length;
  const nextSection = /^\[/m.exec(text.slice(bodyStart));
  const bodyEnd = nextSection ? bodyStart + nextSection.index : text.length;
  return { start: bodyStart, section: text.slice(bodyStart, bodyEnd) };
}

// Scoped to [package] — Cargo.toml carries a dozen dependency `version =`
// pins in other sections, so a global match would be a footgun.
function locateCargoVersion(text, rel) {
  const { start, section } = packageSection(text, rel);
  const matches = [...section.matchAll(/^(version = ")(\d+\.\d+\.\d+)(?=")/gm)];
  if (matches.length !== 1) {
    fail(`${rel}: expected exactly one version key under [package], found ${matches.length}`);
  }
  const m = matches[0];
  const at = start + m.index + m[1].length;
  return { current: m[2], start: at, end: at + m[2].length };
}

function locateCargoName(text, rel) {
  const { section } = packageSection(text, rel);
  const m = /^name = "([^"]+)"/m.exec(section);
  if (!m) fail(`${rel}: no name key under [package]`);
  return m[1];
}

// The app crate's [[package]] entry. Dependency mentions are quoted strings
// inside `dependencies = [...]` arrays, never a line-start `name =`, so the
// entry is unambiguous.
function locateCargoLockVersion(text, crate, rel) {
  const re = new RegExp(`^name = "${escapeRe(crate)}"\\r?\\n(version = ")(\\d+\\.\\d+\\.\\d+)(?=")`, 'gm');
  const matches = [...text.matchAll(re)];
  if (matches.length !== 1) {
    fail(`${rel}: expected exactly one [[package]] entry for "${crate}", found ${matches.length}`);
  }
  const m = matches[0];
  const start = m.index + m[0].length - m[2].length;
  return { current: m[2], start, end: start + m[2].length };
}

// ---------- lock refresh ----------

// Injectable so unit tests never spawn npm/cargo. stdio inherits so the
// tools' own progress stays visible; only the exit status comes back.
// One command string through the shell: npm is npm.cmd on Windows (which
// spawnSync won't run without a shell), and the args-array + shell:true
// combination is deprecated (DEP0190). Safe: both command lines are
// constants, and the bump version never reaches a subprocess argv (npm
// reads package.json, cargo reads Cargo.toml).
function execCommand(tool, args, cwd) {
  const { status, error } = spawnSync([tool, ...args].join(' '), {
    cwd,
    stdio: 'inherit',
    shell: true,
  });
  return { status: error ? 1 : (status ?? 1), error: error?.message };
}

// ---------- run ----------

const MANIFESTS = [
  { rel: 'package.json', locate: locateJsonVersion },
  { rel: 'src-tauri/Cargo.toml', locate: locateCargoVersion },
  { rel: 'src-tauri/tauri.conf.json', locate: locateJsonVersion },
];

export function runBump(next, { root = DEFAULT_ROOT, exec = execCommand } = {}) {
  if (typeof next !== 'string' || !next) fail('usage: npm run bump <X.Y.Z>');
  if (!VERSION_RE.test(next)) {
    fail(`"${next}" is not a plain X.Y.Z version (no v prefix, no prerelease/build suffix)`);
  }

  // Pre-flight: read and locate everything before touching anything.
  const manifests = MANIFESTS.map(({ rel, locate }) => {
    const path = join(root, ...rel.split('/'));
    const text = readText(path, rel);
    return { rel, path, text, ...locate(text, rel) };
  });

  if (new Set(manifests.map((m) => m.current)).size > 1) {
    const listing = manifests.map((m) => `${m.rel} = ${m.current}`).join(', ');
    fail(`the manifests disagree on the current version (${listing}) — fix them by hand before bumping`);
  }
  const previous = manifests[0].current;
  if (previous === next) fail(`the manifests are already at ${next}`);

  const crate = locateCargoName(manifests[1].text, manifests[1].rel);
  const locks = [
    {
      rel: 'package-lock.json',
      tool: 'npm',
      args: ['install', '--package-lock-only'],
      cwdRel: '.',
      command: 'npm install --package-lock-only',
      locate: (text) => locateJsonVersion(text, 'package-lock.json'),
    },
    {
      rel: 'src-tauri/Cargo.lock',
      tool: 'cargo',
      args: ['check'],
      cwdRel: 'src-tauri',
      command: 'cargo check',
      locate: (text) => locateCargoLockVersion(text, crate, 'src-tauri/Cargo.lock'),
    },
  ].map((lock) => {
    const path = join(root, ...lock.rel.split('/'));
    return { ...lock, path, from: lock.locate(readText(path, lock.rel)).current };
  });

  // Rewrite the manifests (byte splice — see the locator note above).
  for (const m of manifests) {
    writeFileSync(m.path, m.text.slice(0, m.start) + next + m.text.slice(m.end));
  }
  const restore = () => {
    for (const m of manifests) writeFileSync(m.path, m.text);
  };

  const report = {
    previous,
    next,
    manifests: manifests.map((m) => ({ path: m.rel, from: m.current, to: next })),
    locks: [],
  };

  for (const lock of locks) {
    const { status, error } = exec(lock.tool, lock.args, join(root, lock.cwdRel)) ?? {};
    if (error || status !== 0) {
      // Likely environmental (offline npm, missing cargo) — restore so a
      // fixed-environment retry isn't blocked by the same-version gate.
      restore();
      const what = error ? `failed to start: ${error}` : `exited with status ${status}`;
      fail(
        `\`${lock.command}\` ${what} — the three manifests are restored to ${previous}; check \`git status\` for a half-refreshed lock`,
      );
    }
    // Verify the refresh actually landed — an exit-0 run that leaves the old
    // version behind is surprising state, so keep the evidence (no restore).
    const landed = lock.locate(readText(lock.path, lock.rel)).current;
    if (landed !== next) {
      fail(`${lock.rel} is still at ${landed} after \`${lock.command}\` — investigate before retrying (the manifests stay bumped)`);
    }
    report.locks.push({ path: lock.rel, command: lock.command, from: lock.from, to: next });
  }

  return report;
}

// ---------- report ----------

export function printReport(report, log = console.log) {
  const rows = [
    ...report.manifests.map((m) => [m.path, `${m.from} → ${m.to}`]),
    ...report.locks.map((l) => [l.path, `${l.from} → ${l.to}  (${l.command})`]),
  ];
  const width = Math.max(...rows.map(([path]) => path.length));
  for (const [path, change] of rows) log(`${path.padEnd(width + 2)}${change}`);
  log(
    `bump: ${report.previous} → ${report.next} — ${report.manifests.length} manifests rewritten, ${report.locks.length} locks refreshed`,
  );
  log('next: run /check, then open the release PR (CLAUDE.md §3)');
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
    if (process.argv.length > 3) fail('expected exactly one argument — usage: npm run bump <X.Y.Z>');
    printReport(runBump(process.argv[2]));
  } catch (err) {
    if (err instanceof BumpError) {
      console.error(`bump: ${err.message}`);
      process.exit(1);
    }
    throw err;
  }
}

export { BumpError, locateJsonVersion, locateCargoVersion, locateCargoLockVersion };
