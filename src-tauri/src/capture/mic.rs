//! Mic capture via `cpal`.
//!
//! Capture runs on cpal's own dedicated audio-callback thread — never the
//! UI/hotkey handler thread (docs/mutter-project-plan.md Section 3).
//! Captured samples accumulate in a shared buffer at the device's native
//! format, then `stop()` downmixes to mono and resamples to the 16kHz mono
//! f32 PCM every `TranscriptionEngine` implementation expects (whisper.cpp's
//! fixed input format).
//!
//! Buffer is capped at 120s; hitting the cap does not truncate speech. This
//! module only *signals* the cap via `is_at_cap()` — the session
//! orchestrator is responsible for the actual auto-transcribe-and-continue
//! behavior (Section 3: the primary use case, dictating specs to an AI
//! agent, routinely runs past 2 minutes, so silently truncating would lose
//! exactly the content that motivated this project).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};

pub const MAX_DURATION_SECS: u64 = 120;
const TARGET_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no input device available")]
    NoInputDevice,
    #[error("unsupported input sample format: {0}")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("failed to build input stream: {0}")]
    BuildStream(String),
    #[error("failed to start input stream: {0}")]
    PlayStream(String),
}

struct SharedState {
    /// Interleaved samples at the device's native channel count/rate.
    samples: Mutex<Vec<f32>>,
    source_channels: u16,
    source_sample_rate: u32,
    at_cap: AtomicBool,
}

pub struct MicCapture {
    stream: Option<Stream>,
    state: Option<Arc<SharedState>>,
}

impl MicCapture {
    pub fn new() -> Self {
        Self {
            stream: None,
            state: None,
        }
    }

    /// True once the buffered duration has reached `MAX_DURATION_SECS`. The
    /// caller (session orchestrator) should `stop()`, transcribe the
    /// buffer, then immediately `start()` a new segment.
    pub fn is_at_cap(&self) -> bool {
        self.state
            .as_ref()
            .map(|s| s.at_cap.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// True while a capture stream is active.
    pub fn is_capturing(&self) -> bool {
        self.stream.is_some()
    }

    /// Start capturing on cpal's dedicated audio-callback thread.
    pub fn start(&mut self) -> Result<(), CaptureError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(CaptureError::NoInputDevice)?;
        let config = device
            .default_input_config()
            .map_err(|e| CaptureError::BuildStream(e.to_string()))?;

        let sample_format = config.sample_format();
        let channels = config.channels();
        let sample_rate = config.sample_rate().0;
        let stream_config: cpal::StreamConfig = config.into();

        let state = Arc::new(SharedState {
            samples: Mutex::new(Vec::new()),
            source_channels: channels,
            source_sample_rate: sample_rate,
            at_cap: AtomicBool::new(false),
        });

        let cap_samples = MAX_DURATION_SECS as usize * sample_rate as usize * channels as usize;

        let stream = match sample_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &stream_config, state.clone(), cap_samples)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &stream_config, state.clone(), cap_samples)?
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &stream_config, state.clone(), cap_samples)?
            }
            other => return Err(CaptureError::UnsupportedSampleFormat(other)),
        };

        stream
            .play()
            .map_err(|e| CaptureError::PlayStream(e.to_string()))?;

        self.stream = Some(stream);
        self.state = Some(state);
        Ok(())
    }

    /// Stop capturing and return 16kHz mono f32 PCM, ready for
    /// `TranscriptionEngine::transcribe`.
    pub fn stop(&mut self) -> Vec<f32> {
        self.stream.take(); // dropping the cpal Stream stops it
        let Some(state) = self.state.take() else {
            return Vec::new();
        };
        let raw = std::mem::take(
            &mut *state.samples.lock().expect("capture buffer lock poisoned"),
        );
        let mono = downmix_to_mono(&raw, state.source_channels);
        resample_linear(&mono, state.source_sample_rate, TARGET_SAMPLE_RATE)
    }
}

impl Default for MicCapture {
    fn default() -> Self {
        Self::new()
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    state: Arc<SharedState>,
    cap_samples: usize,
) -> Result<Stream, CaptureError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                // Runs on the OS audio callback thread — there's no caller
                // frame to catch_unwind into here, so this must simply never
                // panic (lock poisoning is the only realistic failure mode,
                // and `expect` surfaces that loudly rather than silently
                // dropping audio).
                if state.at_cap.load(Ordering::Relaxed) {
                    return;
                }
                let mut buf = state.samples.lock().expect("capture buffer lock poisoned");
                if buf.len() >= cap_samples {
                    state.at_cap.store(true, Ordering::Relaxed);
                    return;
                }
                buf.extend(data.iter().map(|&s| f32::from_sample(s)));
                if buf.len() >= cap_samples {
                    state.at_cap.store(true, Ordering::Relaxed);
                }
            },
            |err| tracing::error!(?err, "mic input stream error"),
            None,
        )
        .map_err(|e| CaptureError::BuildStream(e.to_string()))
}

fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let channels = channels as usize;
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Linear-interpolation resampler. Good enough for speech input to Whisper
/// (which itself works on a fairly coarse mel-spectrogram) — not
/// broadcast-quality. Upgrade to a proper sinc resampler (e.g. the `rubato`
/// crate) if the Section 6 accuracy benchmark shows this is actually a
/// bottleneck; not worth the extra dependency until real evidence says so.
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if samples.is_empty() || from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).round() as usize;
    (0..out_len)
        .map(|i| {
            let src_pos = i as f64 / ratio;
            let idx = src_pos.floor() as usize;
            let frac = (src_pos - idx as f64) as f32;
            let a = samples.get(idx).copied().unwrap_or(0.0);
            let b = samples.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_stereo_averages_channels() {
        let stereo = vec![1.0, 3.0, 0.0, 0.0]; // frames: (1,3), (0,0)
        assert_eq!(downmix_to_mono(&stereo, 2), vec![2.0, 0.0]);
    }

    #[test]
    fn downmix_mono_is_passthrough() {
        let mono = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix_to_mono(&mono, 1), mono);
    }

    #[test]
    fn resample_same_rate_is_passthrough() {
        let samples = vec![1.0, 2.0, 3.0];
        assert_eq!(resample_linear(&samples, 16_000, 16_000), samples);
    }

    #[test]
    fn resample_downsamples_by_expected_ratio() {
        let samples: Vec<f32> = (0..480).map(|i| i as f32).collect(); // 48kHz, 10ms
        let out = resample_linear(&samples, 48_000, 16_000);
        assert_eq!(out.len(), 160); // 10ms at 16kHz
    }

    #[test]
    fn resample_empty_is_empty() {
        assert!(resample_linear(&[], 48_000, 16_000).is_empty());
    }

    #[test]
    fn new_capture_is_not_at_cap_and_not_capturing() {
        let capture = MicCapture::new();
        assert!(!capture.is_at_cap());
        assert!(!capture.is_capturing());
    }
}
