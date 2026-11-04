//! One running hub: a second launch (or the listen shortcut) hands its request to the first over a local
//! socket and exits. Messages are single words: "show", "listen".
// Unix: a socket in the runtime dir. Windows: a TCP port on 127.0.0.1, the same idea without a named-pipe crate.
use std::path::PathBuf;

fn path() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir).join("cove.sock")
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

// shortcut: a fixed port; if another program holds it, every launch opens its own window again.
#[cfg(not(unix))]
const ADDR: &str = "127.0.0.1:47219";

#[cfg(not(unix))]
pub fn send(msg: &str) -> bool {
    use std::io::Write;
    let Ok(addr) = ADDR.parse() else { return false };
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).and_then(|mut s| s.write_all(msg.as_bytes())).is_ok()
}

#[cfg(not(unix))]
pub fn serve(on: impl Fn(String) + Send + 'static) -> bool {
    use std::io::Read;
    let Ok(l) = std::net::TcpListener::bind(ADDR) else { return false }; // loopback only: nothing outside this PC
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut s = String::new();
            if c.take(64).read_to_string(&mut s).is_ok() {
                on(s.trim().to_string()); // main only acts on "show" and "listen"
            }
        }
    });
    true
}
