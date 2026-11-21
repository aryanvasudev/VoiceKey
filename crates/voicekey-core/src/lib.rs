//! `voicekey-core`: High-performance cross-platform audio DSP, VAD, Whisper inference,
//! audio capture, input injection, and text post-processing engine for VoiceKey.

pub mod audio;
pub mod engine;
pub mod error;
pub mod hotkey;
pub mod input;
pub mod ipc;
pub mod llm;
pub mod text;
pub mod vad;

pub use audio::{
    downmix_to_mono, read_wav, write_wav, AudioCaptureManager, AudioDeviceInfo, AudioRecorder,
    AudioRecorderConfig, AudioResampler, PreRollBuffer,
};
pub use engine::{
    StreamingConfig, StreamingOutput, StreamingTranscriber, TranscriptionResult,
    TranscriptionSegment, WhisperConfig, WhisperEngine,
};
pub use error::CoreError;
pub use hotkey::{HotkeyEvent, HotkeyMode, HotkeyRouter};
pub use input::{is_active_window_terminal, ClipboardManager, KeyInjector, KNOWN_TERMINAL_CLASSES};
pub use ipc::{get_socket_path, send_ipc_command, IpcCommand, IpcResponse, IpcServer};
pub use llm::{
    parse_spoken_command, process_with_llm, FallbackTransformer, HttpLlmClient, LlmApiType,
    LlmClient, LlmConfig, LlmTransformMode, ParsedVoiceCommand,
};
pub use text::{apply_spoken_punctuation, normalize_text, process_transcription, VocabularyPrompt};
pub use vad::{AdaptiveVad, AdaptiveVadConfig, EnergyVad, VadConfig, VadEvent};

/// Result type alias for VoiceKey core operations.
pub type Result<T> = std::result::Result<T, CoreError>;
