//! High-quality audio resampling using `rubato`.

use crate::error::CoreError;
use rubato::{FftFixedInOut, Resampler};

/// Audio resampler converting variable sample rate mono audio into standard 16,000 Hz PCM for Whisper.
pub struct AudioResampler {
    source_rate: u32,
    target_rate: u32,
    chunk_size: usize,
}

impl AudioResampler {
    /// Create a new resampler from `source_rate` to `target_rate`.
    ///
    /// Whisper strictly requires 16,000 Hz.
    pub fn new(source_rate: u32, target_rate: u32) -> Result<Self, CoreError> {
        if source_rate == 0 || target_rate == 0 {
            return Err(CoreError::Resample("Sample rates must be positive".into()));
        }

        // Standard chunk size of 1024 frames for FFT resampler
        let chunk_size = 1024;
        Ok(Self {
            source_rate,
            target_rate,
            chunk_size,
        })
    }

    /// Resample mono audio samples from `source_rate` to `target_rate`.
    ///
    /// If `source_rate == target_rate`, returns a copy of `input`.
    pub fn resample(&self, input: &[f32]) -> Result<Vec<f32>, CoreError> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        if self.source_rate == self.target_rate {
            return Ok(input.to_vec());
        }

        let mut resampler = FftFixedInOut::<f32>::new(
            self.source_rate as usize,
            self.target_rate as usize,
            self.chunk_size,
            1, // mono
        )
        .map_err(|e| CoreError::Resample(format!("Failed to initialize rubato FFT resampler: {e}")))?;

        let mut output = Vec::new();
        let chunk_in_len = resampler.input_frames_next();

        // Process chunk by chunk
        let mut pos = 0;
        while pos + chunk_in_len <= input.len() {
            let chunk = vec![input[pos..pos + chunk_in_len].to_vec()];
            let out_chunks = resampler
                .process(&chunk, None)
                .map_err(|e| CoreError::Resample(format!("Rubato resampling error: {e}")))?;
            if let Some(first_channel) = out_chunks.first() {
                output.extend_from_slice(first_channel);
            }
            pos += chunk_in_len;
        }

        // Process remaining tail samples with zero-padding
        if pos < input.len() {
            let remaining = &input[pos..];
            let mut padded = vec![0.0f32; chunk_in_len];
            padded[..remaining.len()].copy_from_slice(remaining);
            let chunk = vec![padded];
            let out_chunks = resampler
                .process(&chunk, None)
                .map_err(|e| CoreError::Resample(format!("Rubato tail error: {e}")))?;
            if let Some(first_channel) = out_chunks.first() {
                let ratio = self.target_rate as f64 / self.source_rate as f64;
                let valid_out_len = ((remaining.len() as f64) * ratio).round() as usize;
                let take_len = valid_out_len.min(first_channel.len());
                output.extend_from_slice(&first_channel[..take_len]);
            }
        }

        Ok(output)
    }

    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    pub fn target_rate(&self) -> u32 {
        self.target_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_rate_passthrough() {
        let resampler = AudioResampler::new(16000, 16000).unwrap();
        let input = vec![0.1, 0.2, 0.3];
        let out = resampler.resample(&input).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn test_empty_resample() {
        let resampler = AudioResampler::new(48000, 16000).unwrap();
        let out = resampler.resample(&[]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn test_48k_to_16k_resample_ratio() {
        // 48000 -> 16000 is a 3:1 downsample ratio
        let resampler = AudioResampler::new(48000, 16000).unwrap();
        // 1 second of 48000 Hz sine wave
        let samples_in: Vec<f32> = (0..48000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48000.0).sin())
            .collect();

        let out = resampler.resample(&samples_in).unwrap();
        // Output length should be approximately 16000 samples (within 2% due to filter latency/padding)
        let diff = (out.len() as isize - 16000).abs();
        assert!(diff < 350, "Expected ~16000 samples, got {}", out.len());
    }

    #[test]
    fn test_44k1_to_16k_resample_ratio() {
        // Standard CD rate 44.1kHz -> 16kHz
        let resampler = AudioResampler::new(44100, 16000).unwrap();
        let samples_in: Vec<f32> = (0..44100)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 44100.0).sin())
            .collect();

        let out = resampler.resample(&samples_in).unwrap();
        let diff = (out.len() as isize - 16000).abs();
        assert!(diff < 350, "Expected ~16000 samples, got {}", out.len());
    }
}
