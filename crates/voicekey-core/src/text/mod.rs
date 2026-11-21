pub mod normalizer;
pub mod spoken_punct;
pub mod vocabulary;

pub use normalizer::normalize_text;
pub use spoken_punct::apply_spoken_punctuation;
pub use vocabulary::VocabularyPrompt;

/// Process raw transcription text through the post-processing pipeline.
///
/// Features complete parity with Python `voicekey.postprocess.process_transcription`.
pub fn process_transcription(
    raw_text: &str,
    enable_normalization: bool,
    enable_spoken_punctuation: bool,
) -> String {
    if raw_text.is_empty() {
        return String::new();
    }

    let mut result = raw_text.to_string();

    if enable_spoken_punctuation {
        result = apply_spoken_punctuation(&result);
    }

    if enable_normalization {
        result = normalize_text(&result);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_transcription_full_pipeline() {
        let raw = "   hello comma world newline this is voicekey period   ";
        let processed = process_transcription(raw, true, true);
        assert_eq!(processed, "Hello, world\nThis is voicekey.");
    }
}
