//! Facts about one project: description and to-dos from the notes vault (or README), git state, Run actions, icon.
use crate::settings::{legacy_dir, Settings};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct Notes {
    pub desc: String,
    pub todo: Vec<String>,
    pub done: usize,
    pub dir: Option<PathBuf>,
}

/// First prose paragraph of markdown: skips headings, quotes, tables, code fences; strips links and emphasis.
pub fn first_para(text: &str) -> String {
    let mut out: Vec<String> = vec![];
    for line in text.lines() {
        let t = line.trim();
        let skip = t.is_empty() || ["#", "<!--", ">", "|", "![", "[!", "```", "---"].iter().any(|p| t.starts_with(p));
        if skip {
            if !out.is_empty() {
                break;
            }
            continue;
        }
        out.push(clean_md(t));
    }
    let s = out.join(" ");
    s.chars().take(320).collect()
}

/// [[target|label]] -> label, [[target]] -> target, [text](url) -> text, drop * _ `.
pub fn clean_md(t: &str) -> String {
    let mut s = String::new();
    let mut rest = t;
    while let Some(i) = rest.find('[') {
        s.push_str(&rest[..i]);
        let r = &rest[i..];
        if let Some(inner) = r.strip_prefix("[[").and_then(|x| x.split_once("]]")) {
            s.push_str(inner.0.rsplit('|').next().unwrap_or(inner.0));
            rest = inner.1;
        } else if let Some((label, after)) = r[1..].split_once("](") {
            s.push_str(label);
            rest = after.split_once(')').map(|x| x.1).unwrap_or("");
        } else {
            s.push('[');
            rest = &r[1..];
        }
    }
    s.push_str(rest);
    s.chars().filter(|c| !matches!(c, '*' | '_' | '`')).collect()
}

fn frontmatter_path(f: &Path) -> Option<String> {
    let head: String = fs::read_to_string(f).ok()?.chars().take(3000).collect();
    let line = head.lines().find(|l| l.starts_with("path:"))?;
    Some(crate::scan::norm(line[5..].trim().trim_matches('"')))
}

/// Vault project notes: (code path from frontmatter, note file). Hub notes are <slug>/<slug>.md.
pub fn vault_index(s: &Settings) -> Vec<(String, PathBuf)> {
    if s.vault.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for area in ["02-Projects", "03-UNI"] {
        for e in fs::read_dir(Path::new(&s.vault).join(area)).into_iter().flatten().flatten() {
            let slug = e.file_name().to_string_lossy().into_owned();
            let f = e.path().join(format!("{slug}.md"));
            if let Some(p) = frontmatter_path(&f) {
                out.push((p, f));
            }
        }
    }
    out
}

pub fn notes(root: &str, idx: &[(String, PathBuf)]) -> Notes {
    let hit = idx.iter().find(|(p, _)| p == root).or_else(|| idx.iter().find(|(p, _)| p.starts_with(&format!("{root}/")) || root.starts_with(&format!("{p}/"))));
    let mut n = Notes::default();
    if let Some((_, f)) = hit {
        let text = fs::read_to_string(f).unwrap_or_default();
        let body = text.splitn(3, "---").nth(2).unwrap_or(&text);
        n.desc = first_para(body);
        n.dir = f.parent().map(Path::to_path_buf);
        if let Ok(rm) = fs::read_to_string(f.with_file_name("roadmap.md")) {
            for l in rm.lines() {
                if let Some(t) = l.strip_prefix("- [ ] ") {
                    n.todo.push(t.replace("**", "").replace('`', "").chars().take(140).collect());
                } else if l.starts_with("- [x]") || l.starts_with("- [X]") {
                    n.done += 1;
                }
            }
        }
    }
    if n.desc.is_empty() {
        if let Ok(r) = fs::read_to_string(Path::new(root).join("README.md")) {
            n.desc = first_para(&r.chars().take(4000).collect::<String>());
        }
    }
    n
}

#[derive(Clone, Debug, Default)]
pub struct Git {
    pub branch: String,
    pub changed: usize,
    pub ahead: usize,
}

pub fn git(root: &str) -> Option<Git> {
    let o = Command::new("git").args(["-C", root, "status", "--porcelain", "-b"]).output().ok()?;
    let out = String::from_utf8_lossy(&o.stdout);
    let mut lines = out.lines();
    let head = lines.next()?.strip_prefix("## ")?;
    let head = head.strip_prefix("No commits yet on ").unwrap_or(head);
    let branch = head.split("...").next().unwrap_or(head).split(' ').next().unwrap_or(head).to_string();
    let ahead = head.split("[ahead ").nth(1).and_then(|x| x.split([']', ',']).next()).and_then(|n| n.trim().parse().ok()).unwrap_or(0);
    Some(Git { branch, changed: lines.count(), ahead })
}

#[derive(Clone, Debug)]
pub struct Action {
    pub label: String,
    pub cmd: Option<String>,
    pub file: Option<String>,
    pub cwd: String,
    pub url: Option<String>,
    pub needs: Vec<String>,
}

/// Run actions per project root ({root: [{label, cmd|file, cwd, url, needs}]}), from the Python app's actions.json.
pub fn actions(root: &str) -> Vec<Action> {
    let v: Value = fs::read_to_string(legacy_dir().join("actions.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    v[root].as_array().into_iter().flatten().filter_map(|a| {
        Some(Action {
            label: a["label"].as_str()?.into(),
            cmd: a["cmd"].as_str().map(Into::into),
            file: a["file"].as_str().map(Into::into),
            cwd: a["cwd"].as_str().unwrap_or("").into(),
            url: a["url"].as_str().map(Into::into),
            needs: a["needs"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(Into::into)).collect(),
        })
    }).collect()
}

const ICON_GUESSES: &[&str] = &["logo.svg", "icon.svg", "logo.png", "icon.png", "assets/icon.png", "app/web/icons/Icon-512.png", "web/icons/Icon-512.png", "public/icon.svg", "public/favicon.svg"];

pub fn icon(root: &str, art: &Value) -> Option<PathBuf> {
    let mut rels: Vec<String> = art[root].as_str().map(|s| vec![s.to_string()]).unwrap_or_default();
    rels.extend(ICON_GUESSES.iter().map(|s| s.to_string()));
    rels.into_iter().map(|r| Path::new(root).join(r)).find(|p| p.is_file())
}

pub fn art() -> Value {
    fs::read_to_string(legacy_dir().join("art.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wiki_and_md_links_are_cleaned() {
        assert_eq!(clean_md("Notes: [[../../04-Notes/project-hub|Project Hub]]. See [docs](http://x)."), "Notes: Project Hub. See docs.");
        assert_eq!(clean_md("a [[plain]] b [x] c"), "a plain b [x] c");
    }
    #[test]
    fn first_para_skips_headings() {
        assert_eq!(first_para("# Title\n\nHello **world**\nmore\n\nnext"), "Hello world more");
    }
}
