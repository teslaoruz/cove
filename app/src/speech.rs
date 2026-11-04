//! Talking back: text to a WAV with the best voice on this machine (Kokoro via sherpa-onnx, else Piper, else
//! espeak-ng; `say` on macOS, the system voice on Windows), played by the system player. The loudness envelope of the
//! same WAV drives the orb while it speaks.
use crate::settings::cache_dir;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// One synthesised piece of a reply, waiting to be played. Its WAV is deleted when it is dropped.
pub struct Clip {
    env: Vec<f32>, // loudness 0..1 per 20 ms
    wav: PathBuf,
    pub generation: u64,
}

impl Drop for Clip {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.wav);
    }
}

pub struct Speech {
    clip: Clip,
    start: Instant,
    player: Child,
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Stop whatever is being synthesised: pieces of older replies are dropped. Returns the new generation.
pub fn hush() -> u64 {
    GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

impl Speech {
    pub fn level(&self) -> f32 {
        let i = (self.start.elapsed().as_millis() / 20) as usize;
        self.clip.env.get(i).copied().unwrap_or(0.0)
    }
    pub fn done(&mut self) -> bool {
        if self.clip.env.is_empty() {
            return !matches!(self.player.try_wait(), Ok(None)); // no envelope (odd WAV): done when the player is
        }
        self.start.elapsed().as_millis() as usize / 20 >= self.clip.env.len()
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        let _ = self.player.kill(); // a new reply, or talking over it, cuts the old one off
        let _ = self.player.wait(); // reap it, or it lingers as a zombie
    }
}

/// Readable text: no markdown marks, no code blocks, at most ~600 characters.
pub fn speakable(text: &str) -> String {
    let mut out = String::new();
    let mut code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if !code {
            out.push_str(&line.replace(['*', '#', '`', '_', '>'], ""));
            out.push(' ');
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    match out.char_indices().nth(600) {
        Some((i, _)) => format!("{}…", &out[..out[..i].rfind(". ").map(|j| j + 1).unwrap_or(i)]),
        None => out,
    }
}

/// What Settings choose: engine (auto | kokoro | piper | espeak), Kokoro voice, speed (1 = normal).
#[derive(Clone)]
pub struct Voice {
    pub engine: String,
    pub voice: String,
    pub speed: f32,
}

/// Kokoro voices offered in Settings (id, label). The id's index in the model's voices.bin is its speaker id.
pub const KOKORO_VOICES: &[(&str, &str)] = &[
    ("af_heart", "Heart (US, female)"),
    ("af_bella", "Bella (US, female)"),
    ("af_nicole", "Nicole (US, female, soft)"),
    ("af_sarah", "Sarah (US, female)"),
    ("am_michael", "Michael (US, male)"),
    ("am_adam", "Adam (US, male)"),
    ("bf_emma", "Emma (UK, female)"),
    ("bf_isabella", "Isabella (UK, female)"),
    ("bm_george", "George (UK, male)"),
    ("bm_lewis", "Lewis (UK, male)"),
];

/// Speaker ids of kokoro-multi-lang-v1_0, in voices.bin order.
const KOKORO_SIDS: &[&str] = &[
    "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica", "af_kore", "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
    "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael", "am_onyx", "am_puck", "am_santa",
    "bf_alice", "bf_emma", "bf_isabella", "bf_lily", "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
];

/// sherpa-onnx's TTS binary and a Kokoro model (folder, .onnx file) from ~/.local/share/kokoro. The fp32
/// model.onnx is preferred: on x86 CPUs it runs faster than model.int8.onnx.
pub fn kokoro() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let dir = dirs::data_dir()?.join("kokoro");
    let mut bin = None;
    let mut model: Option<(PathBuf, PathBuf)> = None;
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("sherpa-onnx-") && p.join("bin/sherpa-onnx-offline-tts").is_file() {
            bin = Some(p.join("bin/sherpa-onnx-offline-tts"));
        } else if name.starts_with("kokoro-") && p.join("voices.bin").is_file() {
            if p.join("model.onnx").is_file() {
                model = Some((p.clone(), p.join("model.onnx")));
            } else if p.join("model.int8.onnx").is_file() && model.is_none() {
                model = Some((p.clone(), p.join("model.int8.onnx")));
            }
        }
    }
    let (m, f) = model?;
    Some((bin?, m, f))
}

fn kokoro_synth(text: &str, wav: &str, v: &Voice) -> bool {
    let Some((bin, m, model)) = kokoro() else { return false };
    let sid = KOKORO_SIDS.iter().position(|x| *x == v.voice).unwrap_or(3);
    let lex = m.join(if v.voice.starts_with('b') { "lexicon-gb-en.txt" } else { "lexicon-us-en.txt" });
    let mut c = Command::new(bin);
    c.arg(format!("--kokoro-model={}", model.to_string_lossy()))
        .arg(format!("--kokoro-voices={}", m.join("voices.bin").to_string_lossy()))
        .arg(format!("--kokoro-tokens={}", m.join("tokens.txt").to_string_lossy()))
        .arg(format!("--kokoro-lexicon={}", lex.to_string_lossy()))
        .arg(format!("--kokoro-length-scale={}", 1.0 / v.speed.clamp(0.5, 2.0)))
        .arg("--num-threads=8") // fp32 on 8 threads speaks ~4x faster than real time here; more threads were slower
        .arg(format!("--sid={sid}"))
        .arg(format!("--output-filename={wav}"));
    c.arg(text).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

pub fn piper_voice() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("piper");
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "onnx"))
}

fn synth(text: &str, wav: &Path, v: &Voice) -> bool {
    let w = wav.to_string_lossy().into_owned();
    if cfg!(target_os = "macos") {
        return Command::new("say").args(["-o", &w, "--data-format=LEI16@22050", text]).status().is_ok_and(|s| s.success());
    }
    if cfg!(windows) {
        let ps = format!(
            "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('{}'); $s.Speak([Console]::In.ReadToEnd())",
            w.replace('\'', "''")
        );
        return pipe(Command::new("powershell").args(["-NoProfile", "-Command", &ps]), text);
    }
    let any = v.engine == "auto" || v.engine.is_empty();
    if (any || v.engine == "kokoro") && kokoro_synth(text, &w, v) {
        return true;
    }
    if any || v.engine == "piper" {
        if let (Some(p), Some(model)) = (crate::launch::which("piper-tts").or_else(|| crate::launch::which("piper")), piper_voice()) {
            let ls = format!("{}", 1.0 / v.speed.clamp(0.5, 2.0));
            if pipe(Command::new(p).args(["--model", &model.to_string_lossy(), "--length_scale", &ls, "--output_file", &w]), text) {
                return true;
            }
        }
    }
    // espeak-ng: the chosen engine is missing or failed, or it was chosen
    let wpm = format!("{}", (165.0 * v.speed.clamp(0.5, 2.0)) as u32);
    Command::new("espeak-ng").args(["-v", "en-us", "-s", &wpm, "-w", &w, text]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn pipe(c: &mut Command, text: &str) -> bool {
    use std::io::Write;
    let Ok(mut ch) = c.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
    if let Some(mut i) = ch.stdin.take() {
        let _ = i.write_all(text.as_bytes());
    }
    ch.wait().is_ok_and(|s| s.success())
}

/// Loudness per 20 ms of a 16-bit PCM WAV, scaled so speech peaks near 1.
pub fn envelope(bytes: &[u8]) -> Vec<f32> {
    let find = |id: &[u8]| bytes.windows(4).position(|w| w == id);
    let (Some(f), Some(d)) = (find(b"fmt "), find(b"data")) else { return vec![] };
    let u16at = |i: usize| bytes.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).unwrap_or(0) as usize;
    let u32at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0) as usize;
    let (ch, rate, bits) = (u16at(f + 10).max(1), u32at(f + 12).max(1), u16at(f + 22));
    if bits != 16 {
        return vec![];
    }
    let data = &bytes[(d + 8).min(bytes.len())..];
    let frame = rate * ch / 50; // samples per 20 ms
    let mut env: Vec<f32> = data
        .chunks(frame.max(1) * 2)
        .map(|c| {
            let n = c.len() / 2;
            (c.chunks_exact(2).map(|s| (i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).powi(2)).sum::<f32>() / n.max(1) as f32).sqrt()
        })
        .collect();
    let peak = env.iter().cloned().fold(0.0, f32::max).max(1e-4);
    env.iter_mut().for_each(|v| *v = (*v / peak).min(1.0));
    env
}

/// Readable text in pieces that grow: the first sentence alone, so the voice starts at once, then larger
/// pieces that are synthesised while the earlier ones play.
pub fn pieces(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    let mut budget = 30;
    for word in text.split_inclusive(' ') {
        cur.push_str(word);
        let end = word.trim_end().ends_with(['.', '!', '?', ':', ';', '…']);
        if end && cur.len() >= budget {
            out.push(cur.trim().to_string());
            cur.clear();
            budget = budget * 5 / 2;
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Synthesise `text` piece by piece (blocking; call off the UI thread), handing each piece over as soon as it
/// is ready. Stops early when a newer reply (or hush) starts.
pub fn render(text: &str, v: &Voice, generation: u64, mut each: impl FnMut(Clip)) {
    let _ = std::fs::create_dir_all(cache_dir());
    for (k, part) in pieces(&speakable(text)).iter().enumerate() {
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let wav = cache_dir().join(format!("speech-{generation}-{k}.wav"));
        if !synth(part, &wav, v) {
            let _ = std::fs::remove_file(&wav);
            return;
        }
        let env = envelope(&std::fs::read(&wav).unwrap_or_default());
        each(Clip { env, wav, generation });
    }
}

/// Start playing a piece with the system player.
pub fn play(clip: Clip) -> Option<Speech> {
    let w = clip.wav.to_string_lossy().into_owned();
    let player = if cfg!(target_os = "macos") {
        Command::new("afplay").arg(&w).spawn()
    } else if cfg!(windows) {
        Command::new("powershell").args(["-NoProfile", "-Command", &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", w.replace('\'', "''"))]).spawn()
    } else {
        ["pw-play", "paplay", "aplay"].iter().find_map(|p| crate::launch::which(p)).map(|p| Command::new(p).arg(&w).stdout(Stdio::null()).stderr(Stdio::null()).spawn())?
    }
    .ok()?;
    Some(Speech { clip, start: Instant::now(), player })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_piece_is_short_and_nothing_is_lost() {
        let t = "Sure. The audio problem had two causes, both fixed now. First, your speakers are muted, so replies played into silence. Second, the hub lost track of Handy. That is all.";
        let p = pieces(t);
        assert!(p[0].len() < 70 && p.len() >= 2, "{p:?}");
        assert_eq!(p.join(" "), t);
        assert_eq!(pieces("Hi"), ["Hi"]);
    }
    #[test]
    fn markdown_and_code_are_not_read_aloud() {
        assert_eq!(speakable("**Hi** there\n```\nlet x = 1;\n```\n# Done"), "Hi there Done");
    }
    #[test]
    fn envelope_of_a_tone_then_silence() {
        let rate = 1000u32;
        let mut pcm = vec![];
        for i in 0..40 {
            let v: i16 = if i < 20 { if i % 2 == 0 { 8000 } else { -8000 } } else { 0 };
            pcm.extend_from_slice(&v.to_le_bytes());
        }
        let mut w = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        w.extend_from_slice(&pcm);
        let e = envelope(&w);
        assert_eq!(e.len(), 2); // 20 samples per 20 ms frame at 1 kHz
        assert!((e[0] - 1.0).abs() < 1e-3 && e[1] == 0.0);
    }
}
