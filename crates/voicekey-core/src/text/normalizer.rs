//! Text normalization: whitespace cleanup, blank line collapsing, and sentence capitalization.

use regex::Regex;
use std::sync::LazyLock;

static WS_AROUND_NEWLINES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]*\n[ \t]*").unwrap());
static INLINE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
static EXCESS_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

/// Clean whitespace, strip hallucinated blank characters, and normalize capitalization.
pub fn normalize_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Clean whitespace around newlines
    let mut cleaned = WS_AROUND_NEWLINES.replace_all(trimmed, "\n").into_owned();

    // Collapse multiple inline spaces (preserve newlines)
    cleaned = INLINE_SPACES.replace_all(&cleaned, " ").into_owned();

    // Clean blank lines beyond double newlines
    cleaned = EXCESS_NEWLINES.replace_all(&cleaned, "\n\n").into_owned();

    // Capitalize very first letter if lowercase
    if let Some(first_char) = cleaned.chars().next() {
        if first_char.is_lowercase() {
            let mut chars = cleaned.chars();
            let upper = chars.next().unwrap().to_uppercase().to_string();
            cleaned = format!("{}{}", upper, chars.as_str());
        }
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_normalize() {
        assert_eq!(normalize_text(""), "");
        assert_eq!(normalize_text("   \n\t  "), "");
    }

    #[test]
    fn test_collapse_spaces() {
        assert_eq!(
            normalize_text("hello     world   from   rust"),
            "Hello world from rust"
        );
    }

    #[test]
    fn test_excess_newlines() {
        assert_eq!(
            normalize_text("Paragraph one\n\n\n\n\nParagraph two"),
            "Paragraph one\n\nParagraph two"
        );
    }

    #[test]
    fn test_capitalize_first_character() {
        assert_eq!(normalize_text("lower case start"), "Lower case start");
        assert_eq!(normalize_text("Already Capital"), "Already Capital");
        assert_eq!(normalize_text("123 numeric start"), "123 numeric start");
    }
}
