//! Live speech to text with sherpa-onnx (VAD + Parakeet), run as a child process that listens to the default
//! microphone and prints what it hears: `Recognizing: <words>` while a sentence is spoken (re-decoded every
//! 0.2 s), and `[date time] N. <sentence>` once a pause ends it. Nothing is recorded or stored.
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const MODEL: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";

/// What has been heard so far: finished sentences, the sentence being spoken, and whether the mic is open.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Heard {
    pub done: Vec<String>,
    pub now: String,
    pub ready: bool,
    pub ended: bool,
}

pub struct Live {
    child: Child,
    heard: Arc<Mutex<Heard>>,
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The recognizer binary and model, if both are installed (sherpa-onnx beside Kokoro, the model in ~/.local/share/asr).
pub fn installed() -> Option<(PathBuf, PathBuf)> {
    let bin = crate::speech::kokoro()?.0.with_file_name("sherpa-onnx-vad-microphone-simulated-streaming-asr");
    let dir = dirs::data_dir()?.join("asr");
    (bin.is_file() && dir.join(MODEL).join("encoder.int8.onnx").is_file() && dir.join("silero_vad.onnx").is_file()).then_some((bin, dir))
}

/// Drop terminal control sequences (the tool clears the screen between updates).
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next();
                while let Some(d) = it.next() { if d.is_ascii_alphabetic() { break; } }
            }
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// Fold one output line into what has been heard.
fn take(h: &mut Heard, line: &str) {
    let l = plain(line);
    if let Some(t) = l.strip_prefix("Recognizing: ") {
        h.now = t.trim().to_string();
    } else if l.starts_with("Started.") {
        h.ready = true;
    } else if let Some((n, t)) = l.strip_prefix('[').and_then(|r| r.split_once("] ")).and_then(|(_, r)| r.split_once(". ")) {
        let Ok(n) = n.parse::<usize>() else { return };
        let t = t.trim().to_string();
        if n > h.done.len() {
            h.done.push(t);
            h.now.clear(); // the sentence being spoken just became this one
        } else if n > 0 {
            h.done[n - 1] = t;
        }
    }
}

impl Live {
    pub fn start() -> Option<Live> {
        let (bin, dir) = installed()?;
        let m = dir.join(MODEL);
        let arg = |k: &str, p: PathBuf| format!("--{k}={}", p.to_string_lossy());
        // stdbuf: the tool's printf is block-buffered into a pipe; line-buffer it so words arrive as spoken
        let mut child = Command::new("stdbuf")
            .arg("-oL")
            .arg(bin)
            .arg(arg("silero-vad-model", dir.join("silero_vad.onnx")))
            .arg(arg("encoder", m.join("encoder.int8.onnx")))
            .arg(arg("decoder", m.join("decoder.int8.onnx")))
            .arg(arg("joiner", m.join("joiner.int8.onnx")))
            .arg(arg("tokens", m.join("tokens.txt")))
            .arg("--num-threads=4")
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| eprintln!("speech to text: {e}"))
            .ok()?;
        let out = child.stdout.take()?;
        let heard = Arc::new(Mutex::new(Heard::default()));
        let h = heard.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(mut h) = h.lock() { take(&mut h, &line) }
            }
            if let Ok(mut h) = h.lock() { h.ended = true }
        });
        Some(Live { child, heard })
    }

    pub fn heard(&self) -> Heard {
        self.heard.lock().map(|h| h.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_partials_and_sentences() {
        let mut h = Heard::default();
        for l in ["Started. Please speak", "\x1b[H\x1b[2J=== Speech Recognition with Next-gen Kaldi ===", "------------------------------", "Recognizing: hello the"] {
            take(&mut h, l);
        }
        assert!(h.ready);
        assert_eq!(h.now, "hello the");
        take(&mut h, "[2026-11-03 18:20:01] 1. Hello there.");
        assert_eq!((h.done.clone(), h.now.as_str()), (vec!["Hello there.".to_string()], ""));
        // the next screen repeats sentence 1 and shows the new partial
        for l in ["=== Speech Recognition with Next-gen Kaldi ===", "[2026-11-03 18:20:01] 1. Hello there.", "Recognizing: open it"] {
            take(&mut h, l);
        }
        assert_eq!((h.done.len(), h.now.as_str()), (1, "open it"));
    }
}
