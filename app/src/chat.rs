//! The hub's own chat: a real session of the selected agent (Claude Code, opencode or Codex) in a staging folder,
//! so a Claude Code chat can later move into a new project.
use crate::scan::{claude_dir, encode};
use crate::settings::cache_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

static DIR: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);
static AGENT: std::sync::RwLock<(String, String)> = std::sync::RwLock::new((String::new(), String::new()));

/// The agent chats go to, and the local model it runs on ("" = its own). Set on start and on every change.
pub fn set_agent(agent: &str, local_model: &str) {
    *AGENT.write().unwrap() = (agent.to_string(), local_model.to_string());
}

fn agent() -> String {
    let a = AGENT.read().unwrap().0.clone();
    if a.is_empty() { "claude".into() } else { a }
}

fn local() -> Option<(Vec<String>, Vec<(&'static str, String)>)> {
    let (a, m) = AGENT.read().unwrap().clone();
    crate::launch::local_model(if a.is_empty() { "claude" } else { &a }, &m)
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
        "codex" => return crate::agents::codex_history(id),
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

/// Run an agent with `text` on stdin and hand each line of its answer to `line` as it arrives; at most `secs`.
fn run(mut c: Command, text: &str, secs: u64, mut line: impl FnMut(&str) + Send + 'static) -> Result<(), String> {
    use std::io::BufRead;
    let mut ch = c.current_dir(chat_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    if let Some(mut i) = ch.stdin.take() {
        let _ = i.write_all(text.as_bytes()); // dropped here, so the agent sees the end of the message
    }
    let out = ch.stdout.take().ok_or("The agent did not start.")?;
    // read on its own thread, so a long answer cannot fill the pipe and stall the agent
    let reader = std::thread::spawn(move || {
        for l in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            line(&l);
        }
    });
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while ch.try_wait().map_err(|e| e.to_string())?.is_none() {
        if std::time::Instant::now() > end {
            let _ = ch.kill();
            let _ = ch.wait();
            return Err("The agent took too long to answer.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = reader.join();
    Ok(())
}

/// Lines and the reply so far, shared between the reading thread and the caller.
type Sink = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

fn collect(c: Command, text: &str, on: &Partial) -> Result<Vec<String>, String> {
    let lines: Sink = Default::default();
    let (l, on) = (lines.clone(), on.clone());
    let mut reply = Reply::default();
    run(c, text, 300, move |line| {
        l.lock().unwrap().push(line.to_string());
        if reply.feed(line) {
            on(&reply.text());
        }
    })?;
    let v = lines.lock().unwrap().clone();
    Ok(v)
}

/// The reply so far, built from the lines an agent prints (each agent's own event format).
#[derive(Default)]
struct Reply {
    parts: Vec<String>,
}

impl Reply {
    /// Take one line; true when the reply grew.
    fn feed(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return false };
        match v["type"].as_str() {
            // Claude Code: text deltas, a new part at each content block
            Some("stream_event") if v["event"]["type"] == "content_block_start" => {
                self.parts.push(String::new());
                false
            }
            Some("stream_event") if v["event"]["delta"]["type"] == "text_delta" => {
                if self.parts.is_empty() {
                    self.parts.push(String::new());
                }
                self.parts.last_mut().unwrap().push_str(v["event"]["delta"]["text"].as_str().unwrap_or(""));
                true
            }
            Some("text") => v["part"]["text"].as_str().map(|t| self.parts.push(t.to_string())).is_some(),
            Some("item.completed") if v["item"]["type"] == "agent_message" => v["item"]["text"].as_str().map(|t| self.parts.push(t.to_string())).is_some(),
            _ => false,
        }
    }

    fn text(&self) -> String {
        self.parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n\n")
    }
}

/// Called with the whole reply so far, each time it grows.
pub type Partial = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// Send one message to the chat agent; resumes `sid` when given. `on` sees the reply as it is written.
/// Returns (session id, reply).
// The chat reads outside text (web pages, mail), so no agent may run commands or edit files from it.
pub fn send(sid: Option<&str>, text: &str, on: Partial) -> Result<(String, String), String> {
    fs::create_dir_all(chat_dir()).map_err(|e| e.to_string())?;
    match agent().as_str() {
        "opencode" => send_opencode(sid, text, &on),
        "codex" => send_codex(sid, text, &on),
        "claude" => send_claude(sid, text, &on),
        _ => Err("Chat works with Claude Code, Codex and opencode. Pick one at the bottom of the sidebar.".into()),
    }
}

fn send_claude(sid: Option<&str>, text: &str, on: &Partial) -> Result<(String, String), String> {
    let claude = crate::launch::which("claude").ok_or("The chat needs Claude Code (claude) on PATH.")?;
    let mut c = Command::new(claude);
    crate::launch::subscription_only(&mut c);
    if let Some((args, env)) = local() {
        c.args(args).envs(env); // a local model: Ollama stands in for Anthropic's API
    }
    // web tools only, and nothing that would ask; stream-json shows the reply as it is written
    c.args(["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--tools", "WebSearch,WebFetch", "--allowedTools", "WebSearch,WebFetch", "--permission-mode", "dontAsk"]);
    if let Some(s) = sid {
        c.args(["--resume", s]);
    }
    let lines = collect(c, text, on)?;
    let result = lines.iter().rev().find_map(|l| serde_json::from_str::<serde_json::Value>(l).ok().filter(|v| v["type"] == "result"));
    let v = result.ok_or_else(|| "Claude Code did not answer. Is it signed in?".to_string())?;
    let sid = v["session_id"].as_str().ok_or("Claude Code did not answer.")?.to_string();
    if v["is_error"] == true {
        return Err(v["result"].as_str().unwrap_or("Claude Code could not answer.").to_string());
    }
    Ok((sid, v["result"].as_str().unwrap_or("").trim().to_string()))
}

/// opencode's permission rules for the chat: web tools only.
// shortcut: these rules were accepted by opencode 2.2 but not yet seen blocking a tool; check before relying on them.
const OPENCODE_CHAT_CONFIG: &str = r#"{"permission":{"*":"deny","bash":"deny","shell":"deny","edit":"deny","webfetch":"allow","websearch":"allow"}}"#;

fn send_opencode(sid: Option<&str>, text: &str, on: &Partial) -> Result<(String, String), String> {
    let bin = crate::launch::which("opencode").ok_or("The chat needs opencode on PATH.")?;
    let mut c = Command::new(bin);
    // --standalone: a private server, so these permission rules apply instead of the background service's own
    let mut config: serde_json::Value = serde_json::from_str(OPENCODE_CHAT_CONFIG).unwrap_or_default();
    c.args(["run", "--standalone", "--format", "json"]);
    if let Some((args, _)) = local() {
        let model = args.get(1).map(|m| m.trim_start_matches("ollama/").to_string()).unwrap_or_default();
        config["provider"] = crate::launch::opencode_ollama(&model)["provider"].clone();
        c.args(args);
    }
    c.env("OPENCODE_CONFIG_CONTENT", config.to_string());
    if let Some(s) = sid {
        c.args(["--session", s]);
    }
    parse_opencode(collect(c, text, on)?.join("\n").as_bytes())
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

fn send_codex(sid: Option<&str>, text: &str, on: &Partial) -> Result<(String, String), String> {
    let bin = crate::launch::which("codex").ok_or("The chat needs Codex (codex) on PATH.")?;
    let mut c = Command::new(bin);
    // read-only sandbox; ~/Chats is not a git repository
    c.arg("exec");
    if let Some((args, _)) = local() {
        c.args(args);
    }
    c.args(["--json", "--sandbox", "read-only", "--skip-git-repo-check"]);
    if let Some(s) = sid {
        c.args(["resume", s]);
    }
    c.arg("-"); // the message comes on stdin
    let out = collect(c, text, on)?.join("\n");
    let (mut sid, mut parts, mut err) = (sid.map(str::to_string), vec![], None);
    for line in out.lines() {
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
    fn reply_follows_claude_deltas() {
        let mut r = super::Reply::default();
        assert!(!r.feed(r#"{"type":"system"}"#));
        assert!(!r.feed(r#"{"type":"stream_event","event":{"type":"content_block_start"}}"#));
        assert!(r.feed(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"one "}}}"#));
        assert!(r.feed(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"two"}}}"#));
        assert_eq!(r.text(), "one two");
        assert!(!r.feed(r#"{"type":"assistant"}"#), "only lines that add text report");
    }
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
