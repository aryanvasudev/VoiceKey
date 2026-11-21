//! Low-latency streaming transcription and speculative output engine.
//!
//! Provides progressive windowing, silence hysteresis detection,
//! and real-time partial transcript streaming for sub-100ms perceived latency.

use crate::engine::whisper::{TranscriptionResult, WhisperEngine};
use crate::error::CoreError;
use crate::vad::streaming::{AdaptiveVad, AdaptiveVadConfig, VadEvent};
use std::sync::Arc;

/// Configuration for the streaming transcription window.
#[derive(Debug, Clone, Copy)]
pub struct StreamingConfig {
    /// Step size in samples before triggering a speculative inference (default: 8000 samples = 500ms at 16kHz).
    pub chunk_step_samples: usize,
    /// Minimum speech duration before first speculative run (default: 8000 samples = 500ms).
    pub min_samples_for_inference: usize,
    /// Maximum lookback window for speculative sliding window (default: 160000 samples = 10s).
    pub max_lookback_samples: usize,
    /// Adaptive VAD configuration.
    pub vad_config: AdaptiveVadConfig,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_step_samples: 8000,        // 500ms @ 16kHz
            min_samples_for_inference: 8000, // 500ms @ 16kHz
            max_lookback_samples: 160000,    // 10s @ 16kHz
            vad_config: AdaptiveVadConfig::default(),
        }
    }
}

/// Output event from a streaming chunk step.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamingOutput {
    /// No new text yet (e.g. silence or accumulating samples).
    None,
    /// Speculative partial transcription update for live visualizer.
    Partial { text: String, raw_text: String },
    /// Utterance completed (auto-detected via silence hysteresis).
    Final { result: TranscriptionResult },
}

/// State-machine managing continuous streaming audio, VAD hysteresis,
/// and speculative partial transcriptions.
pub struct StreamingTranscriber {
    engine: Arc<WhisperEngine>,
    config: StreamingConfig,
    vad: AdaptiveVad,
    audio_buffer: Vec<f32>,
    last_speculative_len: usize,
    last_partial_text: String,
    initial_prompt: Option<String>,
}

impl StreamingTranscriber {
    /// Create a new StreamingTranscriber wrapping a shared WhisperEngine.
    pub fn new(
        engine: Arc<WhisperEngine>,
        config: StreamingConfig,
        initial_prompt: Option<String>,
    ) -> Self {
        let vad = AdaptiveVad::new(config.vad_config);
        Self {
            engine,
            config,
            vad,
            audio_buffer: Vec::new(),
            last_speculative_len: 0,
            last_partial_text: String::new(),
            initial_prompt,
        }
    }

    /// Reset internal state and buffers.
    pub fn reset(&mut self) {
        self.vad.reset();
        self.audio_buffer.clear();
        self.last_speculative_len = 0;
        self.last_partial_text.clear();
    }

    /// Whether active speech is currently detected by VAD.
    pub fn is_speaking(&self) -> bool {
        self.vad.is_speaking()
    }

    /// Return the most recent speculative partial text.
    pub fn current_partial_text(&self) -> &str {
        &self.last_partial_text
    }

    /// Feed a frame of 16kHz audio samples into the streaming pipeline.
    /// Returns a [StreamingOutput] containing partial updates or final utterance completion.
    pub fn feed_frame(&mut self, frame: &[f32]) -> Result<StreamingOutput, CoreError> {
        self.audio_buffer.extend_from_slice(frame);
        let vad_event = self.vad.process_frame(frame);

        // Bound memory while detector is idle (retain up to 1 second pre-roll)
        if !self.vad.is_speaking() {
            let max_idle_preroll = 16000;
            if self.audio_buffer.len() > max_idle_preroll {
                let excess = self.audio_buffer.len() - max_idle_preroll;
                self.audio_buffer.drain(0..excess);
                self.last_speculative_len = self.last_speculative_len.saturating_sub(excess);
            }
        }

        match vad_event {
            VadEvent::UtteranceComplete { .. } => {
                // Hysteresis silence timeout reached: auto-commit final utterance!
                let result = match self.finish() {
                    Ok(r) => r,
                    Err(e) => {
                        self.reset();
                        return Err(e);
                    }
                };
                Ok(StreamingOutput::Final { result })
            }
            VadEvent::SpeechStarted
            | VadEvent::Speaking { .. }
            | VadEvent::SilencePending { .. } => {
                let len = self.audio_buffer.len();
                if len >= self.config.min_samples_for_inference
                    && len.saturating_sub(self.last_speculative_len)
                        >= self.config.chunk_step_samples
                {
                    // Run speculative inference on window
                    let start_idx = len.saturating_sub(self.config.max_lookback_samples);
                    let window = &self.audio_buffer[start_idx..];

                    let res = self
                        .engine
                        .transcribe_with_prompt(window, self.initial_prompt.as_deref())?;

                    self.last_speculative_len = len;
                    if res.text != self.last_partial_text {
                        self.last_partial_text = res.text.clone();
                        return Ok(StreamingOutput::Partial {
                            text: res.text,
                            raw_text: res.raw_text,
                        });
                    }
                }
                Ok(StreamingOutput::None)
            }
            VadEvent::Silence => Ok(StreamingOutput::None),
        }
    }

    /// Finalize and transcribe the entire recorded utterance (e.g. on manual hotkey release).
    pub fn finish(&mut self) -> Result<TranscriptionResult, CoreError> {
        if self.audio_buffer.is_empty() {
            self.reset();
            return Ok(TranscriptionResult {
                raw_text: String::new(),
                text: String::new(),
                segments: Vec::new(),
            });
        }

        let res = self
            .engine
            .transcribe_with_prompt(&self.audio_buffer, self.initial_prompt.as_deref())?;

        self.reset();
        Ok(res)
    }
}
