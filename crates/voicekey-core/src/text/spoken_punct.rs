//! Spoken punctuation transformer mapping verbal commands to punctuation symbols.

use regex::Regex;
use std::sync::LazyLock;

struct PunctRule {
    pattern: Regex,
    replacement: &'static str,
}

static PUNCT_RULES: LazyLock<Vec<PunctRule>> = LazyLock::new(|| {
    vec![
        PunctRule {
            pattern: Regex::new(r"(?i)\bnew paragraph\b").unwrap(),
            replacement: "\n\n",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\b(new line|newline)\b").unwrap(),
            replacement: "\n",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\bcomma\b").unwrap(),
            replacement: ",",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\bcomma\b").unwrap(),
            replacement: ",",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\b(period|full stop)\b").unwrap(),
            replacement: ".",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\b(period|full stop)\b").unwrap(),
            replacement: ".",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\bquestion mark\b").unwrap(),
            replacement: "?",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\bquestion mark\b").unwrap(),
            replacement: "?",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\b(exclamation mark|exclamation point)\b").unwrap(),
            replacement: "!",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\b(exclamation mark|exclamation point)\b").unwrap(),
            replacement: "!",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\bcolon\b").unwrap(),
            replacement: ":",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\bsemicolon\b").unwrap(),
            replacement: ";",
        },
        PunctRule {
            pattern: Regex::new(r"(?i)\s+\b(hyphen|dash)\b").unwrap(),
            replacement: "-",
        },
    ]
});

static SPACES_BEFORE_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+([,.:;?!])").unwrap());
static SPACE_AFTER_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([,.:;?!])([A-Za-z0-9])").unwrap());
static WS_AROUND_NEWLINES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]*\n[ \t]*").unwrap());
static CAPITALIZE_AFTER_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([.!?\n]\s*)([a-z])").unwrap());

/// Transform explicitly spoken punctuation commands into punctuation symbols.
pub fn apply_spoken_punctuation(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();
    for rule in PUNCT_RULES.iter() {
        result = rule
            .pattern
            .replace_all(&result, rule.replacement)
            .into_owned();
    }

    // Clean up spaces before punctuation: "word ," -> "word,"
    result = SPACES_BEFORE_PUNCT.replace_all(&result, "$1").into_owned();

    // Ensure space after punctuation if followed by alphanumeric: "word,next" -> "word, next"
    result = SPACE_AFTER_PUNCT.replace_all(&result, "$1 $2").into_owned();

    // Clean up whitespace around newlines
    result = WS_AROUND_NEWLINES.replace_all(&result, "\n").into_owned();

    // Capitalize first letter after period, exclamation mark, question mark, or newline
    result = CAPITALIZE_AFTER_PUNCT
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let ch = &caps[2];
            format!("{}{}", prefix, ch.to_uppercase())
        })
        .into_owned();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spoken_comma_and_period() {
        let input = "Hello comma world period this is great";
        let out = apply_spoken_punctuation(input);
        assert_eq!(out, "Hello, world. This is great");
    }

    #[test]
    fn test_newlines_and_paragraphs() {
        let input = "First line newline second line new paragraph third paragraph";
        let out = apply_spoken_punctuation(input);
        assert_eq!(out, "First line\nSecond line\n\nThird paragraph");
    }

    #[test]
    fn test_question_and_exclamation() {
        let input = "How are you question mark I am great exclamation mark";
        let out = apply_spoken_punctuation(input);
        assert_eq!(out, "How are you? I am great!");
    }

    #[test]
    fn test_spacing_cleanup() {
        let input = "one , two .three :four ;five -six";
        let out = apply_spoken_punctuation(input);
        assert_eq!(out, "one, two. Three: four; five -six");
    }
}
