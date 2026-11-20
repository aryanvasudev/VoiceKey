"""Text post-processing and spoken punctuation transformer."""

from __future__ import annotations

import re


# Mappings for spoken punctuation commands (case-insensitive)
PUNCTUATION_RULES: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"\b(new paragraph)\b", re.IGNORECASE), "\n\n"),
    (re.compile(r"\b(new line|newline)\b", re.IGNORECASE), "\n"),
    (re.compile(r"\s+\b(comma)\b", re.IGNORECASE), ","),
    (re.compile(r"\b(comma)\b", re.IGNORECASE), ","),
    (re.compile(r"\s+\b(period|full stop)\b", re.IGNORECASE), "."),
    (re.compile(r"\b(period|full stop)\b", re.IGNORECASE), "."),
    (re.compile(r"\s+\b(question mark)\b", re.IGNORECASE), "?"),
    (re.compile(r"\b(question mark)\b", re.IGNORECASE), "?"),
    (re.compile(r"\s+\b(exclamation mark|exclamation point)\b", re.IGNORECASE), "!"),
    (re.compile(r"\b(exclamation mark|exclamation point)\b", re.IGNORECASE), "!"),
    (re.compile(r"\s+\b(colon)\b", re.IGNORECASE), ":"),
    (re.compile(r"\s+\b(semicolon)\b", re.IGNORECASE), ";"),
    (re.compile(r"\s+\b(hyphen|dash)\b", re.IGNORECASE), "-"),
]


def apply_spoken_punctuation(text: str) -> str:
    """Transform explicitly spoken punctuation commands into punctuation symbols."""
    result = text
    for pattern, replacement in PUNCTUATION_RULES:
        result = pattern.sub(replacement, result)

    # Clean up spaces before punctuation: e.g. "word ," -> "word,"
    result = re.sub(r"\s+([,.:;?!])", r"\1", result)

    # Ensure space after punctuation if followed by word character, e.g. "word,next" -> "word, next"
    result = re.sub(r"([,.:;?!])([A-Za-z0-9])", r"\1 \2", result)

    # Clean up whitespace around newlines
    result = re.sub(r"[ \t]*\n[ \t]*", "\n", result)

    # Capitalize first letter after period, exclamation mark, question mark, or newline
    result = re.sub(
        r"([.!?\n]\s*)([a-z])",
        lambda m: m.group(1) + m.group(2).upper(),
        result,
    )

    return result


def normalize_text(text: str) -> str:
    """Clean whitespace, strip hallucinated blank characters, and normalize capitalization."""
    if not text:
        return ""

    # Strip surrounding whitespace
    cleaned = text.strip()

    # Clean whitespace around newlines
    cleaned = re.sub(r"[ \t]*\n[ \t]*", "\n", cleaned)

    # Collapse multiple inline spaces (preserve newlines)
    cleaned = re.sub(r"[ \t]+", " ", cleaned)

    # Clean blank lines beyond double newlines
    cleaned = re.sub(r"\n{3,}", "\n\n", cleaned)

    # Capitalize very first letter if lower
    if cleaned and cleaned[0].islower():
        cleaned = cleaned[0].upper() + cleaned[1:]

    return cleaned


def process_transcription(
    raw_text: str,
    enable_normalization: bool = True,
    enable_spoken_punctuation: bool = False,
) -> str:
    """Process raw transcription through enabled pipelines according to user config toggles."""
    if not raw_text:
        return ""

    result = raw_text

    if enable_spoken_punctuation:
        result = apply_spoken_punctuation(result)

    if enable_normalization:
        result = normalize_text(result)

    return result
