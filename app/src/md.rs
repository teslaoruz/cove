//! Chat replies are markdown. Slint's StyledText draws inline marks (bold, italic, code, links) and lists, but
//! rejects a whole string that holds a heading, code fence, table, quote or rule. So a reply is split into
//! blocks first, and each block goes to a renderer that can draw it.

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    Prose,
    Heading,
    Code,
    Table,
    Quote,
    Rule,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Prose => "p",
            Kind::Heading => "h",
            Kind::Code => "code",
            Kind::Table => "table",
            Kind::Quote => "quote",
            Kind::Rule => "rule",
        }
    }
}

fn is_rule(t: &str) -> bool {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    s.len() >= 3 && ["-", "*", "_"].iter().any(|m| s.chars().all(|c| c.to_string() == *m))
}

fn heading(t: &str) -> Option<&str> {
    let n = t.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&n).then(|| t[n..].strip_prefix(' ')).flatten().map(|h| h.trim_end_matches('#').trim())
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().replace("**", "").replace('`', "")).collect()
}

/// A markdown table as aligned monospace rows; the |---| separator row is dropped.
fn table(lines: &[&str]) -> String {
    let rows: Vec<Vec<String>> = lines
        .iter()
        .map(|l| cells(l))
        .filter(|r| !r.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))))
        .collect();
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let width: Vec<usize> = (0..cols).map(|i| rows.iter().map(|r| r.get(i).map_or(0, |c| c.chars().count())).max().unwrap_or(0)).collect();
    rows.iter()
        .map(|r| (0..cols).map(|i| { let c = r.get(i).map_or("", |c| c.as_str()); format!("{c}{}", " ".repeat(width[i] - c.chars().count())) }).collect::<Vec<_>>().join("   ").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split markdown into blocks. Prose keeps its markdown (for StyledText); code and tables come back as plain text.
pub fn blocks(text: &str) -> Vec<(Kind, String)> {
    let mut out = vec![];
    let mut prose: Vec<String> = vec![];
    let mut code: Option<Vec<&str>> = None;
    let mut tab: Vec<&str> = vec![];
    let mut quote: Vec<&str> = vec![];
    fn flush(out: &mut Vec<(Kind, String)>, kind: Kind, s: String) {
        if !s.trim().is_empty() {
            out.push((kind, s.trim_matches('\n').to_string()));
        }
    }
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(c) = code.as_mut() {
            if t.starts_with("```") {
                flush(&mut out, Kind::Code, code.take().unwrap().join("\n"));
            } else {
                c.push(line);
            }
            continue;
        }
        if !tab.is_empty() && !t.starts_with('|') {
            flush(&mut out, Kind::Table, table(&tab));
            tab.clear();
        }
        if !quote.is_empty() && !t.starts_with('>') {
            flush(&mut out, Kind::Quote, quote.join("\n"));
            quote.clear();
        }
        let special = t.starts_with("```") || t.starts_with('|') || t.starts_with('>') || heading(t).is_some() || is_rule(t);
        if special {
            flush(&mut out, Kind::Prose, prose.join("\n"));
            prose.clear();
        }
        if t.starts_with("```") {
            code = Some(vec![]);
        } else if t.starts_with('|') {
            tab.push(t);
        } else if let Some(q) = t.strip_prefix('>') {
            quote.push(q.strip_prefix(' ').unwrap_or(q));
        } else if let Some(h) = heading(t) {
            flush(&mut out, Kind::Heading, h.replace("**", ""));
        } else if is_rule(t) {
            out.push((Kind::Rule, String::new()));
        } else {
            // task lists are not drawn by StyledText; show them as boxes
            let l = line.replacen("- [ ] ", "- ☐ ", 1).replacen("- [x] ", "- ☑ ", 1).replacen("- [X] ", "- ☑ ", 1);
            prose.push(l);
        }
    }
    if let Some(c) = code {
        flush(&mut out, Kind::Code, c.join("\n")); // an unclosed fence (a cut-off reply) is still code
    }
    flush(&mut out, Kind::Table, if tab.is_empty() { String::new() } else { table(&tab) });
    flush(&mut out, Kind::Quote, quote.join("\n"));
    flush(&mut out, Kind::Prose, prose.join("\n"));
    out
}

/// Prose markdown to StyledText. `<` is escaped first (replies are not HTML); anything the parser still
/// rejects is shown as plain text rather than dropped.
pub fn rich(md: &str) -> slint::StyledText {
    // backslash escapes do not apply inside `code`, so only the parts between code spans are escaped
    let esc: String = md.split('`').enumerate().map(|(i, part)| if i % 2 == 0 { part.replace('<', "\\<") } else { part.to_string() }).collect::<Vec<_>>().join("`");
    slint::StyledText::from_markdown(&esc).unwrap_or_else(|_| slint::StyledText::from_plain_text(md))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn splits_a_typical_reply() {
        let md = "# Plan\nSome **bold** text.\n\n- [ ] one\n- [x] two\n\n```rust\nlet x = 1;\n```\n| A | B |\n|---|:-:|\n| long cell | **x** |\n> quoted\n---\nend";
        let b = blocks(md);
        let kinds: Vec<Kind> = b.iter().map(|x| x.0).collect();
        assert_eq!(kinds, [Kind::Heading, Kind::Prose, Kind::Code, Kind::Table, Kind::Quote, Kind::Rule, Kind::Prose]);
        assert_eq!(b[0].1, "Plan");
        assert!(b[1].1.contains("- ☐ one") && b[1].1.contains("- ☑ two"));
        assert_eq!(b[2].1, "let x = 1;");
        assert_eq!(b[3].1, "A           B\nlong cell   x");
        assert_eq!(b[4].1, "quoted");
    }
    #[test]
    fn unclosed_fence_is_code_and_rules_need_three() {
        assert_eq!(blocks("```\ncut off"), [(Kind::Code, "cut off".to_string())]);
        assert_eq!(blocks("--"), [(Kind::Prose, "--".to_string())]);
        assert_eq!(blocks("#nospace"), [(Kind::Prose, "#nospace".to_string())]);
    }
    #[test]
    fn angle_brackets_show_as_written() {
        for s in ["`Vec<String>` and `x < 5`", "a < b", "<br> stays text"] {
            let shown = format!("{:?}", rich(s));
            assert!(!shown.contains("\\\\<") && !shown.contains("\\<"), "{s} -> {shown}");
            assert!(shown.contains('<'), "{s} -> {shown}");
        }
    }
    #[test]
    fn prose_parses_after_escaping() {
        // inline HTML and comparisons would make the parser fail; escaped, they parse as text
        for s in ["a < b and <br> here", "**bold** and `code` and [link](https://x.y)", "1. one\n2. two\n\npara"] {
            assert!(slint::StyledText::from_markdown(&s.replace('<', "\\<")).is_ok(), "{s}");
        }
    }
}
