//! Cross-platform audio capture using `cpal` with dynamic device selection and resampling.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Sample, SampleFormat, Stream, SupportedStreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::audio::{downmix_to_mono, AudioResampler, PreRollBuffer};
use crate::error::CoreError;

/// Information about a detected audio input device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub channels: u16,
    pub default_sample_rate: u32,
}

fn get_device_name(dev: &Device) -> String {
    dev.description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| dev.to_string())
}

/// Enumerate audio devices and check input capabilities.
pub struct AudioCaptureManager {
    host: Host,
}

impl AudioCaptureManager {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
        }
    }

    /// List all available audio input devices.
    pub fn list_input_devices(&self) -> Result<Vec<AudioDeviceInfo>, CoreError> {
        let default_name = self
            .host
            .default_input_device()
            .map(|d| get_device_name(&d));

        let devices = self
            .host
            .input_devices()
            .map_err(|e| CoreError::Audio(format!("Failed to query input devices: {e}")))?;

        let mut list = Vec::new();
        for dev in devices {
            let name = get_device_name(&dev);
            let is_default = default_name.as_deref() == Some(&name);
            let (channels, default_sample_rate) = match dev.default_input_config() {
                Ok(cfg) => (cfg.channels(), cfg.sample_rate()),
                Err(_) => (1, 16000),
            };
            list.push(AudioDeviceInfo {
                name,
                is_default,
                channels,
                default_sample_rate,
            });
        }

        Ok(list)
    }

    /// Get the default audio input device if available.
    pub fn default_input_device(&self) -> Result<Option<AudioDeviceInfo>, CoreError> {
        let dev = match self.host.default_input_device() {
            Some(d) => d,
            None => return Ok(None),
        };

        let name = get_device_name(&dev);
        let (channels, default_sample_rate) = match dev.default_input_config() {
            Ok(cfg) => (cfg.channels(), cfg.sample_rate()),
            Err(_) => (1, 16000),
        };

        Ok(Some(AudioDeviceInfo {
            name,
            is_default: true,
            channels,
            default_sample_rate,
        }))
    }
}

impl Default for AudioCaptureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for real-time audio recording.
#[derive(Debug, Clone)]
pub struct AudioRecorderConfig {
    /// Specific microphone device name, or None for system default.
    pub device_name: Option<String>,

    /// Duration of circular pre-roll buffer in milliseconds (default: 200ms).
    pub pre_roll_ms: u32,

    /// Target sample rate for transcription (default: 16,000 Hz).
    pub target_sample_rate: u32,
}

impl Default for AudioRecorderConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            pre_roll_ms: 200,
            target_sample_rate: 16000,
        }
    }
}

/// Thread-safe real-time microphone stream recorder.
pub struct AudioRecorder {
    config: AudioRecorderConfig,
    is_recording: Arc<AtomicBool>,
    rms_bits: Arc<AtomicU32>,
    peak_bits: Arc<AtomicU32>,
    captured_samples: Arc<Mutex<Vec<f32>>>,
    pre_roll_buffer: Arc<Mutex<PreRollBuffer>>,
    stream: Option<Stream>,
    active_sample_rate: u32,
}

impl AudioRecorder {
    pub fn new(config: AudioRecorderConfig) -> Self {
        let pre_roll_buffer = Arc::new(Mutex::new(PreRollBuffer::new(
            config.target_sample_rate,
            config.pre_roll_ms,
        )));

        Self {
            config,
            is_recording: Arc::new(AtomicBool::new(false)),
            rms_bits: Arc::new(AtomicU32::new(0)),
            peak_bits: Arc::new(AtomicU32::new(0)),
            captured_samples: Arc::new(Mutex::new(Vec::new())),
            pre_roll_buffer,
            stream: None,
            active_sample_rate: 16000,
        }
    }

    /// Start audio capture from the configured or default microphone.
    pub fn start(&mut self) -> Result<(), CoreError> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = match &self.config.device_name {
            Some(name) => {
                let mut found = None;
                if let Ok(devices) = host.input_devices() {
                    for d in devices {
                        if get_device_name(&d) == *name {
                            found = Some(d);
                            break;
                        }
                    }
                }
                found.ok_or_else(|| CoreError::Audio(format!("Device not found: {name}")))?
            }
            None => host
                .default_input_device()
                .ok_or_else(|| CoreError::Audio("No default audio input device found".into()))?,
        };

        let stream_config = device
            .default_input_config()
            .map_err(|e| CoreError::Audio(format!("Failed to get default input config: {e}")))?;

        let sample_rate = stream_config.sample_rate();
        let channels = stream_config.channels() as usize;
        self.active_sample_rate = sample_rate;

        // Reset buffers
        {
            let mut buf = self.captured_samples.lock().unwrap();
            buf.clear();
        }
        {
            let mut pre = self.pre_roll_buffer.lock().unwrap();
            *pre = PreRollBuffer::new(sample_rate, self.config.pre_roll_ms);
        }

        self.is_recording.store(true, Ordering::SeqCst);

        let is_rec = Arc::clone(&self.is_recording);
        let captured = Arc::clone(&self.captured_samples);
        let _pre_roll = Arc::clone(&self.pre_roll_buffer);
        let rms_bits = Arc::clone(&self.rms_bits);
        let peak_bits = Arc::clone(&self.peak_bits);

        let err_fn = |err| {
            tracing::error!("Audio stream error: {err}");
        };

        let stream = match stream_config.sample_format() {
            SampleFormat::F32 => Self::build_stream::<f32>(
                &device,
                &stream_config,
                channels,
                is_rec,
                captured,
                rms_bits,
                peak_bits,
                err_fn,
            )?,
            SampleFormat::I16 => Self::build_stream::<i16>(
                &device,
                &stream_config,
                channels,
                is_rec,
                captured,
                rms_bits,
                peak_bits,
                err_fn,
            )?,
            SampleFormat::U16 => Self::build_stream::<u16>(
                &device,
                &stream_config,
                channels,
                is_rec,
                captured,
                rms_bits,
                peak_bits,
                err_fn,
            )?,
            other => {
                return Err(CoreError::Audio(format!(
                    "Unsupported audio sample format: {other:?}"
                )))
            }
        };

        stream
            .play()
            .map_err(|e| CoreError::Audio(format!("Failed to play audio stream: {e}")))?;

        self.stream = Some(stream);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_stream<T>(
        device: &Device,
        config: &SupportedStreamConfig,
        channels: usize,
        is_rec: Arc<AtomicBool>,
        captured: Arc<Mutex<Vec<f32>>>,
        rms_bits: Arc<AtomicU32>,
        peak_bits: Arc<AtomicU32>,
        err_fn: impl FnMut(cpal::Error) + Send + 'static,
    ) -> Result<Stream, CoreError>
    where
        T: Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let stream = device
            .build_input_stream(
                config.config(),
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    if !is_rec.load(Ordering::Relaxed) {
                        return;
                    }

                    // Convert raw samples to f32
                    let float_samples: Vec<f32> =
                        data.iter().map(|&s| f32::from_sample(s)).collect();

                    // Downmix interleaved multi-channel to mono
                    let mono_samples = if channels > 1 {
                        downmix_to_mono(&float_samples, channels)
                    } else {
                        float_samples
                    };

                    if mono_samples.is_empty() {
                        return;
                    }

                    // Compute RMS and peak for level monitoring
                    let mut sum_sq = 0.0f32;
                    let mut peak = 0.0f32;
                    for &s in &mono_samples {
                        let abs = s.abs();
                        if abs > peak {
                            peak = abs;
                        }
                        sum_sq += s * s;
                    }
                    let rms = (sum_sq / mono_samples.len() as f32).sqrt();

                    rms_bits.store(rms.to_bits(), Ordering::Relaxed);
                    peak_bits.store(peak.to_bits(), Ordering::Relaxed);

                    // Append to recording buffer
                    if let Ok(mut buf) = captured.lock() {
                        buf.extend_from_slice(&mono_samples);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| CoreError::Audio(format!("Failed to build input stream: {e}")))?;

        Ok(stream)
    }

    /// Stop recording and return accumulated 16,000 Hz mono audio samples.
    pub fn stop(&mut self) -> Result<Vec<f32>, CoreError> {
        self.is_recording.store(false, Ordering::SeqCst);

        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
        }

        let mut raw_recorded = {
            let mut buf = self.captured_samples.lock().unwrap();
            let data = buf.clone();
            buf.clear();
            data
        };

        let pre_roll_samples = {
            let pre = self.pre_roll_buffer.lock().unwrap();
            pre.get_pre_roll()
        };

        // Prepend pre-roll buffer
        if !pre_roll_samples.is_empty() {
            let mut combined = pre_roll_samples;
            combined.append(&mut raw_recorded);
            raw_recorded = combined;
        }

        // Resample to target 16,000 Hz if recorded at different sample rate
        let final_16k = if self.active_sample_rate != self.config.target_sample_rate {
            let resampler =
                AudioResampler::new(self.active_sample_rate, self.config.target_sample_rate)?;
            resampler.resample(&raw_recorded)?
        } else {
            raw_recorded
        };

        self.rms_bits.store(0, Ordering::Relaxed);
        self.peak_bits.store(0, Ordering::Relaxed);

        Ok(final_16k)
    }

    /// Peek or clone the currently accumulated 16,000 Hz mono audio samples without stopping the stream.
    pub fn peek_samples(&self) -> Result<Vec<f32>, CoreError> {
        let raw = {
            let buf = self.captured_samples.lock().unwrap();
            buf.clone()
        };

        let pre = {
            let pre_buf = self.pre_roll_buffer.lock().unwrap();
            pre_buf.get_pre_roll()
        };

        let mut combined = pre;
        combined.extend(raw);

        if self.active_sample_rate != self.config.target_sample_rate && !combined.is_empty() {
            let resampler =
                AudioResampler::new(self.active_sample_rate, self.config.target_sample_rate)?;
            resampler.resample(&combined)
        } else {
            Ok(combined)
        }
    }

    /// Query the current RMS and peak volume amplitude levels.
    pub fn current_levels(&self) -> (f32, f32) {
        let rms = f32::from_bits(self.rms_bits.load(Ordering::Relaxed));
        let peak = f32::from_bits(self.peak_bits.load(Ordering::Relaxed));
        (rms, peak)
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Configured target sample rate.
    pub fn target_sample_rate(&self) -> u32 {
        self.config.target_sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_and_default_devices_query() {
        let mgr = AudioCaptureManager::new();
        // Querying devices should safely return a Result without panicking
        let devices = mgr.list_input_devices();
        assert!(devices.is_ok());

        let default_dev = mgr.default_input_device();
        assert!(default_dev.is_ok());
    }
}
