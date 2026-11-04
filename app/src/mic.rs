//! Microphone loudness for the orb while the hub listens. Only a level (0..1) leaves the audio callback;
//! nothing is recorded or stored. The stream is open only while listening.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct Mic {
    _stream: cpal::Stream, // dropping it closes the microphone
    level: Arc<AtomicU32>,
}

fn rms<T: Copy>(data: &[T], f: impl Fn(T) -> f32) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    (data.iter().map(|s| f(*s).powi(2)).sum::<f32>() / data.len() as f32).sqrt()
}

impl Mic {
    pub fn open() -> Option<Mic> {
        let device = cpal::default_host().default_input_device()?;
        let config = device.default_input_config().ok()?;
        let level = Arc::new(AtomicU32::new(0));
        let l = level.clone();
        // speech RMS is ~0.02–0.2; scale so normal talking reaches ~0.6–1
        let put = move |v: f32| l.store((v * 6.0).min(1.0).to_bits(), Ordering::Relaxed);
        let err = |e| eprintln!("microphone: {e}");
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(config.into(), move |d: &[f32], _: &_| put(rms(d, |s| s)), err, None),
            SampleFormat::I16 => device.build_input_stream(config.into(), move |d: &[i16], _: &_| put(rms(d, |s| s as f32 / 32768.0)), err, None),
            SampleFormat::I32 => device.build_input_stream(config.into(), move |d: &[i32], _: &_| put(rms(d, |s| s as f32 / 2147483648.0)), err, None),
            _ => return None,
        }
        .ok()?;
        stream.play().ok()?;
        Some(Mic { _stream: stream, level })
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rms_of_a_square_wave_is_its_amplitude() {
        assert!((super::rms(&[0.5f32, -0.5, 0.5, -0.5], |s| s) - 0.5).abs() < 1e-6);
    }
}
