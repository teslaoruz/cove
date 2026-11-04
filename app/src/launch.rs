//! Starting things: a coding agent in a terminal, the editor, the file manager, the browser.
use crate::settings::Settings;
use std::path::Path;
use std::process::{Command, Stdio};

/// (key, label, binary) of terminals the app knows how to start in a folder.
pub const TERMINALS: &[(&str, &str, &str)] = &[
    ("foot", "foot", "foot"),
    ("kitty", "kitty", "kitty"),
    ("alacritty", "Alacritty", "alacritty"),
    ("wezterm", "WezTerm", "wezterm"),
    ("ghostty", "Ghostty", "ghostty"),
    ("konsole", "Konsole", "konsole"),
    ("gnome-terminal", "GNOME Terminal", "gnome-terminal"),
    ("ptyxis", "Ptyxis", "ptyxis"),
    ("xterm", "xterm", "xterm"),
    ("wt", "Windows Terminal", "wt.exe"),
    ("terminal-app", "Terminal", "open"),
];
pub const EDITORS: &[(&str, &str)] = &[("code", "VS Code"), ("codium", "VSCodium"), ("cursor", "Cursor"), ("zed", "Zed"), ("subl", "Sublime Text"), ("nvim", "Neovim"), ("idea", "IntelliJ IDEA")];
pub const BROWSERS: &[(&str, &str)] = &[("", "System default"), ("google-chrome-stable", "Chrome"), ("firefox", "Firefox"), ("chromium", "Chromium"), ("brave", "Brave"), ("microsoft-edge", "Edge")];
pub const FILE_MANAGERS: &[(&str, &str)] = &[("", "System default"), ("nautilus", "Files"), ("dolphin", "Dolphin"), ("thunar", "Thunar"), ("nemo", "Nemo"), ("pcmanfm", "PCManFM")];
pub const AGENTS: &[(&str, &str)] = &[("claude", "Claude Code"), ("codex", "Codex"), ("opencode", "opencode")];

pub fn which(bin: &str) -> Option<std::path::PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &["", ".exe", ".cmd", ".bat"] } else { &[""] };
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(h) = dirs::home_dir() {
        dirs.push(h.join(".local/bin")); // the desktop session's PATH often lacks it
    }
    dirs.into_iter().flat_map(|d| exts.iter().map(move |e| d.join(format!("{bin}{e}")))).find(|p| p.is_file())
}

pub fn installed(list: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    list.iter().copied().filter(|(bin, _)| bin.is_empty() || which(bin).is_some()).collect()
}

pub fn installed_terminals() -> Vec<(&'static str, &'static str)> {
    TERMINALS
        .iter()
        .filter(|(k, _, bin)| if *k == "terminal-app" { cfg!(target_os = "macos") } else { which(bin).is_some() })
        .map(|(k, l, _)| (*k, *l))
        .collect()
}

fn default_terminal() -> &'static str {
    installed_terminals().first().map(|t| t.0).unwrap_or(if cfg!(windows) { "cmd" } else { "xterm" })
}

/// Quote one argument for the shell the terminal runs: POSIX sh, or cmd.exe on Windows.
pub fn sh_quote(s: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

pub enum Mode<'a> {
    New(Option<&'a str>), // optional first prompt
    Continue,
    ContinueWith(&'a str), // continue the last session with a new message
    Resume(&'a str),
}

/// Command line that starts `agent` in the given mode.
pub fn agent_cmd(agent: &str, mode: &Mode) -> String {
    let exe = which(agent).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| agent.to_string());
    let exe = sh_quote(&exe);
    let args = match (agent, mode) {
        ("codex", Mode::Continue) => " resume --last".to_string(),
        ("codex", Mode::ContinueWith(p)) => format!(" resume --last {}", sh_quote(p)),
        ("opencode", Mode::ContinueWith(p)) => format!(" --continue --prompt {}", sh_quote(p)),
        (_, Mode::ContinueWith(p)) => format!(" --continue {}", sh_quote(p)),
        ("codex", Mode::New(Some(p))) => format!(" {}", sh_quote(p)),
        ("opencode", Mode::Continue) => " --continue".into(),
        ("opencode", Mode::New(Some(p))) => format!(" --prompt {}", sh_quote(p)),
        (_, Mode::Continue) => " --continue".into(),
        ("codex", Mode::Resume(id)) => format!(" resume {}", sh_quote(id)),
        ("opencode", Mode::Resume(id)) => format!(" --session {}", sh_quote(id)),
        (_, Mode::Resume(id)) => format!(" --resume {}", sh_quote(id)),
        (_, Mode::New(Some(p))) => format!(" {}", sh_quote(p)),
        _ => String::new(),
    };
    format!("{exe}{args}")
}

/// Make `claude` use the claude.ai subscription login, never an API key that happens to be in the environment.
pub fn subscription_only(c: &mut Command) {
    c.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN");
}

fn spawn(mut c: Command) -> std::io::Result<()> {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0); // the terminal outlives the hub
    }
    c.spawn().map(|_| ())
}

/// Open a terminal in `dir`, running `cmd` (a shell command line) if given, keeping the window open on failure.
pub fn terminal(s: &Settings, dir: &str, cmd: Option<&str>) -> std::io::Result<()> {
    let key = if s.terminal.is_empty() { default_terminal() } else { s.terminal.as_str() };
    #[cfg(windows)]
    {
        let mut c = if key == "wt" { let mut c = Command::new("wt.exe"); c.args(["-d", dir, "cmd"]); c } else { let mut c = Command::new("cmd"); c.args(["/c", "start", "", "cmd"]); c.current_dir(dir); c };
        if let Some(cmd) = cmd { c.args(["/k", cmd]); }
        return spawn(c);
    }
    #[cfg(target_os = "macos")]
    if key == "terminal-app" {
        let script = format!("cd {} && {}", sh_quote(dir), cmd.unwrap_or("clear"));
        let mut c = Command::new("osascript");
        c.args(["-e", &format!("tell application \"Terminal\" to do script {}", applescript_quote(&script)), "-e", "tell application \"Terminal\" to activate"]);
        return spawn(c);
    }
    let wrapped = cmd.map(|cmd| format!("{cmd}; s=$?; [ $s -eq 0 ] || {{ echo \"exited $s - press Enter\"; read _; }}"));
    let bin = TERMINALS.iter().find(|t| t.0 == key).map(|t| t.2).unwrap_or(key);
    let mut c = Command::new(bin);
    let run: &[&str] = match key {
        "foot" => { c.args(["-D", dir]); &[] }
        "kitty" => { c.args(["--directory", dir]); &[] }
        "alacritty" => { c.args(["--working-directory", dir]); &["-e"] }
        "wezterm" => { c.args(["start", "--cwd", dir]); &["--"] }
        "ghostty" => { c.arg(format!("--working-directory={dir}")); &["-e"] }
        "konsole" => { c.args(["--workdir", dir]); &["-e"] }
        "gnome-terminal" | "ptyxis" => { c.arg(format!("--working-directory={dir}")); &["--"] }
        _ => { c.current_dir(dir); &["-e"] }
    };
    if let Some(w) = wrapped {
        c.args(run).args(["sh", "-c", &w]);
    }
    if Path::new(dir).is_dir() {
        c.current_dir(dir);
    }
    spawn(c)
}

#[cfg(target_os = "macos")]
fn applescript_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn agent(s: &Settings, dir: &str, mode: Mode) -> std::io::Result<()> {
    terminal(s, dir, Some(&agent_cmd(&s.agent, &mode)))
}

/// Open a path or URL with the system handler.
pub fn system_open(target: &str) -> std::io::Result<()> {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", "start", "", target]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(target);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    c.stdin(Stdio::null());
    spawn(c)
}

pub fn editor(s: &Settings, path: &str) -> std::io::Result<()> {
    if s.editor == "nvim" {
        return terminal(s, path, Some("nvim ."));
    }
    let mut c = Command::new(which(&s.editor).unwrap_or_else(|| s.editor.clone().into()));
    c.arg(path);
    spawn(c)
}

pub fn files(s: &Settings, path: &str) -> std::io::Result<()> {
    if s.files.is_empty() {
        return system_open(path);
    }
    let mut c = Command::new(&s.files);
    c.arg(path);
    spawn(c)
}

pub fn browser(s: &Settings, url: &str) -> std::io::Result<()> {
    if s.browser.is_empty() {
        return system_open(url);
    }
    let mut c = Command::new(&s.browser);
    c.arg(url);
    spawn(c)
}

/// GitHub web URL of a repo's origin remote, if any.
pub fn github_url(root: &str) -> Option<String> {
    let cfg = std::fs::read_to_string(Path::new(root).join(".git/config")).ok()?;
    for line in cfg.lines() {
        let l = line.trim();
        if let Some(u) = l.strip_prefix("url = ").or_else(|| l.strip_prefix("url=")) {
            let u = u.trim();
            let rest = u.strip_prefix("git@github.com:").or_else(|| u.strip_prefix("https://github.com/"))?;
            return Some(format!("https://github.com/{}", rest.trim_end_matches(".git")));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn quoting_survives_single_quotes() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }
    #[test]
    fn agent_modes() {
        assert!(agent_cmd("claude", &Mode::Resume("abc")).ends_with(" --resume 'abc'"));
        assert!(agent_cmd("codex", &Mode::Resume("abc")).ends_with(" resume 'abc'"));
        assert!(agent_cmd("opencode", &Mode::Resume("ses_1")).ends_with(" --session 'ses_1'"));
        assert!(agent_cmd("codex", &Mode::Continue).ends_with(" resume --last"));
        assert!(agent_cmd("opencode", &Mode::New(Some("hi"))).ends_with(" --prompt 'hi'"));
        assert!(!agent_cmd("claude", &Mode::New(None)).contains("--"));
        assert!(agent_cmd("claude", &Mode::ContinueWith("go on")).ends_with(" --continue 'go on'"));
    }
}
