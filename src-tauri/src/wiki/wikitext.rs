//! Best-effort MediaWiki wikitext → plaintext conversion — the **fallback**
//! cleaner.
//!
//! The primary path reads rendered HTML (`fetch.rs` → `html.rs`); pages whose
//! parse call fails or times out arrive here as raw wikitext (`prop=revisions`,
//! one batched request), which every MediaWiki wiki serves, including the many
//! game wikis that lack the TextExtracts extension (Core Keeper, Stardew). This
//! is a pragmatic cleaner, not a full parser: templates and tables are stripped
//! (their rendered data lives in a backend the raw wikitext doesn't contain
//! anyway), leaving the article prose readable for the LLM.

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
///
/// Unbalanced markers degrade gracefully instead of eating the page: an
/// unmatched `open` is dropped and everything after it kept (inner balanced
/// regions still removed), and a stray `close` at depth zero stays literal —
/// unlike `convert_wiki_links`, which keeps an unmatched `[[` as-is, dropping
/// the bare marker reads cleaner in prose handed to the LLM.
fn remove_balanced(input: &str, open: &str, close: &str) -> String {
    let bytes = input.as_bytes();
    let (ob, cb) = (open.as_bytes(), close.as_bytes());
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    // `out.len()` at each still-unmatched `open`: emit everything as we go, and
    // truncate back to the mark when its `close` arrives. Marks left at the end
    // are unmatched opens — their content already survived in `out`.
    let mut marks: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(ob) {
            marks.push(out.len());
            i += ob.len();
        } else if !marks.is_empty() && bytes[i..].starts_with(cb) {
            out.truncate(marks.pop().expect("guarded by !marks.is_empty()"));
            i += cb.len();
        } else {
            out.push(bytes[i]);
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
    fn unclosed_open_keeps_the_rest_of_the_page() {
        // The bug this pins: an unclosed {{ or {| used to silently discard
        // everything after it, so the page cleaned to empty and was dropped.
        for wikitext in [
            "intro {{Infobox\n|name=X\n rest of the article",
            "intro {|\n|-\n| cell\n rest of the article",
        ] {
            let text = to_plaintext(wikitext);
            assert!(text.contains("intro"), "prefix lost: {text:?}");
            assert!(text.contains("rest of the article"), "tail lost: {text:?}");
        }
    }

    #[test]
    fn unmatched_opens_drop_the_marker_but_keep_content() {
        // Inner balanced regions are still removed under an unmatched outer.
        assert_eq!(remove_balanced("x{{a{{b}}c", "{{", "}}"), "xac");
        assert_eq!(remove_balanced("x{{y{{z", "{{", "}}"), "xyz");
    }

    #[test]
    fn stray_close_markers_stay_literal() {
        assert_eq!(remove_balanced("a}}b", "{{", "}}"), "a}}b");
        assert_eq!(remove_balanced("a|}b", "{|", "|}"), "a|}b");
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

    // Freezes the fallback cleaner's exact output over a real captured
    // wikitext revision (see the matching snapshot suite in html.rs).
    #[test]
    fn snapshot_raw_wikitext_fallback_page() {
        insta::assert_snapshot!(
            "raw_wikitext_stardew_wood",
            to_plaintext(include_str!("fixtures/raw_wikitext.txt"))
        );
    }
}

#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_support::{arbitrary_text, marker_soup};

    const WIKITEXT_MARKERS: &[&str] = &[
        "{{", "}}", "{|", "|}", "[[", "]]", "<!--", "-->", "''", "'''", "&#", "&amp;",
        "__NOTOC__", "[File:", "[[Category:", "== ", "[https://", "]", "<br>", "<item>",
        "<", ">", "|", "=", "*", ":", "\n",
    ];

    /// Marker-free prose for building balanced constructs: excludes every byte
    /// that participates in a marker, so removals can never join fragments
    /// into a new marker (replace-with-empty passes make a universal
    /// no-residue claim over arbitrary input provably false — `{''{` cleans
    /// to `{{`).
    fn prose() -> impl Strategy<Value = String> {
        "[A-Za-z0-9 .,\\n-]{0,20}"
    }

    /// Well-formed wikitext: prose and non-nested links, recursively wrapped
    /// in balanced templates and tables. Links never nest inside links —
    /// `convert_wiki_links` is non-nesting-aware by design.
    fn balanced_wikitext() -> impl Strategy<Value = String> {
        let link = (prose(), proptest::option::of(prose())).prop_map(|(t, d)| match d {
            Some(d) => format!("[[{t}|{d}]]"),
            None => format!("[[{t}]]"),
        });
        let leaf = prop_oneof![prose(), link];
        leaf.prop_recursive(3, 24, 3, |inner| {
            let seq = proptest::collection::vec(inner, 0..4).prop_map(|v| v.concat());
            prop_oneof![
                seq.clone().prop_map(|b| format!("{{{{Tpl|{b}}}}}")),
                seq.clone().prop_map(|b| format!("{{|\n|-\n| {b}\n|}}")),
                seq,
            ]
        })
    }

    proptest! {
        #[test]
        fn never_panics_and_never_grows_on_arbitrary_text(s in arbitrary_text()) {
            let out = to_plaintext(&s);
            prop_assert!(out.len() <= s.len(), "grew: {} -> {}", s.len(), out.len());
        }

        #[test]
        fn never_panics_and_never_grows_on_marker_soup(s in marker_soup(WIKITEXT_MARKERS)) {
            let out = to_plaintext(&s);
            prop_assert!(out.len() <= s.len(), "grew: {} -> {}", s.len(), out.len());
        }

        #[test]
        fn balanced_wikitext_leaves_no_marker_residue(s in balanced_wikitext()) {
            let out = to_plaintext(&s);
            for marker in ["{{", "}}", "{|", "|}", "[[", "]]"] {
                prop_assert!(!out.contains(marker), "{marker} leaked from {s:?}: {out:?}");
            }
        }

        #[test]
        fn unmatched_template_open_keeps_prefix_and_tail(
            a in "[^{}]{0,30}",
            b in "[^{}]{0,30}",
        ) {
            let cleaned = remove_balanced(&format!("{a}{{{{{b}"), "{{", "}}");
            prop_assert_eq!(cleaned, format!("{a}{b}"));
        }

        #[test]
        fn unmatched_table_open_keeps_prefix_and_tail(
            a in "[^{|}]{0,30}",
            b in "[^{|}]{0,30}",
        ) {
            let cleaned = remove_balanced(&format!("{a}{{|{b}"), "{|", "|}");
            prop_assert_eq!(cleaned, format!("{a}{b}"));
        }
    }
}
