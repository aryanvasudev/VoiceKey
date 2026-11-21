//! Android UniFFI FFI bindings for VoiceKey core engine.

uniffi::setup_scaffolding!();

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use voicekey_core::audio::downmix::downmix_to_mono;
use voicekey_core::audio::resampler::AudioResampler;
use voicekey_core::engine::whisper::{WhisperConfig, WhisperEngine};
use voicekey_core::text::normalizer::normalize_text as core_normalize_text;
use voicekey_core::text::process_transcription as core_process_transcription;
use voicekey_core::text::spoken_punct::apply_spoken_punctuation as core_apply_spoken_punct;
use voicekey_core::vad::EnergyVad;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum VoiceKeyFfiError {
    #[error("Initialization error: {msg}")]
    InitError { msg: String },
    #[error("Transcription error: {msg}")]
    TranscriptionError { msg: String },
    #[error("Audio error: {msg}")]
    AudioError { msg: String },
    #[error("Model not found at: {path}")]
    ModelNotFound { path: String },
    #[error("Internal error: {msg}")]
    InternalError { msg: String },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct TranscriptionSegmentOutput {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct TranscriptionOutput {
    pub raw_text: String,
    pub text: String,
    pub segments: Vec<TranscriptionSegmentOutput>,
}

#[derive(uniffi::Record, Debug, Clone)]
pub struct EngineConfig {
    pub threads: u32,
    pub language: Option<String>,
    pub translate: bool,
    pub enable_normalization: bool,
    pub enable_spoken_punctuation: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            threads: 4,
            language: Some("en".to_string()),
            translate: false,
            enable_normalization: true,
            enable_spoken_punctuation: false,
        }
    }
}

/// Thread-safe VoiceKey Engine instance exposed to Android/Kotlin via UniFFI.
#[derive(uniffi::Object)]
pub struct VoiceKeyEngine {
    inner: Mutex<WhisperEngine>,
}

#[uniffi::export]
impl VoiceKeyEngine {
    /// Initialize a new VoiceKeyEngine with the model at `model_path`.
    #[uniffi::constructor]
    pub fn new(
        model_path: String,
        config: Option<EngineConfig>,
    ) -> Result<Arc<Self>, VoiceKeyFfiError> {
        let path = PathBuf::from(&model_path);
        if !path.exists() {
            return Err(VoiceKeyFfiError::ModelNotFound { path: model_path });
        }

        let cfg = config.unwrap_or_default();
        let whisper_cfg = WhisperConfig {
            model_path: path,
            language: cfg.language,
            threads: cfg.threads.max(1) as usize,
            temperature: 0.0,
            translate: cfg.translate,
            initial_prompt: None,
            enable_normalization: cfg.enable_normalization,
            enable_spoken_punctuation: cfg.enable_spoken_punctuation,
        };

        let engine = WhisperEngine::load_from_file(whisper_cfg)
            .map_err(|e| VoiceKeyFfiError::InitError { msg: e.to_string() })?;

        Ok(Arc::new(Self {
            inner: Mutex::new(engine),
        }))
    }

    /// Transcribe 16kHz mono float32 audio samples with an optional custom prompt.
    pub fn transcribe(
        &self,
        samples: Vec<f32>,
        initial_prompt: Option<String>,
    ) -> Result<TranscriptionOutput, VoiceKeyFfiError> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| VoiceKeyFfiError::InternalError {
                msg: format!("Mutex lock poisoned: {e}"),
            })?;

        let res = guard
            .transcribe_with_prompt(&samples, initial_prompt.as_deref())
            .map_err(|e| VoiceKeyFfiError::TranscriptionError { msg: e.to_string() })?;

        Ok(TranscriptionOutput {
            raw_text: res.raw_text,
            text: res.text,
            segments: res
                .segments
                .into_iter()
                .map(|s| TranscriptionSegmentOutput {
                    text: s.text,
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                })
                .collect(),
        })
    }

    /// Transcribe raw PCM 16-bit bytes (as captured by Android AudioRecord).
    /// Converts, downmixes to mono, and resamples to 16kHz automatically before inference.
    pub fn transcribe_pcm16(
        &self,
        pcm_bytes: Vec<u8>,
        sample_rate: u32,
        channels: u16,
        initial_prompt: Option<String>,
    ) -> Result<TranscriptionOutput, VoiceKeyFfiError> {
        let samples = voicekey_pcm16_to_samples(pcm_bytes, sample_rate, channels)?;
        self.transcribe(samples, initial_prompt)
    }

    /// Check if speech is present in audio samples.
    pub fn has_speech(&self, samples: Vec<f32>) -> bool {
        voicekey_has_speech(samples)
    }

    /// Trim leading and trailing silence from audio samples.
    pub fn trim_silence(&self, samples: Vec<f32>) -> Vec<f32> {
        voicekey_trim_silence(samples)
    }
}

// -----------------------------------------------------------------------------
// Standalone utility functions exported to Kotlin
// -----------------------------------------------------------------------------

/// Get VoiceKey Core version string.
#[uniffi::export]
pub fn voicekey_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if speech activity is detected in the given 16kHz audio samples.
#[uniffi::export]
pub fn voicekey_has_speech(samples: Vec<f32>) -> bool {
    EnergyVad::default().has_speech(&samples)
}

/// Trim leading and trailing silence from 16kHz audio samples.
#[uniffi::export]
pub fn voicekey_trim_silence(samples: Vec<f32>) -> Vec<f32> {
    EnergyVad::default().trim_silence(&samples).to_vec()
}

/// Calculate Root-Mean-Square (RMS) energy of audio samples.
#[uniffi::export]
pub fn voicekey_calculate_rms(samples: Vec<f32>) -> f32 {
    EnergyVad::calculate_rms(&samples)
}

/// Calculate absolute peak amplitude of audio samples.
#[uniffi::export]
pub fn voicekey_calculate_peak(samples: Vec<f32>) -> f32 {
    EnergyVad::calculate_peak(&samples)
}

/// Convert raw 16-bit little-endian PCM bytes to 16kHz mono float32 samples.
/// Supports downmixing multi-channel audio and resampling to 16kHz.
#[uniffi::export]
#[allow(clippy::manual_is_multiple_of, clippy::chunks_exact_to_as_chunks)]
pub fn voicekey_pcm16_to_samples(
    pcm_bytes: Vec<u8>,
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<f32>, VoiceKeyFfiError> {
    if pcm_bytes.len() % 2 != 0 {
        return Err(VoiceKeyFfiError::AudioError {
            msg: "PCM 16-bit byte buffer length must be even".to_string(),
        });
    }

    if channels == 0 {
        return Err(VoiceKeyFfiError::AudioError {
            msg: "Channel count must be at least 1".to_string(),
        });
    }

    let mut samples: Vec<f32> = pcm_bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample_i16 as f32 / 32768.0
        })
        .collect();

    if channels > 1 {
        samples = downmix_to_mono(&samples, channels as usize);
    }

    if sample_rate != 16000 {
        let resampler = AudioResampler::new(sample_rate, 16000)
            .map_err(|e| VoiceKeyFfiError::AudioError { msg: e.to_string() })?;
        samples = resampler
            .resample(&samples)
            .map_err(|e| VoiceKeyFfiError::AudioError { msg: e.to_string() })?;
    }

    Ok(samples)
}

/// Run full text post-processing (spoken punctuation + normalizer).
#[uniffi::export]
pub fn voicekey_process_text(
    raw_text: String,
    enable_normalization: bool,
    enable_spoken_punctuation: bool,
) -> String {
    core_process_transcription(&raw_text, enable_normalization, enable_spoken_punctuation)
}

/// Clean whitespace and normalize capitalization on text.
#[uniffi::export]
pub fn voicekey_normalize_text(text: String) -> String {
    core_normalize_text(&text)
}

/// Convert spoken punctuation keywords ("comma", "period", "new line") to punctuation marks.
#[uniffi::export]
pub fn voicekey_spoken_punctuation(text: String) -> String {
    core_apply_spoken_punct(&text)
}

/// Smart micro-LLM / rule-based transformation for Android.
///
/// Modes:
/// 0: Auto (fast-path unless spoken command prefix like "VoiceKey bullets:" is detected)
/// 1: Clean transcript (strip filler words & stutters)
/// 2: Bullet points
/// 3: Professional tone
/// 4: Casual tone
/// 5: Voice to code
#[uniffi::export]
pub fn voicekey_transform_text(text: String, mode: i32) -> String {
    let forced_mode = match mode {
        1 => Some(voicekey_core::llm::LlmTransformMode::CleanTranscript),
        2 => Some(voicekey_core::llm::LlmTransformMode::BulletPoints),
        3 => Some(voicekey_core::llm::LlmTransformMode::ProfessionalTone),
        4 => Some(voicekey_core::llm::LlmTransformMode::CasualTone),
        5 => Some(voicekey_core::llm::LlmTransformMode::VoiceToCode),
        _ => None,
    };

    voicekey_core::llm::process_with_llm(&text, forced_mode, None).unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(voicekey_version(), "0.2.0");
    }

    #[test]
    fn test_text_processing_ffi() {
        let text = "hello world period this is a test comma right question mark";
        let processed = voicekey_process_text(text.to_string(), true, true);
        assert_eq!(processed, "Hello world. This is a test, right?");

        let normalized = voicekey_normalize_text("   multiple   spaces  ".to_string());
        assert_eq!(normalized, "Multiple spaces");

        let punct = voicekey_spoken_punctuation("one comma two".to_string());
        assert_eq!(punct, "one, two");

        let bullets = voicekey_transform_text(
            "VoiceKey bullets: item one and then item two".to_string(),
            0,
        );
        assert!(bullets.contains("- Item one"));
        assert!(bullets.contains("- Item two"));
    }

    #[test]
    fn test_pcm16_to_samples_conversion() {
        // Create 100 samples of 16-bit PCM at 16000Hz mono (amplitude 16384 -> ~0.5)
        let sample_val: i16 = 16384;
        let bytes = sample_val.to_le_bytes();
        let mut pcm = Vec::new();
        for _ in 0..100 {
            pcm.extend_from_slice(&bytes);
        }

        let samples = voicekey_pcm16_to_samples(pcm, 16000, 1).expect("pcm conversion failed");
        assert_eq!(samples.len(), 100);
        assert!((samples[0] - 0.5).abs() < 0.001);

        let rms = voicekey_calculate_rms(samples.clone());
        let peak = voicekey_calculate_peak(samples);
        assert!((rms - 0.5).abs() < 0.01);
        assert!((peak - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_pcm16_odd_length_returns_error() {
        let pcm = vec![0u8, 1, 2];
        let res = voicekey_pcm16_to_samples(pcm, 16000, 1);
        assert!(res.is_err());
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_vad_functions() {
        let silence = vec![0.0f32; 16000];
        assert!(!voicekey_has_speech(silence));

        // Create speech burst
        let mut speech = vec![0.0f32; 16000];
        for i in 4000..12000 {
            speech[i] = 0.5 * ((i as f32 * 0.1).sin());
        }
        assert!(voicekey_has_speech(speech.clone()));
        let trimmed = voicekey_trim_silence(speech);
        assert!(trimmed.len() < 16000);
    }

    #[test]
    fn test_engine_init_missing_model_returns_model_not_found() {
        let res = VoiceKeyEngine::new("/nonexistent/model.bin".to_string(), None);
        assert!(matches!(res, Err(VoiceKeyFfiError::ModelNotFound { .. })));
    }
}
