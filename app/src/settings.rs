//! User settings, stored as JSON in the platform config dir (e.g. ~/.config/cove/settings.json).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScanDir {
    pub path: String,
    /// Collection name shown in the sidebar; several dirs may share one.
    pub collection: String,
    /// List every folder inside, or only folders that have agent sessions.
    #[serde(default = "yes")]
    pub all: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub agent: String,        // claude | codex | opencode
    pub terminal: String,     // key of launch::TERMINALS, or "" for the platform default
    pub editor: String,       // command, e.g. "code"
    pub browser: String,      // command, or "" for the system default
    pub files: String,        // file manager command, or "" for the system default
    pub theme: String,        // system | light | dark
    pub glass: String,        // auto | clear | frosted | solid
    pub vault: String,        // Obsidian vault with project notes, "" = none
    pub scan_dirs: Vec<ScanDir>,
    pub pinned: Vec<String>,
    pub hidden: Vec<String>,
    pub sidebar_collapsed: bool,
    pub chat_dir: String,     // where hub chats run, "" = ~/Chats
    pub talkback: bool,       // speak replies in Chat mode
    pub auto_send: bool,      // send a dictated message once Handy has typed it
    pub voice_engine: String, // auto | kokoro | piper | espeak
    pub voice: String,        // Kokoro voice, e.g. af_heart
    pub voice_speed: f32,     // 1 = normal
    pub auto_update: bool,    // install new releases by itself (installed copies only)
    pub local_model: String,  // Ollama model the agent runs on, "" = the agent's own models
    pub sidebar_width: f32,   // logical px
    pub chat_agent: String,   // agent for Chat; "" = the coding agent when it can chat, else Claude Code
    pub zoom: f32,            // page zoom, 1 = 100 %, like a browser
    pub mail: bool,           // Inbox: ask claude for unread mail and events; off until the user turns it on
}

impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let vault = home.join("vault");
        Settings {
            agent: "claude".into(),
            terminal: String::new(),
            editor: "code".into(),
            browser: String::new(),
            files: String::new(),
            theme: "system".into(),
            glass: "auto".into(),
            vault: if vault.join("02-Projects").is_dir() { vault.to_string_lossy().into() } else { String::new() },
            scan_dirs: default_scan_dirs(&home),
            pinned: vec![],
            hidden: vec![],
            sidebar_collapsed: false,
            chat_dir: String::new(),
            talkback: true,
            auto_send: true,
            voice_engine: "auto".into(),
            voice: "af_heart".into(),
            voice_speed: 1.0,
            auto_update: true,
            mail: false,
            zoom: 1.0,
            chat_agent: String::new(),
            sidebar_width: 236.0,
            local_model: String::new(),
        }
    }
}

/// First-run guess: common code folders in the home folder.
fn default_scan_dirs(home: &Path) -> Vec<ScanDir> {
    ["Projects", "projects", "code", "Code", "dev", "src", "repos", "source/repos"]
        .iter()
        .map(|name| home.join(name))
        .filter(|p| p.is_dir())
        .map(|p| ScanDir { path: p.to_string_lossy().into(), collection: "Projects".into(), all: true })
        .collect()
}

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("cove");
    let old = base.join("project-hub"); // the app's name before 0.2
    if !dir.exists() && old.is_dir() {
        let _ = std::fs::rename(&old, &dir);
    }
    dir
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("cove")
}

/// The Python app's config folder; its actions.json / art.json / projects.json are still read.
pub fn legacy_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("claude-launcher")
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read_to_string(config_dir().join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, dir.join("settings.json")) // atomic: a crash never leaves half a file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"agent":"codex"}"#).unwrap();
        assert_eq!(s.agent, "codex");
        assert_eq!(s.theme, "system");
        assert_eq!(s.glass, "auto");
    }
}
