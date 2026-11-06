//! Sessions of opencode and Codex. Claude Code keeps one JSONL file per session (scan.rs); these two keep theirs in
//! SQLite. The schemas are internal to each tool and change between versions, so any error means "no sessions".
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rec {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub updated: f64,            // unix seconds
    pub prompts: Vec<f64>,       // when each typed message was sent
    pub tokens: Vec<(f64, u64)>, // (when, output tokens)
}

fn open(p: &Path) -> Option<Connection> {
    Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()
}

/// opencode's database. shortcut: assumes XDG paths on every OS (true on Linux); check where it lives on Windows.
pub fn opencode_db() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| Some(dirs::home_dir()?.join(".local/share")))?;
    Some(base.join("opencode/opencode.db")).filter(|p| p.is_file())
}

/// Codex's newest state database (state_5.sqlite today) in CODEX_HOME or ~/.codex.
pub fn codex_db() -> Option<PathBuf> {
    let home = std::env::var_os("CODEX_HOME").map(PathBuf::from).or_else(|| Some(dirs::home_dir()?.join(".codex")))?;
    std::fs::read_dir(home).ok()?.flatten().filter_map(|e| {
        let name = e.file_name().to_string_lossy().into_owned();
        let n: u32 = name.strip_prefix("state_")?.strip_suffix(".sqlite")?.parse().ok()?;
        Some((n, e.path()))
    }).max_by_key(|x| x.0).map(|x| x.1)
}

/// Top-level opencode sessions with their typed messages and output tokens.
pub fn opencode(db: &Path) -> Vec<Rec> {
    let Some(c) = open(db) else { return vec![] };
    let read = || -> rusqlite::Result<Vec<Rec>> {
        let mut by_id: HashMap<String, Rec> = HashMap::new();
        let mut order = vec![];
        let mut st = c.prepare("SELECT id, directory, COALESCE(title, ''), time_updated FROM session_v2 WHERE parent_id IS NULL AND time_archived IS NULL")?;
        for r in st.query_map([], |r| Ok(Rec { id: r.get(0)?, cwd: r.get(1)?, title: r.get(2)?, updated: r.get::<_, i64>(3)? as f64 / 1000.0, ..Rec::default() }))? {
            let r = r?;
            order.push(r.id.clone());
            by_id.insert(r.id.clone(), r);
        }
        let mut st = c.prepare("SELECT session_id, type, time_created, json_extract(data, '$.text'), json_extract(data, '$.tokens.output') FROM session_message WHERE type IN ('user', 'assistant') ORDER BY session_id, seq")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<i64>>(4)?)))?;
        for row in rows {
            let (sid, kind, ms, text, out) = row?;
            let Some(rec) = by_id.get_mut(&sid) else { continue };
            let t = ms as f64 / 1000.0;
            if kind == "user" {
                rec.prompts.push(t);
                if rec.title.is_empty() {
                    rec.title = text.unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ");
                }
            } else if let Some(n) = out {
                rec.tokens.push((t, n.max(0) as u64));
            }
        }
        Ok(order.into_iter().filter_map(|id| by_id.remove(&id)).collect())
    };
    read().unwrap_or_default()
}

/// Codex threads. shortcut: one prompt per thread at its start time and its token total, because the per-message
/// log is in rollout files this does not read; read them if Codex activity needs to be exact.
pub fn codex(db: &Path) -> Vec<Rec> {
    let Some(c) = open(db) else { return vec![] };
    let read = || -> rusqlite::Result<Vec<Rec>> {
        let mut st = c.prepare("SELECT * FROM threads WHERE archived = 0")?;
        let rows = st.query_map([], |r| {
            let text = |k: &str| r.get::<_, String>(k).unwrap_or_default();
            let num = |k: &str| r.get::<_, i64>(k).unwrap_or(0);
            let created = num("created_at") as f64;
            let updated = num("updated_at") as f64;
            let title = if text("title").is_empty() { text("first_user_message") } else { text("title") };
            Ok(Rec { id: text("id"), cwd: text("cwd"), title, updated, prompts: vec![created], tokens: vec![(updated, num("tokens_used").max(0) as u64)] })
        })?;
        rows.collect()
    };
    read().unwrap_or_default().into_iter().filter(|r| !r.id.is_empty() && !r.cwd.is_empty()).collect()
}

/// Sessions of `agent`, or none for Claude Code (scan.rs reads its files) and for tools that are not installed.
pub fn load(agent: &str) -> Vec<Rec> {
    match agent {
        "opencode" => opencode_db().map(|p| opencode(&p)),
        "codex" => codex_db().map(|p| codex(&p)),
        _ => None,
    }
    .unwrap_or_default()
}

/// The typed messages and text replies of one opencode session, oldest first: (text, from the user).
pub fn opencode_history(id: &str) -> Vec<(String, bool)> {
    let Some(c) = opencode_db().and_then(|p| open(&p)) else { return vec![] };
    let read = || -> rusqlite::Result<Vec<(String, bool)>> {
        let mut st = c.prepare("SELECT type, data FROM session_message WHERE session_id = ?1 AND type IN ('user', 'assistant') ORDER BY seq")?;
        let rows = st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = vec![];
        for row in rows {
            let (kind, data) = row?;
            let v: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            let text = if kind == "user" {
                v["text"].as_str().unwrap_or("").to_string()
            } else {
                v["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")
            };
            if !text.trim().is_empty() {
                out.push((text.trim().to_string(), kind == "user"));
            }
        }
        Ok(out)
    };
    read().unwrap_or_default()
}

/// The model a session used, from the agent's own database (opencode, Codex).
pub fn model_of(agent: &str, id: &str) -> Option<String> {
    let (db, q) = match agent {
        "opencode" => (opencode_db()?, "SELECT json_extract(model, '$.id') FROM session_v2 WHERE id = ?1"),
        "codex" => (codex_db()?, "SELECT model FROM threads WHERE id = ?1"),
        _ => return None,
    };
    open(&db)?.query_row(q, [id], |r| r.get::<_, Option<String>>(0)).ok().flatten().filter(|m| !m.is_empty())
}

/// The typed messages and text replies of one Codex thread, from its rollout file, oldest first.
pub fn codex_history(id: &str) -> Vec<(String, bool)> {
    let Some(c) = codex_db().and_then(|p| open(&p)) else { return vec![] };
    let Ok(path) = c.query_row("SELECT rollout_path FROM threads WHERE id = ?1", [id], |r| r.get::<_, String>(0)) else { return vec![] };
    let Ok(f) = std::fs::File::open(path) else { return vec![] };
    use std::io::BufRead;
    let mut out = vec![];
    for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        if !line.contains("\"response_item\"") {
            continue; // cheap filter before parsing
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let p = &v["payload"];
        let user = p["role"] == "user";
        if p["type"] != "message" || !(user || p["role"] == "assistant") {
            continue;
        }
        let text: String = p["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
        let text = text.trim();
        if !text.is_empty() && !text.starts_with('<') {
            out.push((text.to_string(), user)); // '<' starts Codex's own context blocks, not typed text
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_opencode_sessions() {
        let dir = std::env::temp_dir().join(format!("cove-oc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("opencode.db");
        let _ = std::fs::remove_file(&db);
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE session_v2 (id TEXT, directory TEXT, title TEXT, time_updated INTEGER, parent_id TEXT, time_archived INTEGER);
             CREATE TABLE session_message (session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER, data TEXT);
             INSERT INTO session_v2 VALUES ('ses_a', '/p/one', NULL, 2000000, NULL, NULL), ('ses_child', '/p/one', 'sub', 1, 'ses_a', NULL);
             INSERT INTO session_message VALUES
               ('ses_a', 'synthetic', 0, 1000, '{\"text\":\"system\"}'),
               ('ses_a', 'user', 1, 1000000, '{\"text\":\"fix  the bug\"}'),
               ('ses_a', 'assistant', 2, 1001000, '{\"content\":[{\"type\":\"text\",\"text\":\"done\"}],\"tokens\":{\"output\":42}}');",
        )
        .unwrap();
        drop(c);
        let recs = opencode(&db);
        assert_eq!(recs.len(), 1, "child sessions are not listed");
        let r = &recs[0];
        assert_eq!((r.id.as_str(), r.cwd.as_str(), r.title.as_str()), ("ses_a", "/p/one", "fix the bug"));
        assert_eq!(r.updated, 2000.0);
        assert_eq!(r.prompts, vec![1000.0]);
        assert_eq!(r.tokens, vec![(1001.0, 42)]);
        assert!(opencode(&dir.join("missing.db")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
