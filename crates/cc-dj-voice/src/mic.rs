//! Microphone capture via cpal — streams 16-bit PCM to an async channel.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Result of mic initialisation — carries the actual sample rate negotiated with the device.
pub struct MicHandle {
    /// The sample rate the device is actually capturing at.
    pub sample_rate: u32,
    /// Join handle for the capture thread.
    pub thread: std::thread::JoinHandle<()>,
}

/// Preferred sample rate for Gemini Live (16 kHz mono PCM).
const PREFERRED_RATE: u32 = 16_000;

/// Starts microphone capture on a dedicated thread.
///
/// Audio is captured as f32 from the default input device, converted to 16-bit
/// little-endian PCM, and sent over `audio_tx` in chunks. The capture loop runs
/// until `shutdown` is set to `true`.
///
/// Returns a [`MicHandle`] containing the negotiated sample rate and the thread
/// join handle.
pub fn start_mic_capture(
    shutdown: Arc<AtomicBool>,
    audio_tx: mpsc::Sender<Vec<u8>>,
) -> anyhow::Result<MicHandle> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No default input device found"))?;

    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    info!("Using input device: {}", device_name);

    // Pick the best config — prefer 16 kHz mono, fall back to whatever is supported.
    let supported = device.supported_input_configs()?;
    let configs: Vec<_> = supported.collect();

    let (sample_rate, channels) = pick_best_config(&configs);
    info!(
        "Mic config: {}Hz, {} channel(s)",
        sample_rate, channels
    );

    let stream_config = StreamConfig {
        channels,
        sample_rate: SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let rate = sample_rate;
    let ch = channels;
    let shutdown_clone = shutdown.clone();

    let thread = std::thread::Builder::new()
        .name("cc-dj-mic".into())
        .spawn(move || {
            run_capture_loop(&device, &stream_config, ch, shutdown_clone, audio_tx);
        })?;

    Ok(MicHandle {
        sample_rate: rate,
        thread,
    })
}

/// Selects the best supported config from available options.
fn pick_best_config(configs: &[cpal::SupportedStreamConfigRange]) -> (u32, u16) {
    // First try: mono at 16 kHz
    for cfg in configs {
        if cfg.channels() == 1
            && cfg.min_sample_rate().0 <= PREFERRED_RATE
            && cfg.max_sample_rate().0 >= PREFERRED_RATE
        {
            return (PREFERRED_RATE, 1);
        }
    }

    // Second try: stereo at 16 kHz (we'll downmix)
    for cfg in configs {
        if cfg.min_sample_rate().0 <= PREFERRED_RATE
            && cfg.max_sample_rate().0 >= PREFERRED_RATE
        {
            return (PREFERRED_RATE, cfg.channels());
        }
    }

    // Third try: mono at the closest rate above 16 kHz
    for cfg in configs {
        if cfg.channels() == 1 {
            let rate = cfg.min_sample_rate().0.max(PREFERRED_RATE);
            let rate = rate.min(cfg.max_sample_rate().0);
            return (rate, 1);
        }
    }

    // Last resort: first available config at its minimum rate
    if let Some(cfg) = configs.first() {
        let rate = cfg.min_sample_rate().0.max(PREFERRED_RATE);
        let rate = rate.min(cfg.max_sample_rate().0);
        return (rate, cfg.channels());
    }

    // Absolute fallback
    (44_100, 1)
}

/// Runs the cpal input stream until shutdown.
fn run_capture_loop(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: u16,
    shutdown: Arc<AtomicBool>,
    audio_tx: mpsc::Sender<Vec<u8>>,
) {
    let ch = channels as usize;

    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                let pcm = f32_to_i16_pcm(data, ch);
                if !pcm.is_empty() {
                    // Non-blocking send — if the receiver falls behind, drop the chunk.
                    if let Err(_) = audio_tx.try_send(pcm) {
                        debug!("Audio channel full, dropping chunk");
                    }
                }
            },
            move |err| {
                error!("Audio input error: {}", err);
            },
            None, // No timeout
        )
        .expect("Failed to build input stream");

    stream.play().expect("Failed to start audio stream");
    info!("Mic capture running");

    // Spin-wait until shutdown (sleep to avoid busy loop)
    while !shutdown.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    drop(stream);
    info!("Mic capture stopped");
}

/// Converts interleaved f32 samples to mono 16-bit LE PCM bytes.
fn f32_to_i16_pcm(data: &[f32], channels: usize) -> Vec<u8> {
    let frame_count = data.len() / channels;
    let mut bytes = Vec::with_capacity(frame_count * 2);

    for frame in 0..frame_count {
        // Average channels for mono downmix
        let mut sum = 0.0f32;
        for ch in 0..channels {
            sum += data[frame * channels + ch];
        }
        let mono = sum / channels as f32;

        // Clamp and convert to i16
        let clamped = mono.clamp(-1.0, 1.0);
        let sample = (clamped * 32767.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_to_i16_mono() {
        let data = [0.0f32, 1.0, -1.0, 0.5];
        let pcm = f32_to_i16_pcm(&data, 1);
        assert_eq!(pcm.len(), 8); // 4 samples * 2 bytes

        // First sample: 0.0 → 0
        let s0 = i16::from_le_bytes([pcm[0], pcm[1]]);
        assert_eq!(s0, 0);

        // Second sample: 1.0 → 32767
        let s1 = i16::from_le_bytes([pcm[2], pcm[3]]);
        assert_eq!(s1, 32767);

        // Third sample: -1.0 → -32767
        let s2 = i16::from_le_bytes([pcm[4], pcm[5]]);
        assert_eq!(s2, -32767);
    }

    #[test]
    fn test_f32_to_i16_stereo_downmix() {
        // Stereo: L=1.0, R=-1.0 → mono = 0.0
        let data = [1.0f32, -1.0];
        let pcm = f32_to_i16_pcm(&data, 2);
        assert_eq!(pcm.len(), 2); // 1 frame * 2 bytes

        let s0 = i16::from_le_bytes([pcm[0], pcm[1]]);
        assert_eq!(s0, 0);
    }
}
