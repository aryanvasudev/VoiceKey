"""Pluggable transcription engine registry and factory."""

from __future__ import annotations

import logging
from typing import Type

from voicekey.config import TranscriptionConfig
from voicekey.transcriber.base import BaseTranscriber
from voicekey.transcriber.faster_whisper import FasterWhisperTranscriber
from voicekey.transcriber.openai_whisper import OpenAIWhisperTranscriber
from voicekey.transcriber.whisper_cpp import WhisperCppTranscriber
from voicekey.transcriber.whisperx import WhisperXTranscriber

logger = logging.getLogger("voicekey.transcriber")

BACKEND_REGISTRY: dict[str, Type[BaseTranscriber]] = {
    "faster-whisper": FasterWhisperTranscriber,
    "whisper.cpp": WhisperCppTranscriber,
    "openai-whisper": OpenAIWhisperTranscriber,
    "whisperx": WhisperXTranscriber,
}


def list_available_backends() -> dict[str, bool]:
    """Query availability status for each supported transcription engine."""
    return {backend: cls.is_available() for backend, cls in BACKEND_REGISTRY.items()}


def create_transcriber(config: TranscriptionConfig) -> BaseTranscriber:
    """Factory function to instantiate the configured transcription engine."""
    backend_name = config.backend.lower().strip()
    cls = BACKEND_REGISTRY.get(backend_name)

    if cls is None:
        supported = ", ".join(sorted(BACKEND_REGISTRY.keys()))
        raise ValueError(f"Unknown transcription backend '{backend_name}'. Supported backends: {supported}")

    if not cls.is_available():
        if backend_name == "whisper.cpp":
            hint = "whisper.cpp requires git and cmake/make to compile automatically, or specify 'whisper_cpp_path' in config.toml."
        elif backend_name == "openai-whisper":
            hint = "Install with: uv pip install openai-whisper"
        elif backend_name == "whisperx":
            hint = "Install with: uv pip install whisperx"
        else:
            hint = "Install with: uv pip install faster-whisper"

        raise RuntimeError(
            f"Transcription backend '{backend_name}' is not currently available.\n{hint}\n"
            f"Run 'voicekey doctor' to inspect your hardware and available engines."
        )

    return cls(config)


__all__ = [
    "BACKEND_REGISTRY",
    "BaseTranscriber",
    "FasterWhisperTranscriber",
    "OpenAIWhisperTranscriber",
    "WhisperCppTranscriber",
    "WhisperXTranscriber",
    "create_transcriber",
    "list_available_backends",
]
