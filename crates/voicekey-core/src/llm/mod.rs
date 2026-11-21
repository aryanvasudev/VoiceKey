//! Micro-LLM Post-Processing & Smart Dictation engine for VoiceKey.
//!
//! Provides deterministic prompt templates, spoken directive parsing (e.g. "VoiceKey bullets: ..."),
//! an HTTP client for local/embedded LLM engines (Ollama, llama.cpp, vLLM),
//! and zero-latency fallback transformation.

pub mod client;
pub mod templates;

pub use client::{FallbackTransformer, HttpLlmClient, LlmApiType, LlmClient, LlmConfig};
pub use templates::{parse_spoken_command, LlmTransformMode, ParsedVoiceCommand};

use crate::Result;
use tracing::warn;

/// Main high-level orchestrator for smart dictation transformation.
///
/// **Fast Path Guarantee**: If `forced_mode` is `None` and `raw_text` does not start with
/// a recognized spoken voice command (e.g. "VoiceKey bullets: ..."), this function returns
/// `raw_text` immediately with zero HTTP requests, zero allocations, and 0ms latency.
///
/// If an AI mode is active or spoken command prefix is parsed:
/// 1. The payload and target mode are extracted.
/// 2. If a connected `LlmClient` is provided, it performs inference.
/// 3. If the LLM is unavailable or fails, it falls back gracefully to `FallbackTransformer`.
pub fn process_with_llm(
    raw_text: &str,
    forced_mode: Option<LlmTransformMode>,
    client: Option<&dyn LlmClient>,
) -> Result<String> {
    let (mode, payload, custom_instruction) = match forced_mode {
        Some(m) => (m, raw_text.to_string(), None),
        None => match parse_spoken_command(raw_text) {
            Some(cmd) => (cmd.mode, cmd.payload, cmd.custom_instruction),
            None => {
                // FAST PATH: standard voice dictation with zero overhead.
                return Ok(raw_text.to_string());
            }
        },
    };

    let system_prompt = if mode == LlmTransformMode::Custom {
        if let Some(ref custom) = custom_instruction {
            custom.as_str()
        } else {
            mode.system_prompt()
        }
    } else {
        mode.system_prompt()
    };

    // If a client is provided, attempt LLM inference
    if let Some(llm) = client {
        match llm.transform(&payload, system_prompt) {
            Ok(output) if !output.trim().is_empty() => return Ok(output),
            Err(e) => {
                warn!(
                    "LLM transformation failed ({}); using rule-based fallback",
                    e
                );
            }
            _ => {}
        }
    }

    // Graceful offline fallback
    Ok(FallbackTransformer::transform_fallback(mode, &payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_untouched() {
        let text = "Hello world, this is normal typing without any commands.";
        let res = process_with_llm(text, None, None).expect("processing succeeds");
        assert_eq!(res, text);
    }

    #[test]
    fn test_spoken_command_triggers_fallback() {
        let text = "VoiceKey bullets: item one and then item two and then item three";
        let res = process_with_llm(text, None, None).expect("processing succeeds");
        assert!(res.contains("- Item one"));
        assert!(res.contains("- Item two"));
        assert!(res.contains("- Item three"));
    }

    #[test]
    fn test_forced_mode_triggers_fallback() {
        let text = "I was um thinking that uh maybe we should go";
        let res = process_with_llm(text, Some(LlmTransformMode::CleanTranscript), None)
            .expect("processing succeeds");
        assert_eq!(res, "I was thinking that maybe we should go");
    }
}
