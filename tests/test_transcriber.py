"""Tests for transcription backends, prompt building, and text post-processing."""

from pathlib import Path
from unittest.mock import MagicMock, patch
import numpy as np
import pytest

from voicekey.config import TranscriptionConfig
from voicekey.postprocess import apply_spoken_punctuation, normalize_text, process_transcription
from voicekey.transcriber import (
    FasterWhisperTranscriber,
    WhisperCppTranscriber,
    create_transcriber,
    list_available_backends,
)
from voicekey.transcriber.whisper_cpp import get_ggml_model_url


def test_process_transcription_empty():
    """Empty string returns empty string."""
    assert process_transcription("") == ""


def test_normalize_text():
    """Text normalization trims, capitalizes first letter, and collapses extra spaces."""
    raw = "   hello   world   from   voicekey   "
    assert normalize_text(raw) == "Hello world from voicekey"

    multiline = "line one\n\n\n\nline two"
    assert normalize_text(multiline) == "Line one\n\nline two"


def test_spoken_punctuation_enabled():
    """Spoken punctuation replaces words with punctuation and adjusts spacing."""
    spoken = "hello comma this is a test period new line how are you question mark"
    result = process_transcription(spoken, enable_normalization=True, enable_spoken_punctuation=True)
    assert "Hello, this is a test." in result
    assert "\nHow are you?" in result


def test_spoken_punctuation_toggle():
    """When spoken_punctuation is False, literal punctuation words are preserved."""
    spoken = "hello comma this is a test period"
    result = process_transcription(spoken, enable_normalization=False, enable_spoken_punctuation=False)
    assert result == "hello comma this is a test period"

    result_normalized = process_transcription(spoken, enable_normalization=True, enable_spoken_punctuation=False)
    assert result_normalized == "Hello comma this is a test period"


def test_build_prompt():
    """Prompt builder correctly merges initial prompt and hotword vocabulary."""
    # Empty
    cfg = TranscriptionConfig()
    transcriber = FasterWhisperTranscriber(cfg)
    assert transcriber.build_prompt() is None

    # Only initial prompt
    cfg_prompt = TranscriptionConfig(initial_prompt="Programming dictation")
    t_prompt = FasterWhisperTranscriber(cfg_prompt)
    assert t_prompt.build_prompt() == "Programming dictation"

    # Only hotwords
    cfg_hotwords = TranscriptionConfig(hotwords=["Kubernetes", "gRPC"])
    t_hotwords = FasterWhisperTranscriber(cfg_hotwords)
    assert t_hotwords.build_prompt() == "Vocabulary: Kubernetes, gRPC"

    # Both
    cfg_both = TranscriptionConfig(
        initial_prompt="Developer note",
        hotwords=["Linux", "Wayland"],
    )
    t_both = FasterWhisperTranscriber(cfg_both)
    assert t_both.build_prompt() == "Developer note. Vocabulary: Linux, Wayland"


def test_create_transcriber_factory():
    """create_transcriber returns appropriate backend instance or raises helpful error."""
    # faster-whisper is installed
    cfg = TranscriptionConfig(backend="faster-whisper")
    t = create_transcriber(cfg)
    assert isinstance(t, FasterWhisperTranscriber)
    assert t.name == "faster-whisper"

    # Unknown backend
    bad_cfg = TranscriptionConfig(backend="unknown-backend")
    with pytest.raises(ValueError, match="Unknown transcription backend"):
        create_transcriber(bad_cfg)

    # Unavailable backend raises RuntimeError with instructions
    with patch("voicekey.transcriber.openai_whisper.OpenAIWhisperTranscriber.is_available", return_value=False):
        openai_cfg = TranscriptionConfig(backend="openai-whisper")
        with pytest.raises(RuntimeError, match="Install with: uv pip install openai-whisper"):
            create_transcriber(openai_cfg)


def test_faster_whisper_transcribe_mocked():
    """FasterWhisperTranscriber runs inference and returns processed output."""
    cfg = TranscriptionConfig(backend="faster-whisper", spoken_punctuation=True, normalize_text=True)
    t = FasterWhisperTranscriber(cfg)

    mock_segment = MagicMock()
    mock_segment.text = "hello comma world"

    mock_model = MagicMock()
    mock_model.transcribe.return_value = ([mock_segment], None)
    t._model = mock_model
    t._is_loaded = True

    audio = np.ones(16000, dtype=np.float32)
    output = t.transcribe(audio)
    assert output == "Hello, world"
    mock_model.transcribe.assert_called_once()

    t.unload()
    assert not t.is_loaded
    assert t._model is None


def test_whisper_cpp_model_url():
    """whisper.cpp GGML model URLs format correctly."""
    assert "ggml-base.en.bin" in get_ggml_model_url("base.en")
    assert "ggml-tiny.bin" in get_ggml_model_url("ggml-tiny")


def test_whisper_cpp_transcribe_mocked(tmp_path: Path):
    """WhisperCppTranscriber writes audio wav, runs binary, and cleans up."""
    cfg = TranscriptionConfig(backend="whisper.cpp", normalize_text=True)
    t = WhisperCppTranscriber(cfg)

    fake_bin = tmp_path / "whisper-cli"
    fake_bin.touch()
    fake_model = tmp_path / "ggml-base.en.bin"
    fake_model.touch()

    t.bin_path = fake_bin
    t.model_path = fake_model
    t._is_loaded = True

    mock_res = MagicMock()
    mock_res.returncode = 0
    mock_res.stdout = "hello from whisper cpp"

    with patch("subprocess.run", return_value=mock_res) as mock_run:
        audio = np.zeros(16000, dtype=np.float32)
        text = t.transcribe(audio)
        assert text == "Hello from whisper cpp"
        mock_run.assert_called_once()
        cmd = mock_run.call_args[0][0]
        assert str(fake_bin) in cmd
        assert str(fake_model) in cmd
