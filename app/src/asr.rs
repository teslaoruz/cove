//! Live speech to text, inside Cove with sherpa-onnx: the Silero voice detector cuts the microphone stream into
//! sentences and Parakeet reads them. The recognizer (about 650 MB) stays loaded between talks so listening starts
//! at once: it loads when Cove starts, is let go after a few idle minutes, and loads again on the next talk.
//! The microphone is open only while listening. Nothing is recorded or stored.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sherpa_onnx::{LinearResampler, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, VadModelConfig, VoiceActivityDetector};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const MODEL: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";
const RATE: i32 = 16_000; // what the voice detector and Parakeet expect
const WINDOW: usize = 512; // Silero's window at 16 kHz

/// What has been heard so far: finished sentences, the sentence being spoken, and whether Cove is ready to hear.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Heard {
    pub done: Vec<String>,
    pub now: String,
    pub ready: bool,
    pub ended: bool, // listening stopped by itself (no microphone, or the models failed to load)
}

/// The model folder, if Parakeet and the voice detector are both in ~/.local/share/asr.
pub fn installed() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("asr");
    (dir.join(MODEL).join("encoder.int8.onnx").is_file() && dir.join("silero_vad.onnx").is_file()).then_some(dir)
}

static EARS: Mutex<Option<Arc<OfflineRecognizer>>> = Mutex::new(None);
static LOADING: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static USED: Mutex<Option<Instant>> = Mutex::new(None);

fn touch() {
    *USED.lock().unwrap() = Some(Instant::now());
}

/// True while the recognizer is loading (the box shows it).
pub fn loading() -> bool {
    LOADING.load(Ordering::Relaxed)
}

fn load(dir: &std::path::Path) -> Option<OfflineRecognizer> {
    let m = |f: &str| Some(dir.join(MODEL).join(f).to_string_lossy().into_owned());
    let mut c = OfflineRecognizerConfig::default();
    c.model_config.transducer = OfflineTransducerModelConfig { encoder: m("encoder.int8.onnx"), decoder: m("decoder.int8.onnx"), joiner: m("joiner.int8.onnx") };
    c.model_config.tokens = m("tokens.txt");
    c.model_config.model_type = Some("nemo_transducer".into());
    c.model_config.num_threads = 4;
    OfflineRecognizer::create(&c)
}

/// Load the recognizer in the background; nothing happens when it is loaded, loading, or not installed.
pub fn preload() {
    if EARS.lock().unwrap().is_some() || LOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(dir) = installed() else { return LOADING.store(false, Ordering::SeqCst) };
    touch();
    std::thread::spawn(move || {
        let r = load(&dir);
        FAILED.store(r.is_none(), Ordering::SeqCst);
        *EARS.lock().unwrap() = r.map(Arc::new);
        LOADING.store(false, Ordering::SeqCst);
        touch();
    });
}

/// Let the recognizer go when it has not been used for `idle` and nobody is listening (a talk holds its own copy).
pub fn unload_if_idle(idle: Duration) {
    let stale = USED.lock().unwrap().is_some_and(|t| t.elapsed() > idle);
    let mut e = EARS.lock().unwrap();
    if stale && e.as_ref().is_some_and(|a| Arc::strong_count(a) == 1) {
        *e = None;
    }
}

pub struct Live {
    heard: Arc<Mutex<Heard>>,
    stop: Arc<AtomicBool>,
    _stream: cpal::Stream, // dropping it closes the microphone
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// The default microphone as a stream of mono samples at its own rate.
fn capture(tx: mpsc::Sender<Vec<f32>>) -> Option<(cpal::Stream, i32)> {
    let device = cpal::default_host().default_input_device()?;
    let config = device.default_input_config().ok()?;
    let (rate, ch) = (config.sample_rate() as i32, config.channels().max(1) as usize);
    let err = |e| eprintln!("microphone: {e}");
    macro_rules! stream {
        ($t:ty, $f:expr) => {{
            let tx = tx.clone();
            device.build_input_stream(config.clone().into(), move |d: &[$t], _: &_| { let _ = tx.send(d.chunks(ch).map(|fr| fr.iter().map(|s| $f(*s)).sum::<f32>() / ch as f32).collect()); }, err, None)
        }};
    }
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => stream!(f32, |s: f32| s),
        cpal::SampleFormat::I16 => stream!(i16, |s: i16| s as f32 / 32768.0),
        cpal::SampleFormat::I32 => stream!(i32, |s: i32| s as f32 / 2147483648.0),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some((stream, rate))
}

fn decode(rec: &OfflineRecognizer, samples: &[f32]) -> String {
    let s = rec.create_stream();
    s.accept_waveform(RATE, samples);
    rec.decode(&s);
    s.get_result().map(|r| r.text.trim().to_string()).unwrap_or_default()
}

/// Split 16 kHz audio into Silero windows, keeping the rest for the next call.
fn windows(pending: &mut Vec<f32>) -> Vec<Vec<f32>> {
    let n = pending.len() / WINDOW;
    let w: Vec<Vec<f32>> = pending[..n * WINDOW].chunks(WINDOW).map(<[f32]>::to_vec).collect();
    pending.drain(..n * WINDOW);
    w
}

impl Live {
    pub fn start() -> Option<Live> {
        let dir = installed()?;
        preload();
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let (stream, rate) = capture(tx)?;
        let heard = Arc::new(Mutex::new(Heard::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (h, st) = (heard.clone(), stop.clone());
        std::thread::spawn(move || {
            let end = |h: &Arc<Mutex<Heard>>| h.lock().unwrap().ended = true;
            // wait for the recognizer; what is said meanwhile waits in the channel and is heard once it is ready
            let rec = loop {
                if st.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(r) = EARS.lock().unwrap().clone() {
                    break r;
                }
                if FAILED.load(Ordering::SeqCst) && !loading() {
                    return end(&h);
                }
                preload(); // it was let go while idle
                std::thread::sleep(Duration::from_millis(50));
            };
            let mut vc = VadModelConfig::default();
            vc.silero_vad.model = Some(dir.join("silero_vad.onnx").to_string_lossy().into_owned());
            (vc.silero_vad.threshold, vc.silero_vad.min_silence_duration, vc.silero_vad.min_speech_duration) = (0.5, 0.6, 0.25);
            (vc.silero_vad.window_size, vc.silero_vad.max_speech_duration) = (WINDOW as i32, 20.0);
            (vc.sample_rate, vc.num_threads) = (RATE, 1);
            let Some(vad) = VoiceActivityDetector::create(&vc, 30.0) else { return end(&h) };
            let resampler = (rate != RATE).then(|| LinearResampler::create(rate, RATE)).flatten();
            h.lock().unwrap().ready = true;
            let (mut pending, mut speech, mut partial) = (vec![], vec![], Instant::now());
            while !st.load(Ordering::SeqCst) {
                let chunk = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(c) => c,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => return end(&h),
                };
                pending.extend(resampler.as_ref().map(|r| r.resample(&chunk, false)).unwrap_or(chunk));
                for w in windows(&mut pending) {
                    vad.accept_waveform(&w);
                    if vad.detected() {
                        speech.extend_from_slice(&w);
                    }
                    // a pause ended a sentence: read the whole of it once more, as final
                    while let Some(seg) = vad.front() {
                        let t = decode(&rec, seg.samples());
                        vad.pop();
                        speech.clear();
                        let mut h = h.lock().unwrap();
                        h.now.clear();
                        if !t.is_empty() {
                            h.done.push(t);
                        }
                    }
                }
                // while a sentence is spoken, re-read it a few times a second so words show as they are said
                if !speech.is_empty() && partial.elapsed() > Duration::from_millis(250) {
                    let t = decode(&rec, &speech);
                    h.lock().unwrap().now = t;
                    partial = Instant::now();
                }
                touch();
            }
        });
        Some(Live { heard, stop, _stream: stream })
    }

    pub fn heard(&self) -> Heard {
        self.heard.lock().map(|h| h.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_splits_into_whole_windows() {
        let mut p = vec![0.0f32; WINDOW * 2 + 10];
        let w = windows(&mut p);
        assert_eq!((w.len(), w[0].len(), p.len()), (2, WINDOW, 10));
    }
}
