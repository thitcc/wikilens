// Port of `src-tauri/src/wiki/titles.rs` — the typo→title fallback (last rung
// of the zero-hit ladder): a per-game page-title index (`list=allpages`, capped)
// fuzzy-matched with Jaro-Winkler. `titles.test.mjs` pins the port against the
// crate's unit vectors.

import { UA, sleep } from './lib.mjs';

export const MAX_TITLES = 6000; // titles.rs MAX_TITLES
export const MAX_REQUESTS = 20; // titles.rs MAX_REQUESTS
export const PAGE_LIMIT = 500; // titles.rs PAGE_LIMIT
export const ALLPAGES_TIMEOUT_MS = 8000; // titles.rs ALLPAGES_TIMEOUT
export const ALLPAGES_BUDGET_MS = 15000; // titles.rs ALLPAGES_BUDGET
export const MATCH_THRESHOLD = 0.92; // titles.rs MATCH_THRESHOLD
export const LENGTH_RATIO_FLOOR = 0.6; // titles.rs LENGTH_RATIO_FLOOR

export class TitleIndex {
  constructor(titles, stripNs) { this.titles = titles; this.stripNs = stripNs; }

  // titles.rs TitleIndex::best_match
  bestMatch(query) {
    const q = normalize(query);
    if (q === '') return null;
    const qlen = [...q].length;
    let best = null;
    for (const title of this.titles) {
      const stripped = this.stripNs ? stripNamespace(title) : title;
      const cand = normalize(stripped);
      const clen = [...cand].length;
      if (clen === 0) continue;
      const [short, long] = qlen <= clen ? [qlen, clen] : [clen, qlen];
      if (short / long < LENGTH_RATIO_FLOOR) continue;
      const score = jaroWinkler(q, cand);
      if (score >= MATCH_THRESHOLD && (best === null || score > best.score)) best = { score, title };
    }
    return best ? best.title : null;
  }
}

// titles.rs fetch_all_titles — sequential, bounded allpages walk.
export async function fetchAllTitles(wiki, pace = 0) {
  const namespace = (wiki.search_namespace ?? '0').split('|')[0] || '0';
  const titles = [];
  let apcontinue = null;
  const start = Date.now();
  for (let i = 0; i < MAX_REQUESTS; i++) {
    if (titles.length >= MAX_TITLES || Date.now() - start >= ALLPAGES_BUDGET_MS) break;
    const params = new URLSearchParams({ action: 'query', list: 'allpages', apnamespace: namespace, aplimit: String(PAGE_LIMIT), format: 'json' });
    if (apcontinue !== null) params.set('apcontinue', apcontinue);
    const resp = await fetch(`${wiki.api_url}?${params}`, { headers: { 'User-Agent': UA }, signal: AbortSignal.timeout(ALLPAGES_TIMEOUT_MS) });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const { titles: page, apcontinue: next } = parseAllpages(await resp.text());
    titles.push(...page);
    if (next === null) break;
    apcontinue = next;
    if (pace) await sleep(pace);
  }
  return titles.slice(0, MAX_TITLES);
}

// titles.rs parse_allpages
export function parseAllpages(body) {
  const json = JSON.parse(body);
  const pages = json?.query?.allpages;
  if (!Array.isArray(pages)) throw new Error('missing `query.allpages` array');
  const titles = pages.map((p) => p?.title).filter((t) => typeof t === 'string');
  const cont = json?.continue?.apcontinue;
  return { titles, apcontinue: typeof cont === 'string' ? cont : null };
}

// titles.rs normalize — lowercase, collapse non-alphanumerics to single spaces.
// Rust `is_alphanumeric` = Unicode Alphabetic ∪ Numeric ⇒ \p{Alphabetic}|\p{N}.
export function normalize(s) {
  let out = '';
  let prevSpace = false;
  for (const c of s) {
    if (/[\p{Alphabetic}\p{N}]/u.test(c)) {
      out += c.toLowerCase();
      prevSpace = false;
    } else if (!prevSpace && out !== '') {
      out += ' ';
      prevSpace = true;
    }
  }
  return out.trim();
}

// titles.rs strip_namespace
export function stripNamespace(title) {
  const i = title.indexOf(':');
  if (i === -1) return title;
  const rest = title.slice(i + 1);
  return rest === '' ? title : rest;
}

// titles.rs jaro_winkler
export function jaroWinkler(a, b) {
  const j = jaro(a, b);
  if (j === 0) return 0;
  const ca = [...a]; const cb = [...b];
  let prefix = 0;
  for (let i = 0; i < 4 && i < ca.length && i < cb.length; i++) { if (ca[i] === cb[i]) prefix++; else break; }
  return j + prefix * 0.1 * (1 - j);
}

// titles.rs jaro
export function jaro(sa, sb) {
  const a = [...sa]; const b = [...sb];
  const alen = a.length; const blen = b.length;
  if (alen === 0 && blen === 0) return 1;
  if (alen === 0 || blen === 0) return 0;
  const matchDist = Math.max(0, Math.floor(Math.max(alen, blen) / 2) - 1);
  const aM = new Array(alen).fill(false); const bM = new Array(blen).fill(false);
  let matches = 0;
  for (let i = 0; i < alen; i++) {
    const lo = Math.max(0, i - matchDist); const hi = Math.min(i + matchDist + 1, blen);
    for (let j = lo; j < hi; j++) {
      if (!bM[j] && a[i] === b[j]) { aM[i] = true; bM[j] = true; matches++; break; }
    }
  }
  if (matches === 0) return 0;
  let transpositions = 0; let k = 0;
  for (let i = 0; i < alen; i++) {
    if (aM[i]) {
      while (!bM[k]) k++;
      if (a[i] !== b[k]) transpositions++;
      k++;
    }
  }
  const m = matches; const t = transpositions / 2;
  return (m / alen + m / blen + (m - t) / m) / 3;
}
