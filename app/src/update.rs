//! Self-update. Once a day, a copy that the installer put in place asks GitHub for the newest release. If it is
//! newer, that release's own installer runs in the background: it checks the SHA-256 and swaps the binary, and the
//! new version starts with the next launch. Copies built from source never update themselves.
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const REPO: &str = "teslaoruz/cove";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the installer puts the binary (install.sh / install.ps1).
fn install_path() -> Option<PathBuf> {
    if cfg!(windows) {
        Some(dirs::data_local_dir()?.join("Programs").join("Cove").join("cove.exe"))
    } else {
        Some(dirs::home_dir()?.join(".local/bin/cove"))
    }
}

/// True if this running copy is the one the installer manages.
pub fn managed() -> bool {
    let (Ok(me), Some(p)) = (std::env::current_exe(), install_path()) else { return false };
    let canon = |p: &PathBuf| p.canonicalize().unwrap_or_else(|_| p.clone());
    canon(&me) == canon(&p)
}

/// "v1.2.3" newer than "1.2.0"? Anything that does not parse is never newer.
pub fn newer(tag: &str, current: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> { s.trim().trim_start_matches('v').split('.').map(|x| x.parse().ok()).collect() };
    matches!((parse(tag), parse(current)), (Some(a), Some(b)) if a > b)
}

fn cmd(bin: &str) -> Command {
    let mut c = Command::new(bin);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashes up
    }
    c
}

/// Tag of the newest release, through curl (part of Windows 10 and later, and of almost every Linux).
fn latest_tag() -> Option<String> {
    let mut c = cmd("curl");
    c.stdout(Stdio::piped()).args(["-fsSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json"]);
    let out = c.arg(format!("https://api.github.com/repos/{REPO}/releases/latest")).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v["tag_name"].as_str().map(str::to_string)
}

/// Run the installer of release `tag`. True once it has finished without error.
fn install(tag: &str) -> bool {
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    if cfg!(windows) {
        let ps = format!("& {{ $ErrorActionPreference = 'Stop'; irm '{base}/install.ps1' | iex }}");
        return cmd("powershell").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps]).status().is_ok_and(|s| s.success());
    }
    // download first, so a failed download is not mistaken for a finished install
    let script = std::env::temp_dir().join(format!("cove-install-{}.sh", std::process::id()));
    let ok = cmd("curl").args(["-fsSL", "--max-time", "120", "-o"]).arg(&script).arg(format!("{base}/install.sh")).status().is_ok_and(|s| s.success())
        && cmd("sh").arg(&script).env("COVE_VERSION", tag).status().is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&script);
    ok
}

/// On Windows the installer moves the running exe aside to replace it; remove that leftover.
pub fn clean_up() {
    if let Ok(me) = std::env::current_exe() {
        let _ = std::fs::remove_file(me.with_extension("exe.old"));
    }
}

/// Check now and then once a day; `enabled` is asked before each check, `done` gets the installed tag.
pub fn watch(enabled: impl Fn() -> bool + Send + 'static, done: impl Fn(String) + Send + 'static) {
    if !managed() {
        return;
    }
    std::thread::spawn(move || loop {
        if enabled() {
            if let Some(tag) = latest_tag().filter(|t| newer(t, VERSION)) {
                if install(&tag) {
                    return done(tag);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(24 * 3600));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_by_number() {
        assert!(newer("v0.10.0", "0.9.3"));
        assert!(newer("v1.0.0", "0.2.0"));
        assert!(!newer("v0.2.0", "0.2.0"));
        assert!(!newer("v0.1.9", "0.2.0"));
        assert!(!newer("nightly", "0.2.0"));
    }
}
