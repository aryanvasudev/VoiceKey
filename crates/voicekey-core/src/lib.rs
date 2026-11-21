//! `voicekey-core`: High-performance cross-platform audio DSP, VAD, Whisper inference,
//! audio capture, and text post-processing engine for VoiceKey.

pub mod audio;
pub mod engine;
pub mod error;
pub mod text;
pub mod vad;

pub use audio::{
    downmix_to_mono, read_wav, write_wav, AudioCaptureManager, AudioDeviceInfo, AudioRecorder,
    AudioRecorderConfig, AudioResampler, PreRollBuffer,
};
pub use engine::{TranscriptionResult, TranscriptionSegment, WhisperConfig, WhisperEngine};
pub use error::CoreError;
pub use text::{
    apply_spoken_punctuation, normalize_text, process_transcription, VocabularyPrompt,
};
pub use vad::{EnergyVad, VadConfig};

/// Result type alias for VoiceKey core operations.
pub type Result<T> = std::result::Result<T, CoreError>;
