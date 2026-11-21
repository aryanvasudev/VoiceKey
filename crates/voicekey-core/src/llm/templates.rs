//! Micro-LLM templates and command parser for VoiceKey.
//!
//! Provides deterministic prompt templates for bullet point formatting,
//! filler-word & stutter cleanup, professional/casual tone shifts, and code syntax translations.
//! Also provides spoken directive detection (e.g. "VoiceKey bullets: ...").

use serde::{Deserialize, Serialize};

/// Pre-defined transformation modes supported by the Micro-LLM engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmTransformMode {
    /// Natural clean-up: strips filler words ("uh", "um", "like"), fixes stutters and false starts.
    CleanTranscript,
    /// Converts freeform ideas into a crisp markdown bulleted list.
    BulletPoints,
    /// Rewrites spoken thoughts into a concise, professional business email/message.
    ProfessionalTone,
    /// Formats spoken thoughts into a friendly, casual chat message.
    CasualTone,
    /// Translates spoken programming instructions into valid code syntax.
    VoiceToCode,
    /// Custom instruction supplied by the user.
    Custom,
}

impl LlmTransformMode {
    /// Return the system prompt for the given transform mode.
    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::CleanTranscript => {
                "You are an ultra-fast text editor. Your task is to clean up a raw spoken transcript. \
                 Remove filler words (uh, um, like, you know), false starts, and repeated words. \
                 Fix punctuation and sentence flow, but do NOT add commentary, explanations, or quotes. \
                 Output ONLY the cleaned text."
            }
            Self::BulletPoints => {
                "You are an ultra-fast text formatter. Your task is to turn the spoken input into a clean, \
                 concise markdown bullet point list. Each item should start with '- '. \
                 Do NOT add commentary, explanations, intros, or markdown backticks. \
                 Output ONLY the bullet points."
            }
            Self::ProfessionalTone => {
                "You are an ultra-fast writing assistant. Rewrite the spoken input into clear, polite, \
                 and concise professional workplace communication suitable for an email or formal message. \
                 Do NOT add commentary, subject lines, or explanations. \
                 Output ONLY the rewritten text."
            }
            Self::CasualTone => {
                "You are an ultra-fast writing assistant. Rewrite the spoken input into a friendly, \
                 conversational message suitable for Slack or team chat. Keep it concise and natural. \
                 Do NOT add commentary or explanations. \
                 Output ONLY the rewritten text."
            }
            Self::VoiceToCode => {
                "You are an ultra-fast voice-to-code translator. Convert the spoken programming statements \
                 into clean, idiomatic code without markdown blocks unless necessary. \
                 Do NOT add explanations or prose. \
                 Output ONLY the code."
            }
            Self::Custom => {
                "You are an ultra-fast text assistant. Follow the user's specific instruction concisely. \
                 Output ONLY the final processed text without preamble."
            }
        }
    }
}

/// A parsed voice command from spoken dictation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVoiceCommand {
    /// Detected transformation mode.
    pub mode: LlmTransformMode,
    /// Custom prompt instruction if mode is `Custom`.
    pub custom_instruction: Option<String>,
    /// The actual content payload to transform.
    pub payload: String,
}

/// Spoken command prefix parser.
///
/// Detects phrases like:
/// - `"VoiceKey bullet points: meeting notes ..."`
/// - `"VoiceKey clean: I was thinking like maybe we should ..."`
/// - `"VoiceKey professional: tell him we are delayed ..."`
/// - `"VoiceKey code: const user id equals user dot id ..."`
/// - `"VoiceKey [instruction]: [content]"`
pub fn parse_spoken_command(raw_text: &str) -> Option<ParsedVoiceCommand> {
    let text = raw_text.trim();
    let lower = text.to_lowercase();

    // Look for prefixes like "voicekey", "voice key", or "vk"
    let prefix = if lower.starts_with("voicekey ")
        || lower.starts_with("voicekey:")
        || lower.starts_with("voicekey,")
    {
        "voicekey"
    } else if lower.starts_with("voice key ")
        || lower.starts_with("voice key:")
        || lower.starts_with("voice key,")
    {
        "voice key"
    } else if lower.starts_with("vk ") || lower.starts_with("vk:") {
        "vk"
    } else {
        return None;
    };

    // Strip prefix
    let remainder = text[prefix.len()..]
        .trim_start_matches(|c: char| c == ':' || c == ',' || c.is_whitespace())
        .trim();
    if remainder.is_empty() {
        return None;
    }

    let remainder_lower = remainder.to_lowercase();

    // Check specific known action keywords
    if let Some(rest) = strip_command_keyword(
        &remainder_lower,
        &["bullet points", "bullets", "bullet list", "list"],
    ) {
        let payload = extract_payload(remainder, rest);
        return Some(ParsedVoiceCommand {
            mode: LlmTransformMode::BulletPoints,
            custom_instruction: None,
            payload,
        });
    }

    if let Some(rest) = strip_command_keyword(
        &remainder_lower,
        &["clean", "cleanup", "fix grammar", "clean transcript"],
    ) {
        let payload = extract_payload(remainder, rest);
        return Some(ParsedVoiceCommand {
            mode: LlmTransformMode::CleanTranscript,
            custom_instruction: None,
            payload,
        });
    }

    if let Some(rest) = strip_command_keyword(
        &remainder_lower,
        &["professional", "formal", "email tone", "business"],
    ) {
        let payload = extract_payload(remainder, rest);
        return Some(ParsedVoiceCommand {
            mode: LlmTransformMode::ProfessionalTone,
            custom_instruction: None,
            payload,
        });
    }

    if let Some(rest) = strip_command_keyword(&remainder_lower, &["casual", "slack", "friendly"]) {
        let payload = extract_payload(remainder, rest);
        return Some(ParsedVoiceCommand {
            mode: LlmTransformMode::CasualTone,
            custom_instruction: None,
            payload,
        });
    }

    if let Some(rest) = strip_command_keyword(&remainder_lower, &["code", "syntax", "program"]) {
        let payload = extract_payload(remainder, rest);
        return Some(ParsedVoiceCommand {
            mode: LlmTransformMode::VoiceToCode,
            custom_instruction: None,
            payload,
        });
    }

    // If separated by a colon, treat first part as custom instruction
    if let Some(colon_idx) = remainder.find(':') {
        let instruction = remainder[..colon_idx].trim().to_string();
        let payload = remainder[colon_idx + 1..].trim().to_string();
        if !instruction.is_empty() && !payload.is_empty() {
            return Some(ParsedVoiceCommand {
                mode: LlmTransformMode::Custom,
                custom_instruction: Some(instruction),
                payload,
            });
        }
    }

    // Default fallback: general cleanup of the remaining phrase
    Some(ParsedVoiceCommand {
        mode: LlmTransformMode::CleanTranscript,
        custom_instruction: None,
        payload: remainder.to_string(),
    })
}

fn strip_command_keyword<'a>(lower_str: &'a str, keywords: &[&str]) -> Option<&'a str> {
    for kw in keywords {
        if let Some(next) = lower_str.strip_prefix(kw) {
            if next.starts_with(':')
                || next.starts_with(',')
                || next.starts_with(' ')
                || next.is_empty()
            {
                return Some(next);
            }
        }
    }
    None
}

fn extract_payload(original_str: &str, lower_remainder: &str) -> String {
    let start_offset = original_str.len() - lower_remainder.len();
    original_str[start_offset..]
        .trim_start_matches(|c: char| c == ':' || c == ',' || c.is_whitespace())
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bullet_points_command() {
        let cmd = parse_spoken_command("VoiceKey bullets: buy milk, call John, finish PR");
        assert!(cmd.is_some());
        let c = cmd.unwrap();
        assert_eq!(c.mode, LlmTransformMode::BulletPoints);
        assert_eq!(c.payload, "buy milk, call John, finish PR");
    }

    #[test]
    fn test_parse_clean_command() {
        let cmd = parse_spoken_command("VoiceKey clean: uh I was like thinking we should go");
        assert!(cmd.is_some());
        let c = cmd.unwrap();
        assert_eq!(c.mode, LlmTransformMode::CleanTranscript);
        assert_eq!(c.payload, "uh I was like thinking we should go");
    }

    #[test]
    fn test_parse_professional_command() {
        let cmd = parse_spoken_command(
            "voice key professional: I cannot make it today let us meet tomorrow",
        );
        assert!(cmd.is_some());
        let c = cmd.unwrap();
        assert_eq!(c.mode, LlmTransformMode::ProfessionalTone);
        assert_eq!(c.payload, "I cannot make it today let us meet tomorrow");
    }

    #[test]
    fn test_parse_code_command() {
        let cmd = parse_spoken_command("VK code: const x equals ten");
        assert!(cmd.is_some());
        let c = cmd.unwrap();
        assert_eq!(c.mode, LlmTransformMode::VoiceToCode);
        assert_eq!(c.payload, "const x equals ten");
    }

    #[test]
    fn test_parse_custom_instruction() {
        let cmd = parse_spoken_command(
            "VoiceKey summarize in three words: the quick brown fox jumps over the lazy dog",
        );
        assert!(cmd.is_some());
        let c = cmd.unwrap();
        assert_eq!(c.mode, LlmTransformMode::Custom);
        assert_eq!(
            c.custom_instruction,
            Some("summarize in three words".into())
        );
        assert_eq!(c.payload, "the quick brown fox jumps over the lazy dog");
    }

    #[test]
    fn test_non_command_returns_none() {
        let cmd = parse_spoken_command("Today was a great day working on VoiceKey features");
        assert!(cmd.is_none());
    }
}
