//! Best-effort MediaWiki wikitext → plaintext conversion.
//!
//! WikiLens reads page content as raw wikitext (`prop=revisions`) and cleans it
//! here — this works on every MediaWiki wiki, including the many game wikis that
//! lack the TextExtracts extension (Core Keeper, Stardew). This is a pragmatic
//! cleaner, not a full parser: templates and tables are stripped (their rendered
//! data usually lives in a backend the raw wikitext doesn't contain anyway),
//! leaving the article prose readable for the LLM.

/// Convert wikitext to readable plaintext.
pub fn to_plaintext(wikitext: &str) -> String {
    let s = remove_html_comments(wikitext);
    let s = remove_magic_words(&s); // __NOTOC__, __NOEDITSECTION__, …
    let s = remove_balanced(&s, "{{", "}}"); // templates (nesting-aware)
    let s = remove_balanced(&s, "{|", "|}"); // tables (nesting-aware)
    let s = convert_wiki_links(&s); // [[a|b]] -> b, drop File:/Category:
    let s = convert_external_links(&s); // [url text] -> text
    let s = strip_formatting(&s); // bold/italic, headings, list markers, HTML tags
    let s = decode_entities(&s);
    collapse_blank_lines(&s)
}

/// Behavior-switch "magic words" that appear as literal `__WORD__` in wikitext.
const MAGIC_WORDS: &[&str] = &[
    "__NOTOC__",
    "__FORCETOC__",
    "__TOC__",
    "__NOEDITSECTION__",
    "__NEWSECTIONLINK__",
    "__NONEWSECTIONLINK__",
    "__NOGALLERY__",
    "__HIDDENCAT__",
    "__EXPECTUNUSEDCATEGORY__",
    "__INDEX__",
    "__NOINDEX__",
    "__STATICREDIRECT__",
    "__NOWYSIWYG__",
    "__DISAMBIG__",
];

fn remove_magic_words(input: &str) -> String {
    let mut s = input.to_string();
    for word in MAGIC_WORDS {
        if s.contains(word) {
            s = s.replace(word, "");
        }
    }
    s
}

/// Remove `<!-- ... -->` comments (may span lines).
fn remove_html_comments(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => return out, // unterminated comment: drop the remainder
        }
    }
    out.push_str(rest);
    out
}

/// Remove every balanced `open..close` region, honoring nesting. `open`/`close`
/// must be ASCII (they are: `{{`/`}}`, `{|`/`|}`), so byte scanning is safe even
/// through multi-byte UTF-8 text.
fn remove_balanced(input: &str, open: &str, close: &str) -> String {
    let bytes = input.as_bytes();
    let (ob, cb) = (open.as_bytes(), close.as_bytes());
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut depth = 0usize;
    while i < bytes.len() {
        if bytes[i..].starts_with(ob) {
            depth += 1;
            i += ob.len();
        } else if depth > 0 && bytes[i..].starts_with(cb) {
            depth -= 1;
            i += cb.len();
        } else {
            if depth == 0 {
                out.push(bytes[i]);
            }
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Convert `[[target|display]]` → `display` and `[[target]]` → `target`; drop
/// `File:`/`Image:`/`Category:` links entirely. Assumes non-nested links (media
/// captions, the main source of nesting, are dropped anyway).
fn convert_wiki_links(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("]]") {
            Some(end) => {
                out.push_str(&render_link_inner(&after[..end]));
                rest = &after[end + 2..];
            }
            None => {
                // No closing brackets; keep the text as-is and stop.
                out.push_str("[[");
                rest = after;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn render_link_inner(inner: &str) -> String {
    let lower = inner.to_ascii_lowercase();
    if lower.starts_with("file:")
        || lower.starts_with("image:")
        || lower.starts_with("category:")
        || lower.starts_with(":category:")
    {
        return String::new();
    }
    // The display text is whatever follows the last `|` (or the whole thing).
    match inner.rsplit_once('|') {
        Some((_, display)) => display.to_string(),
        None => inner.to_string(),
    }
}

/// Convert `[https://url display text]` → `display text` and `[https://url]` → ``.
/// Runs after wiki-link conversion, so any remaining `[...]` is an external link
/// or a literal bracket (kept as-is).
fn convert_external_links(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if after.starts_with("http://") || after.starts_with("https://") {
            if let Some(end) = after.find(']') {
                let inner = &after[..end];
                let display = inner
                    .split_once(char::is_whitespace)
                    .map(|(_, d)| d.trim())
                    .unwrap_or("");
                out.push_str(display);
                rest = &after[end + 1..];
                continue;
            }
        }
        out.push('[');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Strip bold/italic markers, HTML tags, heading `=` fences, and leading list /
/// indent markers, line by line.
fn strip_formatting(input: &str) -> String {
    let no_marks = input.replace("'''''", "").replace("'''", "").replace("''", "");
    let no_tags = remove_html_tags(&no_marks);

    let mut lines = Vec::new();
    for line in no_tags.lines() {
        let trimmed = line.trim();
        let cleaned = if trimmed.len() >= 2 && trimmed.starts_with('=') && trimmed.ends_with('=') {
            trimmed.trim_matches('=').trim().to_string()
        } else {
            trimmed
                .trim_start_matches(['*', '#', ':', ';'])
                .trim_start()
                .to_string()
        };
        lines.push(cleaned);
    }
    lines.join("\n")
}

/// Known HTML/wiki tag names to strip (their text content is kept). Anything
/// else in angle brackets — command placeholders like `<item>`, comparisons like
/// `a < b` — is left as literal text.
const HTML_TAGS: &[&str] = &[
    "br", "hr", "p", "div", "span", "b", "i", "u", "s", "strong", "em", "sup",
    "sub", "small", "big", "code", "pre", "tt", "kbd", "var", "samp", "abbr",
    "mark", "del", "ins", "wbr", "ref", "references", "nowiki", "noinclude",
    "includeonly", "onlyinclude", "gallery", "poem", "center", "blockquote",
    "font", "table", "tr", "td", "th", "thead", "tbody", "caption", "ul", "ol",
    "li", "dl", "dt", "dd",
];

/// Strip only recognized `<tag ...>`, `</tag>`, `<tag/>` HTML/wiki tags, keeping
/// their text content. Unknown `<...>` (e.g. `<item>`, `a < b`) is left literal.
fn remove_html_tags(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let name_source = after.strip_prefix('/').unwrap_or(after);
        let name: String = name_source
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();

        if !name.is_empty() && HTML_TAGS.contains(&name.as_str()) {
            if let Some(rel_end) = rest[start..].find('>') {
                out.push_str(&rest[..start]);
                rest = &rest[start + rel_end + 1..];
                continue;
            }
        }
        // Not a recognized tag (or no closing '>'): keep '<' literally, advance.
        out.push_str(&rest[..start + 1]);
        rest = &rest[start + 1..];
    }
    out.push_str(rest);
    out
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Trim trailing spaces and collapse runs of blank lines to a single blank line.
fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::new();
    let mut blank_run = 0;
    for line in input.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_nested_templates() {
        assert_eq!(remove_balanced("a{{b{{c}}d}}e", "{{", "}}"), "ae");
        assert_eq!(remove_balanced("keep {{drop}} keep", "{{", "}}"), "keep  keep");
    }

    #[test]
    fn converts_and_drops_links() {
        assert_eq!(convert_wiki_links("[[Root Seed]]s"), "Root Seeds");
        assert_eq!(convert_wiki_links("see [[Page|the page]] now"), "see the page now");
        assert_eq!(convert_wiki_links("x [[File:a.png|thumb|cap]] y"), "x  y");
        assert_eq!(convert_wiki_links("z [[Category:Items]] z"), "z  z");
    }

    #[test]
    fn converts_external_links() {
        assert_eq!(convert_external_links("[https://x.com click here]"), "click here");
        assert_eq!(convert_external_links("bare [https://x.com]"), "bare ");
        assert_eq!(convert_external_links("keep [brackets]"), "keep [brackets]");
    }

    #[test]
    fn strips_headings_and_marks() {
        assert_eq!(strip_formatting("== Obtaining =="), "Obtaining");
        assert_eq!(strip_formatting("'''Wood''' is ''useful''"), "Wood is useful");
        assert_eq!(strip_formatting("* item one"), "item one");
    }

    #[test]
    fn full_cleanup_of_real_wikitext() {
        let wikitext = "{{Object infobox\n| auto = Wood\n}}\n\n\
'''Wood''' is a [[crafting material]] found naturally in the [[Undergrounds]], \
[[Clay Caves]] and [[Azeos' Wilderness]]. It can also be grown using [[Root Seed]]s.\n\n\
== Obtaining ==\n{{Obtaining}}\n\n== See also ==\n* {{Item|Coral Wood}}";
        let text = to_plaintext(wikitext);
        assert!(
            text.contains("Wood is a crafting material found naturally in the Undergrounds"),
            "unexpected prose: {text:?}"
        );
        assert!(text.contains("grown using Root Seeds."), "unexpected prose: {text:?}");
        assert!(text.contains("Obtaining"));
        assert!(!text.contains("{{"), "templates leaked: {text:?}");
        assert!(!text.contains("[["), "links leaked: {text:?}");
        assert!(!text.contains("'''"), "bold markers leaked: {text:?}");
        assert!(!text.contains("infobox"), "infobox leaked: {text:?}");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("a &amp; b &nbsp;c"), "a & b  c");
    }

    #[test]
    fn removes_magic_words() {
        assert_eq!(remove_magic_words("__NOTOC__Hello __NOEDITSECTION__world"), "Hello world");
        assert_eq!(to_plaintext("__NOTOC__ Real content here."), "Real content here.");
    }

    #[test]
    fn strips_known_tags_but_keeps_placeholders() {
        // Known tags are removed, content kept.
        assert_eq!(remove_html_tags("<br>text</span>"), "text");
        assert_eq!(
            remove_html_tags("<span title=\"note\">Steel</span> <big>bar</big>"),
            "Steel bar"
        );
        // Unknown angle-bracket content is preserved (command syntax, comparisons).
        assert_eq!(remove_html_tags("Type /give <item> <amount>"), "Type /give <item> <amount>");
        assert_eq!(remove_html_tags("if level < 5 and hp > 0"), "if level < 5 and hp > 0");
    }
}
