"""Official vanilla OpenAI Whisper (PyTorch) transcription backend."""

from __future__ import annotations

import gc
import importlib.util
import logging
import numpy as np

from voicekey.config import TranscriptionConfig
from voicekey.transcriber.base import BaseTranscriber

logger = logging.getLogger("voicekey.transcriber.openai_whisper")


class OpenAIWhisperTranscriber(BaseTranscriber):
    """Transcription backend powered by official openai-whisper PyTorch implementation."""

    name = "openai-whisper"

    def __init__(self, config: TranscriptionConfig) -> None:
        super().__init__(config)
        self._model = None

    @classmethod
    def is_available(cls) -> bool:
        """Check if openai-whisper package is installed."""
        return importlib.util.find_spec("whisper") is not None

    def initialize(self) -> None:
        """Load openai-whisper model into memory."""
        if self._is_loaded and self._model is not None:
            return

        if not self.is_available():
            raise RuntimeError(
                "openai-whisper backend is selected but the package is not installed.\n"
                "Install it using: uv pip install openai-whisper"
            )

        import whisper

        logger.info(
            "Loading openai-whisper model '%s' on %s...",
            self.config.model_size,
            self.config.device,
        )

        self._model = whisper.load_model(
            self.config.model_size,
            device=self.config.device if self.config.device != "auto" else None,
        )
        self._is_loaded = True
        logger.info("openai-whisper model '%s' loaded.", self.config.model_size)

    def transcribe_raw(self, audio_data: np.ndarray) -> str:
        """Transcribe audio using openai-whisper."""
        if self._model is None or not self._is_loaded:
            self.initialize()

        prompt = self.build_prompt()
        use_fp16 = (self.config.device == "cuda" and self.config.compute_type == "float16")

        result = self._model.transcribe(
            audio_data,
            initial_prompt=prompt,
            fp16=use_fp16,
        )
        return result.get("text", "").strip()

    def unload(self) -> None:
        """Unload PyTorch model and reclaim memory."""
        self._model = None
        self._is_loaded = False
        gc.collect()
        logger.debug("openai-whisper model unloaded.")
