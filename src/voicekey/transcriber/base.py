"""Abstract Base Class for all VoiceKey transcription backends."""

from __future__ import annotations

from abc import ABC, abstractmethod
import logging
import numpy as np

from voicekey.config import TranscriptionConfig
from voicekey.postprocess import process_transcription

logger = logging.getLogger("voicekey.transcriber")


class BaseTranscriber(ABC):
    """Abstract interface for speech transcription engines."""

    name: str = "base"

    def __init__(self, config: TranscriptionConfig) -> None:
        self.config = config
        self._is_loaded = False

    @property
    def is_loaded(self) -> bool:
        """Return True if model weights are loaded in memory."""
        return self._is_loaded

    @classmethod
    @abstractmethod
    def is_available(cls) -> bool:
        """Check if backend dependencies or binaries are installed on the system."""
        pass

    @abstractmethod
    def initialize(self) -> None:
        """Load and warm model into memory."""
        pass

    @abstractmethod
    def transcribe_raw(self, audio_data: np.ndarray) -> str:
        """Run raw inference on 1D float32 audio array (16kHz)."""
        pass

    def unload(self) -> None:
        """Unload model from RAM/VRAM to free resources."""
        self._is_loaded = False

    def build_prompt(self) -> str | None:
        """Construct effective prompt combining initial_prompt and hotwords for vocabulary biasing."""
        parts: list[str] = []
        if self.config.initial_prompt.strip():
            parts.append(self.config.initial_prompt.strip())

        if self.config.hotwords:
            unique_hotwords = [w.strip() for w in self.config.hotwords if w.strip()]
            if unique_hotwords:
                parts.append(f"Vocabulary: {', '.join(unique_hotwords)}")

        if not parts:
            return None
        return ". ".join(parts)

    def transcribe(self, audio_data: np.ndarray) -> str:
        """Perform end-to-end transcription with optional text post-processing."""
        if not self._is_loaded:
            self.initialize()

        if len(audio_data) == 0:
            return ""

        raw_text = self.transcribe_raw(audio_data)
        processed = process_transcription(
            raw_text=raw_text,
            enable_normalization=self.config.normalize_text,
            enable_spoken_punctuation=self.config.spoken_punctuation,
        )
        return processed
