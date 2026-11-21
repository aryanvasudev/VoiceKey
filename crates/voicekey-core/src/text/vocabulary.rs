//! Custom vocabulary biasing and initial prompt generator for Whisper decoding.

use std::collections::BTreeSet;

/// Manages vocabulary biasing terms and formats them into an `initial_prompt` string
/// for Whisper inference conditioning.
#[derive(Debug, Clone, Default)]
pub struct VocabularyPrompt {
    base_prompt: Option<String>,
    hotwords: BTreeSet<String>,
}

impl VocabularyPrompt {
    /// Create a new empty vocabulary prompt.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an optional base prompt sentence (e.g., "The following is a technical programming dictation.").
    pub fn with_base_prompt(mut self, base: impl Into<String>) -> Self {
        self.base_prompt = Some(base.into());
        self
    }

    /// Add a domain keyword or acronym (e.g., "Kubernetes", "gRPC", "VoiceKey").
    pub fn add_hotword(&mut self, word: impl Into<String>) -> &mut Self {
        let w = word.into().trim().to_string();
        if !w.is_empty() {
            self.hotwords.insert(w);
        }
        self
    }

    /// Add multiple domain keywords.
    pub fn add_hotwords<I, S>(&mut self, words: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for w in words {
            self.add_hotword(w);
        }
        self
    }

    /// Remove a keyword.
    pub fn remove_hotword(&mut self, word: &str) -> bool {
        self.hotwords.remove(word.trim())
    }

    /// Return slice/iterator of active hotwords.
    pub fn hotwords(&self) -> &BTreeSet<String> {
        &self.hotwords
    }

    /// Construct the full `initial_prompt` passed to Whisper decoder.
    ///
    /// Combines the base prompt with comma-separated hotword biasing.
    pub fn build_prompt(&self) -> String {
        let hotwords_str = self
            .hotwords
            .iter()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ");

        match (&self.base_prompt, hotwords_str.is_empty()) {
            (Some(base), false) => format!("{}. {}", base.trim_end_matches('.'), hotwords_str),
            (Some(base), true) => base.clone(),
            (None, false) => hotwords_str,
            (None, true) => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_prompt() {
        let vocab = VocabularyPrompt::new();
        assert_eq!(vocab.build_prompt(), "");
    }

    #[test]
    fn test_hotwords_prompt() {
        let mut vocab = VocabularyPrompt::new();
        vocab.add_hotword("Kubernetes");
        vocab.add_hotword("Rust");
        vocab.add_hotword("VoiceKey");

        let prompt = vocab.build_prompt();
        // BTreeSet ensures deterministic alphabetical order:
        assert_eq!(prompt, "Kubernetes, Rust, VoiceKey");
    }

    #[test]
    fn test_base_prompt_with_hotwords() {
        let mut vocab = VocabularyPrompt::new().with_base_prompt("Programming dictation");
        vocab.add_hotwords(["Tauri", "Svelte"]);

        let prompt = vocab.build_prompt();
        assert_eq!(prompt, "Programming dictation. Svelte, Tauri");
    }
}
