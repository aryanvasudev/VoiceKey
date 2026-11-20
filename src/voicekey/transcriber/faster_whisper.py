"""faster-whisper CTranslate2 transcription backend."""

from __future__ import annotations

import gc
import importlib.util
import logging
import numpy as np

from voicekey.config import TranscriptionConfig
from voicekey.transcriber.base import BaseTranscriber

logger = logging.getLogger("voicekey.transcriber.faster_whisper")


class FasterWhisperTranscriber(BaseTranscriber):
    """Transcription backend powered by faster-whisper (CTranslate2)."""

    name = "faster-whisper"

    def __init__(self, config: TranscriptionConfig) -> None:
        super().__init__(config)
        self._model = None

    @classmethod
    def is_available(cls) -> bool:
        """Check if faster-whisper package is installed."""
        return importlib.util.find_spec("faster_whisper") is not None

    def initialize(self) -> None:
        """Load and warm faster-whisper model into RAM."""
        if self._is_loaded and self._model is not None:
            return

        if not self.is_available():
            raise RuntimeError(
                "faster-whisper backend is selected but the package is not installed.\n"
                "Install it using: uv pip install faster-whisper"
            )

        from faster_whisper import WhisperModel

        logger.info(
            "Loading faster-whisper model '%s' (device=%s, compute_type=%s, cpu_threads=%d)...",
            self.config.model_size,
            self.config.device,
            self.config.compute_type,
            self.config.cpu_threads,
        )

        self._model = WhisperModel(
            model_size_or_path=self.config.model_size,
            device=self.config.device,
            compute_type=self.config.compute_type,
            cpu_threads=self.config.cpu_threads,
        )
        self._is_loaded = True
        logger.info("faster-whisper model '%s' successfully loaded into memory.", self.config.model_size)

    def transcribe_raw(self, audio_data: np.ndarray) -> str:
        """Transcribe audio using preloaded CTranslate2 model."""
        if self._model is None or not self._is_loaded:
            self.initialize()

        prompt = self.build_prompt()
        segments, _info = self._model.transcribe(
            audio_data,
            beam_size=self.config.beam_size,
            vad_filter=self.config.vad_filter,
            initial_prompt=prompt,
        )

        texts = [segment.text for segment in segments]
        return " ".join(texts).strip()

    def unload(self) -> None:
        """Unload faster-whisper model and reclaim memory."""
        self._model = None
        self._is_loaded = False
        gc.collect()
        logger.debug("faster-whisper model unloaded.")
