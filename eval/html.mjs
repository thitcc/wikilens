// Port of `src-tauri/src/wiki/html.rs` — rendered-HTML → plaintext reducer.
// Every function mirrors the Rust item of the same (snake-cased) name;
// `html.test.mjs` pins the port against the crate's insta snapshots, so a
// divergence fails `npm run test:node` instead of skewing the eval's context.

const DROP_TAGS = ['script', 'style', 'svg'];
const DROP_IDS = ['toc', 'navbox'];
const DROP_CLASSES = [
  'mw-editsection',
  'table-progress-checkbox-cell',
  'pi-image',
  'printfooter',
  'catlinks',
];
const BLOCK_TAGS = [
  'p', 'div', 'section', 'aside', 'li', 'ul', 'ol', 'dl', 'dt', 'dd', 'h1',
  'h2', 'h3', 'h4', 'h5', 'h6', 'table', 'caption', 'figcaption',
  'blockquote', 'center', 'pre',
];
const VOID_TAGS = [
  'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link',
  'meta', 'param', 'source', 'track', 'wbr',
];

/** Reduce rendered MediaWiki HTML to plaintext, keeping table/infobox data. */
export function toPlaintext(html) {
  const st = { out: '', stack: [], dropping: 0, afterCell: false };
  let rest = html;
  for (;;) {
    const pos = rest.indexOf('<');
    if (pos === -1) {
      if (st.dropping === 0) st.out += pushText(rest);
      break;
    }
    if (st.dropping === 0) st.out += pushText(rest.slice(0, pos));
    rest = rest.slice(pos);

    if (rest.startsWith('<!--')) {
      const comment = rest.slice(4);
      const end = comment.indexOf('-->');
      if (end === -1) break; // unterminated comment: nothing more to keep
      rest = comment.slice(end + 3);
      continue;
    }

    const end = findTagEnd(rest);
    if (end === null) break; // unterminated tag: drop the remainder
    const tagSrc = rest.slice(1, end);
    rest = rest.slice(end + 1);

    if (tagSrc.startsWith('/')) {
      closeTag(tagSrc.slice(1).trim().toLowerCase(), st);
    } else {
      openTag(tagSrc, st);
    }
  }
  return tidy(decodeEntities(st.out));
}

// Source-formatting newlines/tabs become spaces; line breaks come only from tags.
function pushText(text) {
  return text.replace(/[\n\r\t]/g, ' ');
}

// Index of the tag-closing `>`, skipping over quoted attribute values.
function findTagEnd(rest) {
  let quote = null;
  for (let i = 1; i < rest.length; i++) {
    const c = rest[i];
    if (quote === null) {
      if (c === '>') return i;
      if (c === '"' || c === "'") quote = c;
    } else if (c === quote) {
      quote = null;
    }
  }
  return null;
}

function openTag(tagSrc, st) {
  const m = tagSrc.search(/[\s/]/u);
  const nameEnd = m === -1 ? tagSrc.length : m;
  const name = tagSrc.slice(0, nameEnd).toLowerCase();
  if (name === '') return;
  const attrs = tagSrc.slice(nameEnd);
  const selfClosing = tagSrc.trimEnd().endsWith('/');

  if (VOID_TAGS.includes(name)) {
    if (st.dropping === 0 && (name === 'br' || name === 'hr')) st.out += '\n';
    return;
  }

  const id = attrValue(attrs, 'id') ?? '';
  const cls = attrValue(attrs, 'class') ?? '';
  const tokens = cls.split(/\s+/u).filter(Boolean);
  const drop = DROP_TAGS.includes(name)
    || DROP_IDS.includes(id)
    || tokens.some((t) => t.includes('navbox') || DROP_CLASSES.includes(t));
  const piLabel = tokens.some((t) => t === 'pi-data-label');

  if (!drop && st.dropping === 0) {
    if (name === 'tr') {
      st.afterCell = false;
    } else if ((name === 'td' || name === 'th') && st.afterCell) {
      st.out += ' | ';
      st.afterCell = false;
    }
  }

  if (selfClosing) return; // no subtree, nothing to track
  if (drop) st.dropping += 1;
  st.stack.push({ tag: name, drop, piLabel });
}

function closeTag(name, st) {
  // Pop to the matching open tag, tolerating minor nesting slop.
  while (st.stack.length > 0) {
    const top = st.stack.pop();
    if (top.drop) st.dropping = Math.max(0, st.dropping - 1);
    const matched = top.tag === name;
    if (matched && !top.drop && st.dropping === 0) {
      if (top.piLabel) {
        // `Label` + upcoming value → "Label: Value" on one line.
        st.out = st.out.replace(/ +$/, '') + ': ';
      } else if (top.tag === 'td' || top.tag === 'th') {
        st.afterCell = true;
      } else if (top.tag === 'tr') {
        st.out += '\n';
        st.afterCell = false;
      } else if (BLOCK_TAGS.includes(top.tag)) {
        st.out += '\n';
      }
    }
    if (matched) return;
  }
}

// Extract a double- or single-quoted attribute value (e.g. `id`, `class`).
function attrValue(attrs, name) {
  let rest = attrs;
  for (;;) {
    const pos = rest.indexOf(name);
    if (pos === -1) return null;
    const beforeOk = pos === 0 || /\s/u.test(rest[pos - 1]);
    const after = rest.slice(pos + name.length);
    if (beforeOk) {
      const afterEq = after.trimStart();
      if (afterEq.startsWith('=')) {
        const v = afterEq.slice(1).trimStart();
        for (const q of ['"', "'"]) {
          if (v.startsWith(q)) {
            const body = v.slice(1);
            const end = body.indexOf(q);
            return end === -1 ? null : body.slice(0, end);
          }
        }
      }
    }
    rest = rest.slice(pos + name.length);
  }
}

// The five named XML entities, `&nbsp;`, and numeric forms (NBSP → space).
function decodeEntities(s) {
  let out = '';
  let rest = s;
  for (;;) {
    const pos = rest.indexOf('&');
    if (pos === -1) break;
    out += rest.slice(0, pos);
    rest = rest.slice(pos);
    const semi = rest.indexOf(';');
    if (semi === -1 || semi > 10) {
      out += '&';
      rest = rest.slice(1);
      continue;
    }
    const entity = rest.slice(1, semi);
    let decoded = null;
    if (entity.startsWith('#')) {
      const num = entity.slice(1);
      let cp = null;
      if (/^[xX]/.test(num)) {
        const hex = num.slice(1);
        if (/^[0-9a-fA-F]+$/.test(hex)) cp = parseInt(hex, 16);
      } else if (/^[0-9]+$/.test(num)) {
        cp = parseInt(num, 10);
      }
      if (cp !== null && cp <= 0x10ffff && !(cp >= 0xd800 && cp <= 0xdfff)) {
        decoded = cp === 0xa0 ? ' ' : String.fromCodePoint(cp);
      }
    } else {
      decoded = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' }[entity] ?? null;
    }
    if (decoded !== null) {
      out += decoded;
      rest = rest.slice(semi + 1);
    } else {
      out += '&';
      rest = rest.slice(1);
    }
  }
  return out + rest;
}

// Collapse whitespace per line, dropping empty and separator-only lines.
function tidy(raw) {
  const lines = [];
  for (const line of raw.split('\n')) {
    const collapsed = line.replace(/\r$/, '').split(/\s+/u).filter(Boolean).join(' ');
    if (collapsed === '' || /^[| ]*$/.test(collapsed)) continue;
    lines.push(collapsed);
  }
  return lines.join('\n');
}
