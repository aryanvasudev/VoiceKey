//! HTTP client and fallback rule-based engine for Micro-LLM transformations.

use crate::error::CoreError;
use crate::llm::templates::LlmTransformMode;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Supported API protocol formats for LLM inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmApiType {
    /// OpenAI-compatible chat completions API (`/v1/chat/completions`).
    /// Supported by Ollama, llama.cpp server, LocalAI, vLLM, and LiteLLM.
    OpenAiCompatible,
    /// Ollama native generate API (`/api/generate`).
    OllamaNative,
}

/// Configuration settings for the Micro-LLM post-processing engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Whether LLM transformation is enabled. Defaults to false (0ms fast path).
    pub enabled: bool,
    /// Target API endpoint base URL or full path.
    /// Example: `"http://127.0.0.1:11434"` (Ollama) or `"http://127.0.0.1:8080"` (llama.cpp)
    pub endpoint: String,
    /// Model name identifier (e.g. `"qwen2.5:0.5b"`, `"llama3.2:1b"`, `"qwen2.5:1.5b"`).
    pub model: String,
    /// API protocol type.
    pub api_type: LlmApiType,
    /// Request timeout in milliseconds (default: 2000ms).
    pub timeout_ms: u64,
    /// Generation temperature (default: 0.1 for high determinism).
    pub temperature: f32,
    /// Maximum tokens to generate (default: 256 for fast micro-turnaround).
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://127.0.0.1:11434".to_string(),
            model: "qwen2.5:0.5b".to_string(),
            api_type: LlmApiType::OpenAiCompatible,
            timeout_ms: 2000,
            temperature: 0.1,
            max_tokens: 256,
        }
    }
}

/// Trait defining LLM transformation capabilities.
pub trait LlmClient: Send + Sync {
    /// Transform `prompt` text according to `system_prompt`.
    fn transform(&self, prompt: &str, system_prompt: &str) -> Result<String>;
    /// Check whether the LLM server is reachable.
    fn is_available(&self) -> bool;
}

/// HTTP-based LLM client for local micro-LLMs (Ollama, llama.cpp, etc.).
pub struct HttpLlmClient {
    config: LlmConfig,
    agent: ureq::Agent,
}

impl HttpLlmClient {
    /// Create a new HTTP LLM client from configuration.
    pub fn new(config: LlmConfig) -> Self {
        let timeout = Duration::from_millis(config.timeout_ms);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_millis(500))
            .timeout_read(timeout)
            .timeout_write(timeout)
            .build();
        Self { config, agent }
    }

    /// Access the underlying config.
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    fn resolve_endpoint(&self) -> String {
        let base = self.config.endpoint.trim_end_matches('/');
        match self.config.api_type {
            LlmApiType::OpenAiCompatible => {
                if base.ends_with("/chat/completions") {
                    base.to_string()
                } else if base.ends_with("/v1") {
                    format!("{}/chat/completions", base)
                } else {
                    format!("{}/v1/chat/completions", base)
                }
            }
            LlmApiType::OllamaNative => {
                if base.ends_with("/api/generate") {
                    base.to_string()
                } else {
                    format!("{}/api/generate", base)
                }
            }
        }
    }

    fn clean_llm_output(raw: &str) -> String {
        let trimmed = raw.trim();
        // Remove markdown code fence wrapping if the model returned ```markdown ... ```
        if (trimmed.starts_with("```") && trimmed.ends_with("```"))
            || (trimmed.starts_with("```markdown") && trimmed.ends_with("```"))
        {
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() >= 2 {
                let inner = &lines[1..lines.len() - 1];
                return inner.join("\n").trim().to_string();
            }
        }
        trimmed.to_string()
    }
}

impl LlmClient for HttpLlmClient {
    fn is_available(&self) -> bool {
        let base = self.config.endpoint.trim_end_matches('/');
        let test_url = match self.config.api_type {
            LlmApiType::OpenAiCompatible => format!("{}/v1/models", base),
            LlmApiType::OllamaNative => format!("{}/api/tags", base),
        };

        self.agent
            .get(&test_url)
            .call()
            .map(|resp| resp.status() == 200)
            .unwrap_or(false)
    }

    fn transform(&self, prompt: &str, system_prompt: &str) -> Result<String> {
        let url = self.resolve_endpoint();

        match self.config.api_type {
            LlmApiType::OpenAiCompatible => {
                let body = serde_json::json!({
                    "model": self.config.model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": prompt}
                    ],
                    "temperature": self.config.temperature,
                    "max_tokens": self.config.max_tokens,
                    "stream": false
                });

                let response = self
                    .agent
                    .post(&url)
                    .set("Content-Type", "application/json")
                    .send_json(body)
                    .map_err(|e| CoreError::Llm(format!("HTTP request failed: {}", e)))?;

                let res_json: serde_json::Value = response
                    .into_json()
                    .map_err(|e| CoreError::Llm(format!("Failed to parse JSON response: {}", e)))?;

                let content = res_json["choices"][0]["message"]["content"]
                    .as_str()
                    .ok_or_else(|| {
                        CoreError::Llm(
                            "Missing 'choices[0].message.content' in response".to_string(),
                        )
                    })?;

                Ok(Self::clean_llm_output(content))
            }
            LlmApiType::OllamaNative => {
                let body = serde_json::json!({
                    "model": self.config.model,
                    "system": system_prompt,
                    "prompt": prompt,
                    "stream": false,
                    "options": {
                        "temperature": self.config.temperature,
                        "num_predict": self.config.max_tokens
                    }
                });

                let response = self
                    .agent
                    .post(&url)
                    .set("Content-Type", "application/json")
                    .send_json(body)
                    .map_err(|e| CoreError::Llm(format!("Ollama HTTP request failed: {}", e)))?;

                let res_json: serde_json::Value = response
                    .into_json()
                    .map_err(|e| CoreError::Llm(format!("Failed to parse JSON response: {}", e)))?;

                let content = res_json["response"].as_str().ok_or_else(|| {
                    CoreError::Llm("Missing 'response' in Ollama output".to_string())
                })?;

                Ok(Self::clean_llm_output(content))
            }
        }
    }
}

/// Deterministic, zero-dependency fallback transformer.
///
/// Used when the LLM server is offline, disabled, or unreachable,
/// ensuring spoken formatting commands always succeed with 0ms latency.
pub struct FallbackTransformer;

impl FallbackTransformer {
    /// Apply fallback transformations to `raw_text` for the specified mode.
    pub fn transform_fallback(mode: LlmTransformMode, raw_text: &str) -> String {
        match mode {
            LlmTransformMode::CleanTranscript => Self::clean_transcript(raw_text),
            LlmTransformMode::BulletPoints => Self::to_bullet_points(raw_text),
            LlmTransformMode::ProfessionalTone => Self::to_professional(raw_text),
            LlmTransformMode::CasualTone => Self::to_casual(raw_text),
            LlmTransformMode::VoiceToCode => Self::to_code(raw_text),
            LlmTransformMode::Custom => raw_text.trim().to_string(),
        }
    }

    /// Strips common filler words and repeated stutter tokens.
    pub fn clean_transcript(raw_text: &str) -> String {
        let filler_re =
            regex::Regex::new(r"(?i)\b(uh|um|er|ah|like|you know|sort of|kind of|i mean)\b")
                .expect("valid regex");
        let cleaned = filler_re.replace_all(raw_text, "");

        // Collapse multi-spaces and fix spacing before punctuation
        let multi_space = regex::Regex::new(r"\s+").expect("valid regex");
        let spaced = multi_space.replace_all(&cleaned, " ");

        let punct_space = regex::Regex::new(r"\s+([,.\?!;:])").expect("valid regex");
        let fixed = punct_space.replace_all(&spaced, "$1");

        // Remove duplicate adjacent words (e.g. "the the" -> "the")
        let mut words: Vec<&str> = Vec::new();
        for word in fixed.split_whitespace() {
            if let Some(&last) = words.last() {
                if word.eq_ignore_ascii_case(last) {
                    continue;
                }
            }
            words.push(word);
        }

        words.join(" ")
    }

    /// Converts comma-separated or sentence-separated speech into markdown bullet items.
    pub fn to_bullet_points(raw_text: &str) -> String {
        let cleaned = Self::clean_transcript(raw_text);
        if cleaned.is_empty() {
            return String::new();
        }

        // Split on sentences, commas, or "and"
        let parts: Vec<&str> = cleaned
            .split(['\n', '.', ';'])
            .flat_map(|s| s.split(" and then "))
            .flat_map(|s| s.split(" also "))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return format!("- {}", cleaned);
        }

        parts
            .into_iter()
            .map(|p| {
                let trimmed = p.trim_start_matches("- ").trim_start_matches("* ").trim();
                format!("- {}", capitalize_first(trimmed))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Rewrites spoken text to formal business tone using rule-based substitution.
    pub fn to_professional(raw_text: &str) -> String {
        let mut text = Self::clean_transcript(raw_text);

        let replacements = [
            (r"(?i)\bgonna\b", "going to"),
            (r"(?i)\bwanna\b", "want to"),
            (r"(?i)\bkinda\b", "rather"),
            (r"(?i)\by'all\b", "you all"),
            (r"(?i)\byeah\b", "yes"),
            (r"(?i)\byep\b", "yes"),
            (r"(?i)\bnope\b", "no"),
            (r"(?i)\bcuz\b", "because"),
            (r"(?i)\bcos\b", "because"),
            (r"(?i)\basap\b", "as soon as possible"),
        ];

        for (from, to) in replacements {
            if let Ok(re) = regex::Regex::new(from) {
                text = re.replace_all(&text, to).to_string();
            }
        }

        let capitalized = capitalize_first(&text);
        if !capitalized.ends_with('.')
            && !capitalized.ends_with('?')
            && !capitalized.ends_with('!')
            && !capitalized.is_empty()
        {
            format!("{}.", capitalized)
        } else {
            capitalized
        }
    }

    /// Rewrites text into casual chat tone.
    pub fn to_casual(raw_text: &str) -> String {
        let text = Self::clean_transcript(raw_text);
        let casual_replacements = [
            (
                r"(?i)\bI would like to inform you that\b",
                "Just a heads up,",
            ),
            (r"(?i)\bPlease be advised that\b", "FYI:"),
            (r"(?i)\bI would be grateful if\b", "Could you please"),
            (r"(?i)\bDo not hesitate to contact me\b", "Let me know"),
        ];

        let mut res = text;
        for (from, to) in casual_replacements {
            if let Ok(re) = regex::Regex::new(from) {
                res = re.replace_all(&res, to).to_string();
            }
        }
        res
    }

    /// Voice to code syntax replacements.
    pub fn to_code(raw_text: &str) -> String {
        let text = Self::clean_transcript(raw_text);
        let replacements = [
            (r"(?i)\btriple equals\b", "==="),
            (r"(?i)\bdouble equals\b", "=="),
            (r"(?i)\bnot equals\b", "!="),
            (r"(?i)\bis equal to\b", "="),
            (r"(?i)\bequals\b", "="),
            (r"(?i)\barrow function\b", "=>"),
            (r"(?i)\barrow\b", "=>"),
            (r"(?i)\bopen curly brace\b", "{"),
            (r"(?i)\bclose curly brace\b", "}"),
            (r"(?i)\bopen brace\b", "{"),
            (r"(?i)\bclose brace\b", "}"),
            (r"(?i)\bopen square bracket\b", "["),
            (r"(?i)\bclose square bracket\b", "]"),
            (r"(?i)\bopen bracket\b", "["),
            (r"(?i)\bclose bracket\b", "]"),
            (r"(?i)\bopen paren\b", "("),
            (r"(?i)\bclose paren\b", ")"),
            (r"(?i)\bsemicolon\b", ";"),
            (r"(?i)\bconsole dot log\b", "console.log"),
            (r"(?i)\bconsole log\b", "console.log"),
            (r"(?i)\bdot\b", "."),
        ];

        let mut res = text;
        for (from, to) in replacements {
            if let Ok(re) = regex::Regex::new(from) {
                res = re.replace_all(&res, to).to_string();
            }
        }
        res
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_clean_transcript() {
        let input = "I was um thinking that uh maybe we should go";
        let cleaned = FallbackTransformer::clean_transcript(input);
        assert_eq!(cleaned, "I was thinking that maybe we should go");
    }

    #[test]
    fn test_fallback_dedup() {
        let input = "we need the the report today";
        let cleaned = FallbackTransformer::clean_transcript(input);
        assert_eq!(cleaned, "we need the report today");
    }

    #[test]
    fn test_fallback_bullet_points() {
        let input = "first item and then second item and then third item";
        let bullets = FallbackTransformer::to_bullet_points(input);
        assert!(bullets.contains("- First item"));
        assert!(bullets.contains("- Second item"));
        assert!(bullets.contains("- Third item"));
    }

    #[test]
    fn test_fallback_professional() {
        let input = "we gonna ship this asap yeah";
        let prof = FallbackTransformer::to_professional(input);
        assert_eq!(prof, "We going to ship this as soon as possible yes.");
    }

    #[test]
    fn test_fallback_code() {
        let input = "const x equals five semicolon";
        let code = FallbackTransformer::to_code(input);
        assert_eq!(code, "const x = five ;");
    }
}
