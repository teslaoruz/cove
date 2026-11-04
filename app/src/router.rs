//! Turns a plain request ("progress on geoai", "run lexika", "new project called x") into an action.
use crate::scan::Project;
use serde_json::Value;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq)]
pub enum Route {
    New { name: String },
    Project { idx: usize, run: bool, status: bool },
}

fn alnum(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Similarity 0..1 (1 - normalised Levenshtein distance).
fn ratio(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    1.0 - prev[b.len()] as f64 / a.len().max(b.len()) as f64
}

/// Fuzzy-find a project name in a possibly mis-heard request: "goai" -> geoai.
pub fn find_project(text: &str, projects: &[Project]) -> Option<usize> {
    let toks: Vec<String> = text.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).map(String::from).collect();
    let mut grams = vec![];
    for n in 1..=3 {
        for w in toks.windows(n) {
            grams.push(w.concat());
        }
    }
    let mut best = (None, 0.0);
    for (i, p) in projects.iter().enumerate() {
        let base = std::path::Path::new(&p.root).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut aliases = vec![alnum(&base)];
        aliases.extend(base.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 4).map(String::from));
        for al in &aliases {
            for g in grams.iter().filter(|g| g.len() >= 3) {
                let r = ratio(al, g);
                if r > best.1 {
                    best = (Some(i), r);
                }
            }
        }
    }
    if best.1 >= 0.8 { best.0 } else { None }
}

const KINDS: &[&str] = &["project", "app", "website", "tool", "bot", "game"];

fn is_new(t: &str) -> bool {
    let w: Vec<&str> = t.split_whitespace().collect();
    w.windows(2).any(|p| p[0] == "new" && KINDS.contains(&p[1]))
        || w.iter().enumerate().any(|(i, x)| ["start", "create", "make", "build"].contains(x) && w[i + 1..].iter().take(4).any(|y| KINDS.contains(y)))
}

pub fn route(text: &str, projects: &[Project]) -> Option<Route> {
    let t = text.to_lowercase();
    if is_new(&t) {
        let w: Vec<&str> = text.split_whitespace().collect();
        let name = w.iter().position(|x| ["called", "named"].contains(&x.to_lowercase().as_str())).and_then(|i| w.get(i + 1)).map(|s| s.to_string()).unwrap_or_default();
        return Some(Route::New { name });
    }
    let idx = find_project(text, projects)?;
    let has = |ws: &[&str]| t.split(|c: char| !c.is_alphanumeric()).any(|x| ws.contains(&x));
    let run = has(&["run", "launch", "start", "serve"]);
    let status = has(&["progress", "status", "roadmap", "todo", "left", "next"]) || t.contains("where were we");
    Some(Route::Project { idx, run, status })
}

/// "continue / resume / keep working on X": continue the last session instead of starting one.
pub fn wants_continue(text: &str) -> bool {
    let t = text.to_lowercase();
    ["continue", "resume", "keep working", "pick up", "back to", "carry on"].iter().any(|w| t.contains(w))
}

/// An explicit request to go to a project, as opposed to merely naming one in conversation.
pub fn explicit_open(text: &str) -> bool {
    let t = text.to_lowercase();
    wants_continue(&t) || t.split(|c: char| !c.is_alphanumeric()).any(|w| ["open", "start", "run", "launch", "switch", "work"].contains(&w))
}

/// Ask a small model to route what plain matching could not. Output is untrusted: the caller validates it.
pub fn llm_route(text: &str, projects: &[(String, String)]) -> Option<Value> {
    let cat: String = projects.iter().map(|(root, desc)| format!("- {} | {} | {}\n", std::path::Path::new(root).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), root, desc.chars().take(90).collect::<String>())).collect();
    let prompt = format!(
        "You route a voice/chat request to a project. Projects:\n{cat}\nUser said: {text:?}\nReply with ONLY a JSON object: \
{{\"action\":\"open\"|\"new\"|\"answer\",\"project\":\"<exact root path or null>\",\"name\":\"<new project name or empty>\",\"prompt\":\"<what to tell the coding agent>\",\"reply\":\"<short answer, only for action answer>\"}}"
    );
    let dir = crate::settings::cache_dir().join("router"); // keep the call's session out of every project
    std::fs::create_dir_all(&dir).ok()?;
    let mut child = Command::new(crate::launch::which("claude")?)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN") // subscription login only: never bill an API key
        .args(["-p", "--model", "haiku", "--no-session-persistence", "--tools="])
        .current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    use std::io::Write;
    child.stdin.take()?.write_all(prompt.as_bytes()).ok()?;
    let out = String::from_utf8_lossy(&child.wait_with_output().ok()?.stdout).into_owned();
    serde_json::from_str(&out[out.find('{')?..=out.rfind('}')?]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(root: &str) -> Project {
        Project { root: root.into(), name: String::new(), collection: String::new(), dirs: vec![], recs: vec![], sessions: 0, last: 0.0 }
    }
    #[test]
    fn routes() {
        let ps = vec![p("/data/Projects/geoai"), p("/data/Projects/lexika")];
        assert_eq!(route("progress on goai", &ps), Some(Route::Project { idx: 0, run: false, status: true }));
        assert_eq!(route("run lexika", &ps), Some(Route::Project { idx: 1, run: true, status: false }));
        assert_eq!(route("start a new project called birdwatch", &ps), Some(Route::New { name: "birdwatch".into() }));
        assert_eq!(route("what is the weather", &ps), None);
        assert!(matches!(route("lets continue working on geoai", &ps), Some(Route::Project { idx: 0, .. })));
        assert!(wants_continue("lets continue working on geoai"));
        assert!(!explicit_open("how does this compare to lexika?"));
        assert!(explicit_open("open lexika"));
    }
}
