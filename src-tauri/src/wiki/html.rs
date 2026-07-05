//! Rendered-HTML → plaintext reducer.
//!
//! Input is `action=parse&prop=text` output: machine-generated, well-formed
//! MediaWiki HTML. Unlike the wikitext cleaner (which must drop templates and
//! tables — their data only exists server-side), the rendered page *contains*
//! the infobox and table data, so this reducer keeps it: table cells become
//! ` | `-separated rows and Fandom portable-infobox fields become
//! `Label: Value` lines. Navigation cruft (TOC, navboxes, edit sections,
//! images, scripts/styles) is dropped by tag, id, or class.

/// Elements whose entire subtree is discarded regardless of attributes.
const DROP_TAGS: &[&str] = &["script", "style", "svg"];

/// Ids marking a cruft subtree. Stardew's footer navbox is
/// `class="wikitable" id="navbox"` — only the id identifies it.
const DROP_IDS: &[&str] = &["toc", "navbox"];

/// Exact class tokens marking a cruft subtree. Any token *containing*
/// "navbox" is also dropped (Fandom: `fandom-table navbox`, `navboxGroupTable`).
const DROP_CLASSES: &[&str] = &[
    "mw-editsection",
    "table-progress-checkbox-cell", // Fandom interactive checklist cells
    "pi-image",                     // portable-infobox image figure
    "printfooter",
    "catlinks",
];

/// Tags that end an output line when they close.
const BLOCK_TAGS: &[&str] = &[
    "p", "div", "section", "aside", "li", "ul", "ol", "dl", "dt", "dd", "h1",
    "h2", "h3", "h4", "h5", "h6", "table", "caption", "figcaption",
    "blockquote", "center", "pre",
];

/// Void elements: no closing tag, never pushed on the stack.
const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link",
    "meta", "param", "source", "track", "wbr",
];

struct Elem {
    tag: String,
    drop: bool,
    pi_label: bool,
}

/// Reduce rendered MediaWiki HTML to plaintext, keeping table/infobox data.
pub fn to_plaintext(html: &str) -> String {
    let mut out = String::new();
    let mut stack: Vec<Elem> = Vec::new();
    let mut dropping = 0usize;
    let mut after_cell = false;

    let mut rest = html;
    loop {
        let Some(pos) = rest.find('<') else {
            if dropping == 0 {
                push_text(&mut out, rest);
            }
            break;
        };
        if dropping == 0 {
            push_text(&mut out, &rest[..pos]);
        }
        rest = &rest[pos..];

        if let Some(comment) = rest.strip_prefix("<!--") {
            match comment.find("-->") {
                Some(end) => rest = &comment[end + 3..],
                None => break, // unterminated comment: nothing more to keep
            }
            continue;
        }

        let Some(end) = find_tag_end(rest) else {
            break; // unterminated tag: drop the remainder
        };
        let tag_src = &rest[1..end];
        rest = &rest[end + 1..];

        if let Some(name) = tag_src.strip_prefix('/') {
            close_tag(
                &name.trim().to_ascii_lowercase(),
                &mut stack,
                &mut dropping,
                &mut out,
                &mut after_cell,
            );
        } else {
            open_tag(tag_src, &mut stack, &mut dropping, &mut out, &mut after_cell);
        }
    }

    tidy(&decode_entities(&out))
}

/// Append a text node. Source-formatting newlines/tabs become spaces so that
/// line breaks in the output come only from structural tags (tr, blocks, br).
fn push_text(out: &mut String, text: &str) {
    for c in text.chars() {
        out.push(if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c });
    }
}

/// Byte index of the tag-closing `>`, skipping over quoted attribute values.
fn find_tag_end(rest: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in rest.char_indices().skip(1) {
        match (quote, c) {
            (None, '>') => return Some(i),
            (None, '"') | (None, '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            _ => {}
        }
    }
    None
}

fn open_tag(
    tag_src: &str,
    stack: &mut Vec<Elem>,
    dropping: &mut usize,
    out: &mut String,
    after_cell: &mut bool,
) {
    let name_end = tag_src
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(tag_src.len());
    let name = tag_src[..name_end].to_ascii_lowercase();
    if name.is_empty() {
        return;
    }
    let attrs = &tag_src[name_end..];
    let self_closing = tag_src.trim_end().ends_with('/');

    if VOID_TAGS.contains(&name.as_str()) {
        if *dropping == 0 && (name == "br" || name == "hr") {
            out.push('\n');
        }
        return;
    }

    let id = attr_value(attrs, "id").unwrap_or("");
    let class = attr_value(attrs, "class").unwrap_or("");
    let drop = DROP_TAGS.contains(&name.as_str())
        || DROP_IDS.contains(&id)
        || class.split_whitespace().any(|t| {
            t.contains("navbox") || DROP_CLASSES.contains(&t)
        });
    let pi_label = class.split_whitespace().any(|t| t == "pi-data-label");

    if !drop && *dropping == 0 {
        match name.as_str() {
            "tr" => *after_cell = false,
            "td" | "th" if *after_cell => {
                out.push_str(" | ");
                *after_cell = false;
            }
            _ => {}
        }
    }

    if self_closing {
        return; // no subtree, nothing to track
    }
    if drop {
        *dropping += 1;
    }
    stack.push(Elem {
        tag: name,
        drop,
        pi_label,
    });
}

fn close_tag(
    name: &str,
    stack: &mut Vec<Elem>,
    dropping: &mut usize,
    out: &mut String,
    after_cell: &mut bool,
) {
    // Pop to the matching open tag, tolerating minor nesting slop.
    while let Some(top) = stack.pop() {
        if top.drop {
            *dropping = dropping.saturating_sub(1);
        }
        let matched = top.tag == name;
        if matched && !top.drop && *dropping == 0 {
            if top.pi_label {
                // `Label` + upcoming value → "Label: Value" on one line.
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(": ");
            } else {
                match top.tag.as_str() {
                    "td" | "th" => *after_cell = true,
                    "tr" => {
                        out.push('\n');
                        *after_cell = false;
                    }
                    t if BLOCK_TAGS.contains(&t) => out.push('\n'),
                    _ => {}
                }
            }
        }
        if matched {
            return;
        }
    }
}

/// Extract a double- or single-quoted attribute value (e.g. `id`, `class`).
fn attr_value<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attrs;
    while let Some(pos) = rest.find(name) {
        let before_ok = rest[..pos]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace());
        let after = &rest[pos + name.len()..];
        if before_ok {
            let after_eq = after.trim_start();
            if let Some(v) = after_eq.strip_prefix('=') {
                let v = v.trim_start();
                for q in ['"', '\''] {
                    if let Some(body) = v.strip_prefix(q) {
                        return body.find(q).map(|end| &body[..end]);
                    }
                }
            }
        }
        rest = &rest[pos + name.len()..];
    }
    None
}

/// Decode the entities MediaWiki output actually uses: the five named XML
/// entities, `&nbsp;`, and numeric `&#NNN;` / `&#xHH;` forms (NBSP → space).
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let semi = match rest.find(';') {
            Some(i) if i <= 10 => i,
            _ => {
                out.push('&');
                rest = &rest[1..];
                continue;
            }
        };
        let entity = &rest[1..semi];
        let decoded = if let Some(num) = entity.strip_prefix('#') {
            let cp = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse::<u32>().ok(),
            };
            cp.and_then(char::from_u32)
                .map(|c| if c == '\u{a0}' { ' ' } else { c })
        } else {
            match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => None,
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Collapse whitespace per line, dropping empty and separator-only lines.
fn tidy(raw: &str) -> String {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() || collapsed.chars().all(|c| c == '|' || c == ' ') {
            continue;
        }
        lines.push(collapsed);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stardew_infobox_rows_become_pipe_lines() {
        let html = r#"<div class="mw-parser-output"><div id="infoboxborder"><table id="infoboxtable"><tbody>
<tr><td colspan="2" id="infoboxheader">Powdermelon</td></tr>
<tr><td id="infoboxsection">Growth Time</td><td id="infoboxdetail">7 days</td></tr>
<tr><td id="infoboxsection">Season</td><td id="infoboxdetail"><a href="/Winter" title="Winter">Winter</a></td></tr>
</tbody></table></div><p>A crop.</p></div>"#;
        let text = to_plaintext(html);
        assert!(text.contains("Powdermelon"), "{text}");
        assert!(text.contains("Growth Time | 7 days"), "{text}");
        assert!(text.contains("Season | Winter"), "{text}");
        assert!(text.contains("A crop."), "{text}");
    }

    #[test]
    fn wikitable_rows_become_pipe_lines() {
        let html = r#"<table class="wikitable roundedborder"><tr><th>Stage 1</th><th>Harvest</th></tr>
<tr><td>1 Day</td><td>Total: 7 Days</td></tr></table>"#;
        assert_eq!(
            to_plaintext(html),
            "Stage 1 | Harvest\n1 Day | Total: 7 Days"
        );
    }

    #[test]
    fn drops_navbox_toc_style_images_and_svg() {
        let html = r#"<style>.x{}</style><div id="toc">1 Contents</div>
<table class="wikitable" id="navbox"><tr><td>Crops</td><td>Spring</td></tr></table>
<p>Keep <img src="x.png" alt="icon" /> me</p><svg><path d="M0"/></svg>"#;
        assert_eq!(to_plaintext(html), "Keep me");
    }

    #[test]
    fn portable_infobox_fields_become_label_value_lines() {
        let html = r#"<aside class="portable-infobox pi-background pi-theme-wikia">
<h2 class="pi-item pi-title">Copper Ore</h2>
<figure class="pi-item pi-image"><a href="x"><img src="y.png"/></a></figure>
<div class="pi-item pi-data"><h3 class="pi-data-label pi-secondary-font">Rarity</h3><div class="pi-data-value pi-font">Common</div></div>
<div class="pi-item pi-data"><h3 class="pi-data-label">Stackable</h3><div class="pi-data-value">&#10004;&#160;Yes</div></div>
</aside>"#;
        let text = to_plaintext(html);
        assert!(text.contains("Copper Ore"), "{text}");
        assert!(text.contains("Rarity: Common"), "{text}");
        assert!(text.contains("Stackable: ✔ Yes"), "{text}");
    }

    #[test]
    fn drops_fandom_navbox_and_checkbox_cells() {
        let html = r#"<table class="fandom-table navbox"><tr><td>nav junk</td></tr></table>
<table class="table-progress-tracking sortable fandom-table"><tr>
<td class="table-progress-checkbox-cell"><input type="checkbox"/><label>tick</label></td>
<td>Giant Mushroom</td><td>+25</td></tr></table>"#;
        assert_eq!(to_plaintext(html), "Giant Mushroom | +25");
    }

    #[test]
    fn decodes_numeric_entities_and_strips_comments() {
        let html = "<p>A &#8211; B &#x2192; C &amp; D<!-- hidden --></p>";
        assert_eq!(to_plaintext(html), "A – B → C & D");
    }

    #[test]
    fn br_splits_lines_inside_cells() {
        assert_eq!(to_plaintext("<p>+19 food<br />+4.2 health</p>"), "+19 food\n+4.2 health");
    }

    #[test]
    fn nested_tables_produce_their_own_rows() {
        // Stardew sell-price grids are tables nested inside infobox cells.
        let html = r#"<table id="outer"><tr><td id="infoboxsection" colspan="2">Sell Prices</td></tr>
<tr><td><table class="no-wrap"><tr><td>60g</td><td>75g</td></tr></table></td></tr></table>"#;
        let text = to_plaintext(html);
        assert!(text.contains("Sell Prices"), "{text}");
        assert!(text.contains("60g | 75g"), "{text}");
    }

    #[test]
    fn empty_and_separator_only_lines_are_dropped() {
        let html = "<table><tr><td></td><td></td></tr><tr><td>real</td></tr></table>";
        assert_eq!(to_plaintext(html), "real");
    }
}
