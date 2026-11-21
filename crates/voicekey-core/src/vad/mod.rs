//! Dual Voice Activity Detection (VAD) using energy gating and silence trimming.

pub mod streaming;

pub use streaming::{AdaptiveVad, AdaptiveVadConfig, VadEvent};

/// Configuration for energy-based Voice Activity Detection.
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Minimum RMS amplitude threshold to classify a frame as active speech.
    /// Default is 0.005 (~ -46 dBFS).
    pub energy_threshold: f32,

    /// Number of audio samples per VAD evaluation frame (e.g., 512 samples = 32ms at 16kHz).
    pub frame_size: usize,

    /// Number of frames of padding to retain around speech boundaries to preserve breath/inflection.
    pub padding_frames: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            energy_threshold: 0.0002,
            frame_size: 512,
            padding_frames: 3,
        }
    }
}

/// Voice Activity Detector and silence trimmer.
#[derive(Debug, Clone)]
pub struct EnergyVad {
    config: VadConfig,
}

impl EnergyVad {
    /// Create a new EnergyVad with the given configuration.
    pub fn new(config: VadConfig) -> Self {
        Self { config }
    }

    /// Calculate the Root Mean Square (RMS) volume amplitude of audio samples.
    pub fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    /// Calculate the peak absolute amplitude of audio samples.
    pub fn calculate_peak(samples: &[f32]) -> f32 {
        samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |acc, val| acc.max(val))
    }

    /// Check if the overall audio contains active voice speech above threshold.
    ///
    /// Returns false if audio is completely silent or below threshold, which
    /// prevents Whisper from hallucinating subtitles on empty microphone recordings.
    pub fn has_speech(&self, samples: &[f32]) -> bool {
        if samples.is_empty() {
            return false;
        }

        let rms = Self::calculate_rms(samples);
        let peak = Self::calculate_peak(samples);
        if rms >= self.config.energy_threshold || peak >= 0.001 {
            return true;
        }

        // Check if any frame exceeds the energy threshold
        for chunk in samples.chunks(self.config.frame_size) {
            if Self::calculate_rms(chunk) >= self.config.energy_threshold {
                return true;
            }
        }
        false
    }

    /// Trim leading and trailing silence from an audio buffer while preserving padding.
    pub fn trim_silence<'a>(&self, samples: &'a [f32]) -> &'a [f32] {
        if samples.is_empty() {
            return samples;
        }

        let frame_size = self.config.frame_size;
        let num_frames = samples.len().div_ceil(frame_size);

        let mut first_speech_frame = None;
        let mut last_speech_frame = None;

        for (idx, chunk) in samples.chunks(frame_size).enumerate() {
            if Self::calculate_rms(chunk) >= self.config.energy_threshold {
                if first_speech_frame.is_none() {
                    first_speech_frame = Some(idx);
                }
                last_speech_frame = Some(idx);
            }
        }

        match (first_speech_frame, last_speech_frame) {
            (Some(first), Some(last)) => {
                let pad = self.config.padding_frames;
                let start_frame = first.saturating_sub(pad);
                let end_frame = (last + 1 + pad).min(num_frames);

                let start_idx = (start_frame * frame_size).min(samples.len());
                let end_idx = (end_frame * frame_size).min(samples.len());

                &samples[start_idx..end_idx]
            }
            _ => &[], // Pure silence
        }
    }

    pub fn config(&self) -> &VadConfig {
        &self.config
    }
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self::new(VadConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_and_peak() {
        let silent = vec![0.0f32; 100];
        assert_eq!(EnergyVad::calculate_rms(&silent), 0.0);
        assert_eq!(EnergyVad::calculate_peak(&silent), 0.0);

        let constant = vec![0.5f32; 100];
        assert!((EnergyVad::calculate_rms(&constant) - 0.5).abs() < 1e-6);
        assert!((EnergyVad::calculate_peak(&constant) - 0.5).abs() < 1e-6);

        let wave = vec![-0.8f32, 0.4f32];
        assert_eq!(EnergyVad::calculate_peak(&wave), 0.8);
    }

    #[test]
    fn test_has_speech() {
        let vad = EnergyVad::default();

        let silence = vec![0.0001f32; 16000]; // 1 second of near-zero noise
        assert!(!vad.has_speech(&silence));

        let mut with_speech = vec![0.0f32; 16000];
        // Inject a 200ms burst of speech in the middle
        for s in with_speech[4000..7200].iter_mut() {
            *s = 0.2;
        }
        assert!(vad.has_speech(&with_speech));
    }

    #[test]
    fn test_trim_silence() {
        let config = VadConfig {
            energy_threshold: 0.05,
            frame_size: 10,
            padding_frames: 1, // 1 frame = 10 samples padding
        };
        let vad = EnergyVad::new(config);

        // 30 silent samples, 20 loud speech samples, 30 silent samples
        let mut audio = vec![0.0f32; 80];
        for s in audio[30..50].iter_mut() {
            *s = 0.5;
        }

        let trimmed = vad.trim_silence(&audio);
        // Speech was at frames 3 and 4 (indices 30..50).
        // With 1 frame padding: start frame 2 (idx 20), end frame 4+1+1 = 6 (idx 60).
        assert_eq!(trimmed.len(), 40);
        assert_eq!(trimmed, &audio[20..60]);
    }

    #[test]
    fn test_trim_pure_silence() {
        let vad = EnergyVad::default();
        let silence = vec![0.0f32; 1000];
        let trimmed = vad.trim_silence(&silence);
        assert!(trimmed.is_empty());
    }
}
