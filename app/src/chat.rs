//! The hub's own chat: a real session of the selected agent (Claude Code, opencode or Codex) in a staging folder,
//! so a Claude Code chat can later move into a new project.
use crate::scan::{claude_dir, encode};
use crate::settings::cache_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

static DIR: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);
static AGENT: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

/// The agent chats go to (claude | opencode | codex). Set on start and when the user switches agents.
pub fn set_agent(agent: &str) {
    *AGENT.write().unwrap() = agent.to_string();
}

fn agent() -> String {
    match AGENT.read().unwrap().as_str() {
        a @ ("opencode" | "codex") => a.to_string(),
        _ => "claude".to_string(),
    }
}

const CLAUDE_ONLY: &str = "This works for Claude Code chats only, for now.";

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
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') // opencode ids are ses_…
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
    if agent() != "claude" {
        return Err(std::io::Error::other(CLAUDE_ONLY));
    }
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
    match agent().as_str() {
        "opencode" => {
            let ok = Command::new(crate::launch::which("opencode").unwrap_or_else(|| "opencode".into())).args(["session", "delete", id]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
            return if ok { Ok(()) } else { Err(std::io::Error::other("opencode could not delete it.")) };
        }
        "codex" => return Err(std::io::Error::other(CLAUDE_ONLY)),
        _ => {}
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
    let a = agent();
    if a != "claude" {
        let here = chat_dir().to_string_lossy().into_owned();
        let mut v: Vec<(String, String, f64)> = crate::agents::load(&a).into_iter().filter(|r| r.cwd == here && !r.title.is_empty()).map(|r| (r.id, r.title, r.updated)).collect();
        v.sort_by(|a, b| b.2.total_cmp(&a.2));
        v.truncate(n);
        return v;
    }
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
    match agent().as_str() {
        "opencode" => return crate::agents::opencode_history(id),
        "codex" => return vec![], // shortcut: Codex keeps messages in rollout files this does not read yet
        _ => {}
    }
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

/// Wait for an agent's answer, at most `secs`; returns its stdout. Stdout is read on its own thread, so a long
/// answer cannot fill the pipe and stall the agent.
fn finish(mut ch: std::process::Child, secs: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = ch.stdout.take().ok_or("The agent did not start.")?;
    let reader = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = out.read_to_end(&mut b);
        b
    });
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while ch.try_wait().map_err(|e| e.to_string())?.is_none() {
        if std::time::Instant::now() > end {
            let _ = ch.kill();
            let _ = ch.wait();
            return Err("The agent took too long to answer.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(reader.join().unwrap_or_default())
}

fn start(mut c: Command, text: &str) -> Result<Vec<u8>, String> {
    let mut ch = c.current_dir(chat_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    if let Some(mut i) = ch.stdin.take() {
        let _ = i.write_all(text.as_bytes()); // dropped here, so the agent sees the end of the message
    }
    finish(ch, 300)
}

/// Send one message to the selected agent; resumes `sid` when given. Returns (session id, reply).
// The chat reads outside text (web pages, mail), so no agent may run commands or edit files from it.
pub fn send(sid: Option<&str>, text: &str) -> Result<(String, String), String> {
    fs::create_dir_all(chat_dir()).map_err(|e| e.to_string())?;
    match agent().as_str() {
        "opencode" => send_opencode(sid, text),
        "codex" => send_codex(sid, text),
        _ => send_claude(sid, text),
    }
}

fn send_claude(sid: Option<&str>, text: &str) -> Result<(String, String), String> {
    let claude = crate::launch::which("claude").ok_or("The chat needs Claude Code (claude) on PATH.")?;
    let mut c = Command::new(claude);
    crate::launch::subscription_only(&mut c);
    // web tools only, and nothing that would ask
    c.args(["-p", "--output-format", "json", "--tools", "WebSearch,WebFetch", "--allowedTools", "WebSearch,WebFetch", "--permission-mode", "dontAsk"]);
    if let Some(s) = sid {
        c.args(["--resume", s]);
    }
    let out = start(c, text)?;
    let v: serde_json::Value = serde_json::from_slice(&out).map_err(|_| "Claude Code did not answer. Is it signed in?".to_string())?;
    let sid = v["session_id"].as_str().ok_or("Claude Code did not answer.")?.to_string();
    Ok((sid, v["result"].as_str().unwrap_or("").trim().to_string()))
}

/// opencode's permission rules for the chat: web tools only.
// shortcut: these rules were accepted by opencode 2.2 but not yet seen blocking a tool; check before relying on them.
const OPENCODE_CHAT_CONFIG: &str = r#"{"permission":{"*":"deny","bash":"deny","shell":"deny","edit":"deny","webfetch":"allow","websearch":"allow"}}"#;

fn send_opencode(sid: Option<&str>, text: &str) -> Result<(String, String), String> {
    let bin = crate::launch::which("opencode").ok_or("The chat needs opencode on PATH.")?;
    let mut c = Command::new(bin);
    // --standalone: a private server, so these permission rules apply instead of the background service's own
    c.args(["run", "--standalone", "--format", "json"]).env("OPENCODE_CONFIG_CONTENT", OPENCODE_CHAT_CONFIG);
    if let Some(s) = sid {
        c.args(["--session", s]);
    }
    parse_opencode(&start(c, text)?)
}

/// opencode prints one JSON event per line: text parts carry the reply, every event carries the session id.
fn parse_opencode(out: &[u8]) -> Result<(String, String), String> {
    let (mut sid, mut parts, mut err) = (None, vec![], None);
    for line in String::from_utf8_lossy(out).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if let Some(s) = v["sessionID"].as_str() {
            sid.get_or_insert_with(|| s.to_string());
        }
        match v["type"].as_str() {
            Some("text") => parts.extend(v["part"]["text"].as_str().map(str::to_string)),
            Some("error") => err = v["error"]["message"].as_str().or(v["error"]["data"]["message"].as_str()).map(str::to_string),
            _ => {}
        }
    }
    let reply = parts.join("\n\n").trim().to_string();
    match (sid, err) {
        (_, Some(e)) if reply.is_empty() => Err(format!("opencode: {e}")),
        (Some(s), _) => Ok((s, reply)),
        _ => Err("opencode did not answer. Run opencode once in a terminal to set it up.".into()),
    }
}

// shortcut: written from Codex's documented --json events, untested (no Codex login here); try it before relying on it.
fn send_codex(sid: Option<&str>, text: &str) -> Result<(String, String), String> {
    let bin = crate::launch::which("codex").ok_or("The chat needs Codex (codex) on PATH.")?;
    let mut c = Command::new(bin);
    // read-only sandbox; ~/Chats is not a git repository
    c.args(["exec", "--json", "--sandbox", "read-only", "--skip-git-repo-check"]);
    if let Some(s) = sid {
        c.args(["resume", s]);
    }
    c.arg("-"); // the message comes on stdin
    let out = start(c, text)?;
    let (mut sid, mut parts, mut err) = (sid.map(str::to_string), vec![], None);
    for line in String::from_utf8_lossy(&out).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match v["type"].as_str() {
            Some("thread.started") => sid = v["thread_id"].as_str().map(str::to_string).or(sid),
            Some("item.completed") if v["item"]["type"] == "agent_message" => parts.extend(v["item"]["text"].as_str().map(str::to_string)),
            Some("error") => err = v["message"].as_str().map(str::to_string),
            Some("turn.failed") => err = v["error"]["message"].as_str().map(str::to_string),
            _ => {}
        }
    }
    let reply = parts.join("\n\n").trim().to_string();
    match (sid, err) {
        (_, Some(e)) if reply.is_empty() => Err(format!("Codex: {e}")),
        (Some(s), _) => Ok((s, reply)),
        _ => Err("Codex did not answer. Is it signed in? Run codex login in a terminal.".into()),
    }
}

/// Move a chat session into `to` (a project folder), so `claude --resume <sid>` there continues it.
/// Copies first and removes the original only from the hub's own staging folder.
pub fn move_session(sid: &str, to: &str) -> std::io::Result<()> {
    if agent() != "claude" {
        return Err(std::io::Error::other(CLAUDE_ONLY));
    }
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
    fn reads_opencode_events() {
        let out = br#"{"type":"step_start","sessionID":"ses_1","part":{}}
{"type":"text","sessionID":"ses_1","part":{"type":"text","text":"pong"}}
{"type":"step_finish","sessionID":"ses_1","part":{}}"#;
        assert_eq!(super::parse_opencode(out), Ok(("ses_1".to_string(), "pong".to_string())));
        let limit = br#"{"type":"error","sessionID":"ses_2","error":{"type":"provider.free-limit","message":"Free usage limit reached."}}"#;
        assert_eq!(super::parse_opencode(limit), Err("opencode: Free usage limit reached.".to_string()));
    }
    #[test]
    fn ids_cannot_leave_the_folder() {
        assert!(super::valid_id("bfb25168-4f08-45e1-a77b-34ad6887475d"));
        assert!(super::valid_id("ses_e586c9b2effeVYuTI4MQX8h2J1"));
        for bad in ["", "../x", "a/b", "a.jsonl", "a b"] {
            assert!(!super::valid_id(bad), "{bad}");
        }
    }
}
