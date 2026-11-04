//! The hub's own chat: a real Claude Code session in a staging folder, so it can later move into a new project.
use crate::scan::{claude_dir, encode};
use crate::settings::cache_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

static DIR: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// The chat folder from Settings ("" = ~/Chats). Set on start and when Settings change.
pub fn set_dir(path: &str) {
    *DIR.write().unwrap() = (!path.trim().is_empty()).then(|| PathBuf::from(path.trim()));
}

/// Chats run in their own folder (~/Chats unless Settings say otherwise), so Claude Code keeps their sessions
/// apart; the hub lists them under Chats in the sidebar, never as a project.
pub fn chat_dir() -> PathBuf {
    DIR.read().unwrap().clone().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(cache_dir).join("Chats"))
}

/// A session id as a bare file stem: nothing that could reach outside the sessions folder.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The name Claude Code shows for a session: the last rename (custom-title), else its own summary (ai-title).
fn named_title(f: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let fh = fs::File::open(f).ok()?;
    let (mut custom, mut ai) = (None, None);
    for line in std::io::BufReader::new(fh).lines().map_while(Result::ok) {
        if !line.contains("-title\"") { continue; } // cheap filter before parsing
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        match v["type"].as_str() {
            Some("custom-title") => custom = v["customTitle"].as_str().map(str::to_string),
            Some("ai-title") => ai = v["aiTitle"].as_str().map(str::to_string),
            _ => {}
        }
    }
    custom.or(ai).filter(|t| !t.trim().is_empty())
}

/// Rename a chat the way Claude Code's /rename does, so `claude --resume` shows the same name.
pub fn rename(id: &str, title: &str) -> std::io::Result<()> {
    let title = title.trim();
    if !valid_id(id) || title.is_empty() {
        return Err(std::io::Error::other("Give the chat a name."));
    }
    let line = serde_json::json!({ "type": "custom-title", "customTitle": title, "sessionId": id });
    let mut f = fs::OpenOptions::new().append(true).open(sessions_dir().join(format!("{id}.jsonl")))?;
    writeln!(f, "{line}")
}

/// Delete a chat for good: its transcript and the folder of tool output Claude Code keeps beside it.
pub fn delete(id: &str) -> std::io::Result<()> {
    if !valid_id(id) {
        return Err(std::io::Error::other("Not a chat id."));
    }
    let d = sessions_dir();
    fs::remove_file(d.join(format!("{id}.jsonl")))?;
    let side = d.join(id);
    if side.is_dir() {
        fs::remove_dir_all(side)?;
    }
    Ok(())
}

fn sessions_dir() -> PathBuf {
    claude_dir().join("projects").join(encode(&chat_dir().to_string_lossy()))
}

/// Chat sessions, newest first: (id, title, mtime).
pub fn list(n: usize) -> Vec<(String, String, f64)> {
    let mut v: Vec<(String, String, f64)> = crate::scan::jsonl_files(&sessions_dir())
        .into_iter()
        .filter_map(|f| Some((f.file_stem()?.to_string_lossy().into_owned(), named_title(&f).or_else(|| crate::scan::first_prompt(&f))?, crate::scan::mtime(&f))))
        .collect();
    v.sort_by(|a, b| b.2.total_cmp(&a.2));
    v.truncate(n);
    v
}

/// The typed messages and text replies of a chat, oldest first: (text, from the user).
pub fn history(id: &str) -> Vec<(String, bool)> {
    use std::io::BufRead;
    let Ok(fh) = fs::File::open(sessions_dir().join(format!("{id}.jsonl"))) else { return vec![] };
    let mut out = vec![];
    for line in std::io::BufReader::new(fh).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if let Some(t) = crate::scan::user_text(&v) {
            out.push((t, true));
        } else if v["type"] == "assistant" {
            let t: String = v["message"]["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
            if !t.trim().is_empty() {
                out.push((t.trim().to_string(), false));
            }
        }
    }
    out
}

/// Send one message; resumes `sid` when given. Returns (session id, reply).
pub fn send(sid: Option<&str>, text: &str) -> Result<(String, String), String> {
    let claude = crate::launch::which("claude").ok_or("The chat needs Claude Code (claude) on PATH.")?;
    let dir = chat_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut c = Command::new(claude);
    crate::launch::subscription_only(&mut c);
    // web tools only, and nothing that would ask: the chat reads outside text, so it must not run commands or edit files
    c.args(["-p", "--output-format", "json", "--tools", "WebSearch,WebFetch", "--allowedTools", "WebSearch,WebFetch", "--permission-mode", "dontAsk"]);
    if let Some(s) = sid {
        c.args(["--resume", s]);
    }
    let mut ch = c.current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    ch.stdin.take().map(|mut i| i.write_all(text.as_bytes()));
    let o = ch.wait_with_output().map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).map_err(|_| "Claude Code did not answer. Is it signed in?".to_string())?;
    let sid = v["session_id"].as_str().ok_or("Claude Code did not answer.")?.to_string();
    Ok((sid, v["result"].as_str().unwrap_or("").trim().to_string()))
}

/// Move a chat session into `to` (a project folder), so `claude --resume <sid>` there continues it.
/// Copies first and removes the original only from the hub's own staging folder.
pub fn move_session(sid: &str, to: &str) -> std::io::Result<()> {
    let projects = claude_dir().join("projects");
    let from = projects.join(encode(&chat_dir().to_string_lossy())).join(format!("{sid}.jsonl"));
    let dest = projects.join(encode(to));
    fs::create_dir_all(&dest)?;
    fs::copy(&from, dest.join(format!("{sid}.jsonl")))?;
    fs::remove_file(from)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_cannot_leave_the_folder() {
        assert!(super::valid_id("bfb25168-4f08-45e1-a77b-34ad6887475d"));
        for bad in ["", "../x", "a/b", "a.jsonl", "a b"] {
            assert!(!super::valid_id(bad), "{bad}");
        }
    }
}
