//! One running hub: a second launch (or the listen shortcut) hands its request to the first over a local
//! socket and exits. Messages are single words: "show", "listen".
// shortcut: Unix sockets only; on Windows every launch opens its own window. Upgrade with a named pipe.
use std::path::PathBuf;

fn path() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir).join("project-hub.sock")
}

/// Hand `msg` to a running hub. True if one took it.
#[cfg(unix)]
pub fn send(msg: &str) -> bool {
    use std::io::Write;
    std::os::unix::net::UnixStream::connect(path()).and_then(|mut s| s.write_all(msg.as_bytes())).is_ok()
}

/// Become the running hub: `on` gets each message on a background thread. False if another hub already serves.
#[cfg(unix)]
pub fn serve(on: impl Fn(String) + Send + 'static) -> bool {
    use std::io::Read;
    use std::os::unix::net::{UnixListener, UnixStream};
    let p = path();
    if UnixStream::connect(&p).is_ok() {
        return false; // a live hub owns it
    }
    let _ = std::fs::remove_file(&p); // stale socket from a crash
    let Ok(l) = UnixListener::bind(&p) else { return false };
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut s = String::new();
            if c.take(64).read_to_string(&mut s).is_ok() {
                on(s.trim().to_string());
            }
        }
    });
    true
}

#[cfg(not(unix))]
pub fn send(_msg: &str) -> bool {
    false
}

#[cfg(not(unix))]
pub fn serve(_on: impl Fn(String) + Send + 'static) -> bool {
    true
}
