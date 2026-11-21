//! High-performance GGML Whisper inference engine using `whisper-rs`.

use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::CoreError;
use crate::text::process_transcription;

/// Configuration options for Whisper model inference.
#[derive(Debug, Clone)]
pub struct WhisperConfig {
    /// Path to the quantized GGML / GGUF model file on disk.
    pub model_path: PathBuf,

    /// Target language code (e.g. "en" for English, None for auto-detection).
    pub language: Option<String>,

    /// Number of CPU threads to allocate for inference.
    pub threads: usize,

    /// Temperature for sampling (0.0 = greedy deterministic, best for accuracy).
    pub temperature: f32,

    /// Whether to translate audio from source language into English.
    pub translate: bool,

    /// Initial prompt for vocabulary biasing and context priming.
    pub initial_prompt: Option<String>,

    /// Whether to apply whitespace/capitalization normalization to output.
    pub enable_normalization: bool,

    /// Whether to convert spoken punctuation commands (e.g. "comma" -> ",") into symbols.
    pub enable_spoken_punctuation: bool,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            language: Some("en".to_string()),
            threads: 4,
            temperature: 0.0,
            translate: false,
            initial_prompt: None,
            enable_normalization: true,
            enable_spoken_punctuation: false,
        }
    }
}

/// A timed segment of transcribed speech.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// The complete output of a transcription run.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptionResult {
    /// The raw verbatim text decoded directly from Whisper.
    pub raw_text: String,

    /// The post-processed text (normalized, spoken punctuation applied if enabled).
    pub text: String,

    /// Time segments with start/end millisecond offsets.
    pub segments: Vec<TranscriptionSegment>,
}

/// Thread-safe Whisper inference engine wrapper.
pub struct WhisperEngine {
    context: WhisperContext,
    config: WhisperConfig,
}

impl WhisperEngine {
    /// Load a quantized GGML model from a file path.
    pub fn load_from_file(config: WhisperConfig) -> Result<Self, CoreError> {
        if !config.model_path.exists() {
            return Err(CoreError::Whisper(format!(
                "Whisper model file does not exist: {}",
                config.model_path.display()
            )));
        }

        let path_str = config
            .model_path
            .to_str()
            .ok_or_else(|| CoreError::Whisper("Invalid non-UTF8 model path".into()))?;

        let params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(path_str, params)
            .map_err(|e| CoreError::Whisper(format!("Failed to load Whisper model: {e}")))?;

        Ok(Self { context, config })
    }

    /// Load a quantized GGML model directly from an in-memory buffer.
    pub fn load_from_buffer(buffer: &[u8], config: WhisperConfig) -> Result<Self, CoreError> {
        let params = WhisperContextParameters::default();
        let context = WhisperContext::new_from_buffer_with_params(buffer, params)
            .map_err(|e| CoreError::Whisper(format!("Failed to load model from buffer: {e}")))?;

        Ok(Self { context, config })
    }

    /// Transcribe 16kHz mono float32 audio samples using default configured prompt.
    pub fn transcribe(&self, audio_16k_mono: &[f32]) -> Result<TranscriptionResult, CoreError> {
        self.transcribe_with_prompt(audio_16k_mono, self.config.initial_prompt.as_deref())
    }

    /// Transcribe 16kHz mono float32 audio samples with an optional custom prompt override.
    pub fn transcribe_with_prompt(
        &self,
        audio_16k_mono: &[f32],
        custom_prompt: Option<&str>,
    ) -> Result<TranscriptionResult, CoreError> {
        if audio_16k_mono.is_empty() {
            return Ok(TranscriptionResult {
                raw_text: String::new(),
                text: String::new(),
                segments: Vec::new(),
            });
        }

        let mut state = self
            .context
            .create_state()
            .map_err(|e| CoreError::Whisper(format!("Failed to create Whisper state: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.config.threads as i32);
        params.set_translate(self.config.translate);
        params.set_temperature(self.config.temperature);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        if let Some(ref lang) = self.config.language {
            params.set_language(Some(lang.as_str()));
        }

        if let Some(prompt) = custom_prompt {
            params.set_initial_prompt(prompt);
        }

        state
            .full(params, audio_16k_mono)
            .map_err(|e| CoreError::Whisper(format!("Whisper inference error: {e}")))?;

        let n_segments = state.full_n_segments();
        let mut segments = Vec::with_capacity(n_segments as usize);
        let mut raw_text_parts = Vec::with_capacity(n_segments as usize);

        for i in 0..n_segments {
            let seg = state
                .get_segment(i)
                .ok_or_else(|| CoreError::Whisper(format!("Failed to read segment {i}")))?;
            let text = seg.to_str_lossy().map_err(|e| {
                CoreError::Whisper(format!("Failed to decode segment text {i}: {e}"))
            })?;
            let start_ms = seg.start_timestamp() * 10;
            let end_ms = seg.end_timestamp() * 10;

            raw_text_parts.push(text.to_string());
            segments.push(TranscriptionSegment {
                text: text.to_string(),
                start_ms,
                end_ms,
            });
        }

        tracing::debug!(
            "Whisper transcription completed. Generated {} segments.",
            segments.len()
        );

        let raw_text = raw_text_parts.join(" ");
        let processed_text = process_transcription(
            &raw_text,
            self.config.enable_normalization,
            self.config.enable_spoken_punctuation,
        );

        Ok(TranscriptionResult {
            raw_text,
            text: processed_text,
            segments,
        })
    }

    /// Access the active engine configuration.
    pub fn config(&self) -> &WhisperConfig {
        &self.config
    }
}
