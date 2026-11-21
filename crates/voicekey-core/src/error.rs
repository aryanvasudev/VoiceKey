use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Audio DSP error: {0}")]
    Audio(String),

    #[error("Resampling error: {0}")]
    Resample(String),

    #[error("Whisper engine error: {0}")]
    Whisper(String),

    #[error("LLM engine error: {0}")]
    Llm(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
