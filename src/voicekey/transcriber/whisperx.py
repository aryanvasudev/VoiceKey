"""WhisperX transcription backend with forced alignment and VAD."""

from __future__ import annotations

import gc
import importlib.util
import logging
import numpy as np

from voicekey.config import TranscriptionConfig
from voicekey.transcriber.base import BaseTranscriber

logger = logging.getLogger("voicekey.transcriber.whisperx")


class WhisperXTranscriber(BaseTranscriber):
    """Transcription backend powered by WhisperX."""

    name = "whisperx"

    def __init__(self, config: TranscriptionConfig) -> None:
        super().__init__(config)
        self._model = None

    @classmethod
    def is_available(cls) -> bool:
        """Check if whisperx package is installed."""
        return importlib.util.find_spec("whisperx") is not None

    def initialize(self) -> None:
        """Load WhisperX model into memory."""
        if self._is_loaded and self._model is not None:
            return

        if not self.is_available():
            raise RuntimeError(
                "whisperx backend is selected but the package is not installed.\n"
                "Install it using: uv pip install whisperx"
            )

        import whisperx

        logger.info(
            "Loading whisperx model '%s' (device=%s, compute_type=%s)...",
            self.config.model_size,
            self.config.device,
            self.config.compute_type,
        )

        self._model = whisperx.load_model(
            self.config.model_size,
            device=self.config.device if self.config.device != "auto" else "cpu",
            compute_type=self.config.compute_type,
            threads=self.config.cpu_threads,
        )
        self._is_loaded = True
        logger.info("whisperx model '%s' loaded.", self.config.model_size)

    def transcribe_raw(self, audio_data: np.ndarray) -> str:
        """Transcribe audio using WhisperX."""
        if self._model is None or not self._is_loaded:
            self.initialize()

        result = self._model.transcribe(
            audio_data,
            batch_size=4,
        )

        segments = result.get("segments", [])
        texts = [seg.get("text", "") for seg in segments]
        return " ".join(texts).strip()

    def unload(self) -> None:
        """Unload WhisperX model and reclaim memory."""
        self._model = None
        self._is_loaded = False
        gc.collect()
        logger.debug("whisperx model unloaded.")
