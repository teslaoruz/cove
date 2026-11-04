//! Running coding agents: which project, working or idle, and which window to jump to.
use crate::scan::norm;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub root: String,
    pub tool: String,
    pub pid: u32,
    pub state: String, // working | waiting | idle | running
    pub window: Option<String>,
}

fn tool_of(name: &str) -> Option<&'static str> {
    let n = name.trim_end_matches(".exe");
    crate::launch::AGENTS.iter().map(|a| a.0).find(|t| *t == n)
}

/// The agent a process runs: by its own name, or for agents written in JavaScript or Python (Gemini CLI, Copilot,
/// Aider…) by the script the runtime was started with.
fn tool_of_process(name: &str, cmd: &[std::ffi::OsString]) -> Option<&'static str> {
    tool_of(name).or_else(|| {
        let runtime = ["node", "bun", "python", "python3"].iter().any(|r| name.trim_end_matches(".exe").starts_with(r));
        let script = std::path::Path::new(cmd.get(1)?).file_stem()?.to_string_lossy().into_owned();
        runtime.then(|| tool_of(&script)).flatten()
    })
}

/// Longest project root containing `cwd`.
pub fn project_of<'a>(cwd: &str, roots: &'a [String]) -> Option<&'a String> {
    roots.iter().filter(|r| cwd == r.as_str() || cwd.strip_prefix(r.as_str()).is_some_and(|x| x.starts_with('/') || x.starts_with('\\'))).max_by_key(|r| r.len())
}

/// Bloom (an optional Linux status daemon) writes Claude session states; None when it is not running.
pub fn bloom_status() -> Option<Value> {
    let f = dirs::home_dir()?.join(".local/state/claude-tray/status.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(f).ok()?).ok()?;
    let age = chrono::Local::now().timestamp() as f64 - v.get("updated")?.as_f64()?;
    (age < 600.0).then_some(v)
}

/// {pid: window id} from the running compositor, for jumping to an agent's terminal.
pub fn windows() -> HashMap<u32, String> {
    let run = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
    let mut out = HashMap::new();
    if std::env::var_os("NIRI_SOCKET").is_some() {
        for w in run("niri", &["msg", "-j", "windows"]).and_then(|v| v.as_array().cloned()).unwrap_or_default() {
            if let (Some(pid), Some(id)) = (w["pid"].as_u64(), w["id"].as_u64()) {
                out.insert(pid as u32, format!("niri:{id}"));
            }
        }
    } else if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        for w in run("hyprctl", &["clients", "-j"]).and_then(|v| v.as_array().cloned()).unwrap_or_default() {
            if let (Some(pid), Some(a)) = (w["pid"].as_u64(), w["address"].as_str()) {
                out.insert(pid as u32, format!("hypr:{a}"));
            }
        }
    } else if std::env::var_os("SWAYSOCK").is_some() {
        fn walk(n: &Value, out: &mut HashMap<u32, String>) {
            if let Some(pid) = n["pid"].as_u64() {
                out.insert(pid as u32, format!("sway:{pid}"));
            }
            for k in ["nodes", "floating_nodes"] {
                for c in n[k].as_array().into_iter().flatten() {
                    walk(c, out);
                }
            }
        }
        if let Some(t) = run("swaymsg", &["-t", "get_tree"]) {
            walk(&t, &mut out);
        }
    }
    out
}

/// Bring a window to the front. Returns false when there is no way to on this desktop.
pub fn focus(window: &str) -> bool {
    let (kind, id) = window.split_once(':').unwrap_or(("", window));
    let st = match kind {
        "niri" => Command::new("niri").args(["msg", "action", "focus-window", "--id", id]).status(),
        "hypr" => Command::new("hyprctl").args(["dispatch", "focuswindow", &format!("address:{id}")]).status(),
        "sway" => Command::new("swaymsg").arg(format!("[pid={id}] focus")).status(),
        _ => return false,
    };
    st.is_ok_and(|s| s.success())
}

pub struct Watcher {
    sys: System,
}

impl Watcher {
    pub fn new() -> Option<Self> {
        System::new().ok().map(|sys| Watcher { sys })
    }

    /// Agents running inside one of `roots` (the home folder excluded).
    pub fn scan(&mut self, roots: &[String], home: &str) -> (Vec<Agent>, Option<Value>) {
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cwd(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet).with_cpu());
        let status = bloom_status();
        let states: HashMap<u32, String> = status.as_ref().and_then(|s| s["sessions"].as_array().cloned()).unwrap_or_default().iter()
            .filter_map(|s| Some((s["pid"].as_u64().or_else(|| s["pid"].as_str()?.parse().ok())? as u32, s["state"].as_str().unwrap_or("idle").to_string())))
            .collect();
        let wins = windows();
        let mut out = vec![];
        for (pid, p) in self.sys.processes() {
            let Some(tool) = tool_of_process(&p.name().to_string_lossy(), p.cmd()) else { continue };
            let Some(cwd) = p.cwd() else { continue };
            let cwd = norm(&cwd.to_string_lossy());
            let Some(root) = project_of(&cwd, roots) else { continue };
            if root == home {
                continue;
            }
            let pid = pid.as_u32();
            if tool == "claude" && status.is_some() && !states.contains_key(&pid) {
                continue; // headless `claude -p` helpers are not sessions
            }
            // Without Bloom, CPU use tells working from idle.
            let state = states.get(&pid).cloned().unwrap_or_else(|| if p.cpu_usage() > 3.0 { "working".into() } else { "idle".into() });
            let mut w = None;
            let mut q = Some(Pid::from_u32(pid));
            for _ in 0..12 {
                let Some(cur) = q else { break };
                if let Some(id) = wins.get(&cur.as_u32()) {
                    w = Some(id.clone());
                    break;
                }
                q = self.sys.process(cur).and_then(|x| x.parent());
            }
            out.push(Agent { root: root.clone(), tool: tool.into(), pid, state, window: w });
        }
        let rank = |s: &str| match s { "waiting" => 0, "working" => 1, _ => 2 };
        out.sort_by(|a, b| rank(&a.state).cmp(&rank(&b.state)).then(a.root.cmp(&b.root)).then(a.pid.cmp(&b.pid)));
        (out, status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_agents_run_by_node() {
        let cmd = |v: &[&str]| v.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
        assert_eq!(tool_of_process("node", &cmd(&["node", "/usr/lib/node_modules/@google/gemini-cli/bundle/gemini.js"])), Some("gemini"));
        assert_eq!(tool_of_process("claude", &cmd(&["claude"])), Some("claude"));
        assert_eq!(tool_of_process("node", &cmd(&["node", "server.js"])), None);
        assert_eq!(tool_of_process("vim", &cmd(&["vim", "gemini"])), None);
    }
    #[test]
    fn nested_project_wins() {
        let roots = vec!["/data/UNI".to_string(), "/data/UNI/research/geoai".to_string()];
        assert_eq!(project_of("/data/UNI/research/geoai/backend", &roots).map(|s| s.as_str()), Some("/data/UNI/research/geoai"));
        assert_eq!(project_of("/data/UNIX", &roots), None);
    }
    #[test]
    fn windows_exe_names() {
        assert_eq!(tool_of("claude.exe"), Some("claude"));
        assert_eq!(tool_of("node"), None);
    }
}
