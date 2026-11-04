//! Finds projects (children of the scan folders) and the coding-agent sessions stored for each.
use crate::settings::{legacy_dir, config_dir, Settings};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Project {
    pub root: String,
    pub name: String,
    pub collection: String,
    pub dirs: Vec<PathBuf>, // Claude Code session folders that belong to this project
    pub recs: Vec<crate::agents::Rec>, // opencode or Codex sessions, when that agent is selected
    pub sessions: usize,
    pub last: f64, // newest session file mtime, unix seconds; 0 = none
}

pub fn home() -> String {
    dirs::home_dir().unwrap_or_default().to_string_lossy().into_owned()
}

pub fn claude_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude"))
}

/// /mnt/data is a symlink to /data on the author's machine; old sessions recorded the long form.
pub fn norm(p: &str) -> String {
    match p.strip_prefix("/mnt/data/") {
        Some(rest) => format!("/data/{rest}"),
        None => p.to_string(),
    }
}

/// Claude Code names a project's session folder by replacing every non-alphanumeric char of the path with '-'.
pub fn encode(path: &str) -> String {
    path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

pub fn mtime(p: &Path) -> f64 {
    fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn is_inside(path: &str, dir: &str) -> bool {
    path == dir || path.strip_prefix(dir).is_some_and(|r| r.starts_with('/') || r.starts_with('\\'))
}

/// (collection, project root) for a path: the scan folder child that contains it (longest scan folder wins).
pub fn place(path: &str, s: &Settings) -> Option<(String, String)> {
    let best = s.scan_dirs.iter().filter(|d| is_inside(path, &d.path)).max_by_key(|d| d.path.len())?;
    let rest = path[best.path.len()..].trim_start_matches(['/', '\\']);
    if rest.is_empty() {
        return None; // the scan folder itself is not a project
    }
    let first = rest.split(['/', '\\']).next().unwrap_or(rest);
    Some((best.collection.clone(), Path::new(&best.path).join(first).to_string_lossy().into_owned()))
}

/// Like `place`, but a session started in a scan folder itself makes that folder a project.
fn place_or_self(path: &str, s: &Settings) -> Option<(String, String)> {
    place(path, s).or_else(|| s.scan_dirs.iter().find(|d| d.path == path).map(|d| (d.collection.clone(), path.to_string())))
}

/// Map a recorded cwd to an existing folder: undo a space-to-underscore rename, else climb to the
/// nearest existing ancestor that is still inside a scan folder.
pub fn resolve(cwd: &str, s: &Settings) -> Option<String> {
    for cand in [cwd.to_string(), cwd.replace(' ', "_")] {
        let mut q = Path::new(&cand).to_path_buf();
        while !q.is_dir() {
            if !q.pop() {
                break;
            }
        }
        let q = q.to_string_lossy().into_owned();
        if q == cand || s.scan_dirs.iter().any(|d| is_inside(&q, &d.path)) {
            if Path::new(&q).is_dir() && q != "/" {
                return Some(q);
            }
        }
    }
    None
}

/// Projects created in the app outside the scan folders (this app's list plus the Python app's).
pub fn extra_paths() -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for f in [config_dir().join("projects.json"), legacy_dir().join("projects.json")] {
        if let Ok(v) = fs::read_to_string(f).map(|s| serde_json::from_str::<Vec<String>>(&s).unwrap_or_default()) {
            out.extend(v.into_iter().filter(|p| Path::new(p).is_dir()));
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn add_extra(path: &str) -> std::io::Result<()> {
    let f = config_dir().join("projects.json");
    let mut v: Vec<String> = fs::read_to_string(&f).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if !v.iter().any(|p| p == path) {
        v.push(path.into());
        fs::create_dir_all(config_dir())?;
        fs::write(f, serde_json::to_string_pretty(&v)?)?;
    }
    Ok(())
}

/// The working directory recorded in a session file (first line that has one).
fn session_cwd(file: &Path) -> Option<String> {
    let fh = fs::File::open(file).ok()?;
    for line in BufReader::new(fh).lines().take(50).map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if let Some(c) = v.get("cwd").and_then(|c| c.as_str()) {
                return Some(norm(c));
            }
        }
    }
    None
}

pub fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")).collect()).unwrap_or_default()
}

pub fn collect(s: &Settings) -> Vec<Project> {
    let home = home();
    let mut cards: HashMap<String, Project> = HashMap::new();
    fn add(cards: &mut HashMap<String, Project>, home: &str, root: &str, coll: &str) {
        cards.entry(root.to_string()).or_insert_with(|| Project {
            root: root.into(),
            name: if root == home { "Home folder".into() } else { Path::new(root).file_name().map(|n| n.to_string_lossy().replace('_', " ")).unwrap_or_default() },
            collection: coll.into(),
            dirs: vec![],
            recs: vec![],
            sessions: 0,
            last: 0.0,
        });
    }
    // every child folder of every scan folder is a project, sessions or not
    for d in s.scan_dirs.iter().filter(|d| d.all) {
        for e in fs::read_dir(&d.path).into_iter().flatten().flatten() {
            let p = e.path();
            let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
            if p.is_dir() && !hidden {
                if let Some((coll, root)) = place(&p.to_string_lossy(), s) {
                    add(&mut cards, &home, &root, &coll);
                }
            }
        }
    }
    for p in extra_paths() {
        match place(&p, s) {
            Some((coll, root)) => add(&mut cards, &home, &root, &coll),
            None => add(&mut cards, &home, &p, "Other"),
        }
    }
    // sessions come from the selected agent only, so switching agents switches every count and list
    let claude = !matches!(s.agent.as_str(), "opencode" | "codex");
    // attach Claude Code session folders
    let mut by_enc: HashMap<String, String> = cards.keys().map(|r| (encode(r), r.clone())).collect();
    by_enc.insert(encode(&home), home.clone());
    let mut found: Vec<(String, PathBuf, Vec<PathBuf>)> = vec![];
    for e in fs::read_dir(claude_dir().join("projects")).into_iter().filter(|_| claude).flatten().flatten() {
        let dir = e.path();
        if !dir.is_dir() || fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            continue;
        }
        let files = jsonl_files(&dir);
        if files.is_empty() {
            continue;
        }
        let enc = dir.file_name().unwrap().to_string_lossy().into_owned();
        if enc == encode(&crate::chat::chat_dir().to_string_lossy()) {
            continue; // the hub's own chat, not a project
        }
        let root = match by_enc.get(&enc) {
            Some(r) => r.clone(),
            None => {
                let newest = files.iter().max_by(|a, b| mtime(a).total_cmp(&mtime(b))).unwrap();
                let Some(cwd) = session_cwd(newest).and_then(|c| resolve(&c, s)) else { continue }; // folder deleted
                if cwd == home {
                    home.clone()
                } else if let Some((coll, root)) = place_or_self(&cwd, s) {
                    add(&mut cards, &home, &root, &coll);
                    root
                } else {
                    add(&mut cards, &home, &cwd, "Other");
                    cwd
                }
            }
        };
        found.push((root, dir, files));
    }
    for (root, dir, files) in found {
        if root == home {
            add(&mut cards, &home, &home, "Home");
        }
        if let Some(c) = cards.get_mut(&root) {
            c.sessions += files.len();
            c.last = c.last.max(files.iter().map(|f| mtime(f)).fold(0.0, f64::max));
            c.dirs.push(dir);
        }
    }
    let chats = crate::chat::chat_dir().to_string_lossy().into_owned();
    for rec in crate::agents::load(&s.agent) {
        let cwd = norm(&rec.cwd);
        if cwd == chats {
            continue; // the hub's own chat, not a project
        }
        let Some(cwd) = resolve(&cwd, s) else { continue }; // folder deleted
        let root = if cwd == home {
            add(&mut cards, &home, &home, "Home");
            home.clone()
        } else if let Some((coll, root)) = place_or_self(&cwd, s) {
            add(&mut cards, &home, &root, &coll);
            root
        } else {
            add(&mut cards, &home, &cwd, "Other");
            cwd
        };
        if let Some(c) = cards.get_mut(&root) {
            c.sessions += 1;
            c.last = c.last.max(rec.updated);
            c.recs.push(rec);
        }
    }
    let mut out: Vec<Project> = cards.into_values().filter(|c| !s.hidden.contains(&c.root)).collect();
    let order = collections(s);
    out.sort_by(|a, b| {
        let ia = order.iter().position(|c| *c == a.collection).unwrap_or(usize::MAX);
        let ib = order.iter().position(|c| *c == b.collection).unwrap_or(usize::MAX);
        ia.cmp(&ib).then(b.last.total_cmp(&a.last)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Collection names in sidebar order.
pub fn collections(s: &Settings) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for d in &s.scan_dirs {
        if !v.contains(&d.collection) {
            v.push(d.collection.clone());
        }
    }
    v.push("Other".into());
    v.push("Home".into());
    v
}

// ---- sessions ---------------------------------------------------------------

/// Text of a user message, or None for tool results, commands and other non-typed input.
pub fn user_text(v: &Value) -> Option<String> {
    if v.get("type")?.as_str()? != "user" || v.get("isMeta").and_then(|m| m.as_bool()) == Some(true) {
        return None;
    }
    let c = v.get("message")?.get("content")?;
    let t = match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(|x| x.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join(" "),
        _ => return None,
    };
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty() && !t.starts_with('<')).then_some(t)
}

/// First thing typed in a session; stops reading as soon as it is found (files can be 75 MB).
pub fn first_prompt(path: &Path) -> Option<String> {
    let fh = fs::File::open(path).ok()?;
    for line in BufReader::new(fh).lines().take(400).map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if let Some(t) = user_text(&v) {
                return Some(t);
            }
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub root: String,
    pub title: String,
    pub mtime: f64,
}

fn sessions_of(projects: &[&Project], n: usize) -> Vec<Session> {
    let mut files: Vec<(f64, PathBuf, String)> = projects.iter().flat_map(|p| p.dirs.iter().flat_map(|d| jsonl_files(d)).map(|f| (mtime(&f), f, p.root.clone()))).collect();
    files.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out: Vec<Session> = projects.iter().flat_map(|p| p.recs.iter().filter(|r| !r.title.is_empty()).map(|r| Session { id: r.id.clone(), root: p.root.clone(), title: r.title.clone(), mtime: r.updated })).collect();
    if !out.is_empty() {
        out.sort_by(|a, b| b.mtime.total_cmp(&a.mtime));
        out.truncate(n);
        return out; // another agent: its sessions are all there is
    }
    for (mt, f, root) in files {
        if let Some(t) = first_prompt(&f) {
            out.push(Session { id: f.file_stem().unwrap().to_string_lossy().into(), root, title: t, mtime: mt });
            if out.len() >= n {
                break;
            }
        }
    }
    out
}

pub fn recent_sessions(projects: &[Project], n: usize) -> Vec<Session> {
    sessions_of(&projects.iter().collect::<Vec<_>>(), n)
}

pub fn project_sessions(p: &Project, n: usize) -> Vec<Session> {
    sessions_of(&[p], n)
}

/// Local date a session started: first ISO timestamp in its first lines, else the file's mtime.
pub fn start_day(path: &Path) -> String {
    if let Ok(fh) = fs::File::open(path) {
        for line in BufReader::new(fh).lines().take(20).map_while(Result::ok) {
            if let Some(i) = line.find("\"timestamp\":\"") {
                let ts = &line[i + 13..];
                if let Some(end) = ts.find('"') {
                    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&ts[..end]) {
                        return t.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string();
                    }
                }
            }
        }
    }
    day_of(mtime(path))
}

pub fn day_of(secs: f64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// Sessions started per local day, over all projects.
pub fn day_counts(projects: &[Project]) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for p in projects {
        for d in &p.dirs {
            for f in jsonl_files(d) {
                *out.entry(start_day(&f)).or_insert(0) += 1;
            }
        }
        for r in &p.recs {
            *out.entry(day_of(r.prompts.first().copied().unwrap_or(r.updated))).or_insert(0) += 1;
        }
    }
    out
}

pub fn ago(secs: f64) -> String {
    let now = chrono::Local::now().timestamp() as f64;
    let d = ((now - secs) / 86400.0).floor() as i64;
    match d {
        i64::MIN..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=59 => format!("{d}d ago"),
        _ => chrono::DateTime::from_timestamp(secs as i64, 0).map(|t| t.with_timezone(&chrono::Local).format("%b %Y").to_string()).unwrap_or_default(),
    }
}

pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let s: String = match words.as_slice() {
        [a, b, ..] => format!("{}{}", a.chars().next().unwrap(), b.chars().next().unwrap()),
        [a] => a.chars().take(2).collect(),
        [] => "?".into(),
    };
    s.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ScanDir;

    fn settings() -> Settings {
        Settings { scan_dirs: vec![ScanDir { path: "/data/UNI".into(), collection: "Uni".into(), all: true }, ScanDir { path: "/data/UNI/research".into(), collection: "Research".into(), all: true }], ..Settings::default() }
    }

    #[test]
    fn place_takes_longest_scan_dir_and_first_child() {
        let s = settings();
        assert_eq!(place("/data/UNI/research/geoai/src/x", &s), Some(("Research".into(), "/data/UNI/research/geoai".into())));
        assert_eq!(place("/data/UNI/sem_7/dl", &s), Some(("Uni".into(), "/data/UNI/sem_7".into())));
        assert_eq!(place("/data/UNI", &s), None);
        assert_eq!(place("/data/UNIX/a", &s), None);
    }

    #[test]
    fn encode_matches_claude_code() {
        assert_eq!(encode("/data/Projects/project-hub"), "-data-Projects-project-hub");
        assert_eq!(encode("/data/Personal files"), "-data-Personal-files");
    }

    #[test]
    fn user_text_skips_tool_results_and_commands() {
        let typed: Value = serde_json::from_str(r#"{"type":"user","message":{"content":"fix the  bug"}}"#).unwrap();
        let tool: Value = serde_json::from_str(r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}"#).unwrap();
        let cmd: Value = serde_json::from_str(r#"{"type":"user","message":{"content":"<command-name>/clear</command-name>"}}"#).unwrap();
        assert_eq!(user_text(&typed).as_deref(), Some("fix the bug"));
        assert_eq!(user_text(&tool), None);
        assert_eq!(user_text(&cmd), None);
    }

    #[test]
    fn initials_from_words() {
        assert_eq!(initials("project-hub"), "PH");
        assert_eq!(initials("lexika"), "LE");
        assert_eq!(initials("Deep learning"), "DL");
    }
}
