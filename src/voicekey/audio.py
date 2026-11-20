"""Audio capture engine using sounddevice and PipeWire/ALSA."""

from __future__ import annotations

import logging
import threading
import time
from typing import Callable
import numpy as np
import sounddevice as sd

from voicekey.config import AudioConfig
from voicekey.feedback import AudioFeedback

logger = logging.getLogger("voicekey.audio")


class AudioRecorder:
    """Thread-safe microphone audio recorder buffering 16kHz mono audio into RAM."""

    def __init__(
        self,
        config: AudioConfig | None = None,
        feedback: AudioFeedback | None = None,
        device: int | str | None = None,
    ) -> None:
        self.config = config or AudioConfig()
        self.feedback = feedback if feedback is not None else AudioFeedback(enabled=self.config.sound_feedback)
        self.device = device

        self._stream: sd.InputStream | None = None
        self._lock = threading.Lock()
        self._frames: list[np.ndarray] = []
        self._is_recording = False
        self._start_time: float | None = None

    @property
    def is_recording(self) -> bool:
        """Check if currently recording."""
        with self._lock:
            return self._is_recording

    def start(self, play_cue: bool = True) -> bool:
        """Start capturing audio stream from microphone.

        Returns True if recording started, False if already recording.
        """
        with self._lock:
            if self._is_recording:
                logger.warning("Recorder is already running.")
                return False

            self._frames.clear()
            self._start_time = time.monotonic()
            self._is_recording = True

            try:
                self._stream = sd.InputStream(
                    samplerate=self.config.sample_rate,
                    channels=self.config.channels,
                    dtype="float32",
                    device=self.device,
                    callback=self._audio_callback,
                )
                self._stream.start()
            except Exception as e:
                self._is_recording = False
                self._stream = None
                logger.error("Failed to start audio input stream: %s", e)
                if play_cue:
                    self.feedback.play_error()
                raise

        if play_cue:
            self.feedback.play_start()

        logger.debug("Audio recording started (sample_rate=%d, channels=%d).", self.config.sample_rate, self.config.channels)
        return True

    def _audio_callback(
        self,
        indata: np.ndarray,
        frames: int,
        time_info: dict,
        status: sd.CallbackFlags,
    ) -> None:
        """Callback invoked by PortAudio / sounddevice audio thread."""
        if status:
            logger.warning("Audio callback status warning: %s", status)

        with self._lock:
            if self._is_recording:
                # indata is shape (frames, channels), copy and flatten to 1D
                self._frames.append(indata.copy().flatten())

    def stop(self, play_cue: bool = True) -> np.ndarray:
        """Stop audio capture, close stream, and return accumulated audio as 1D float32 array.

        Returns:
            np.ndarray: 1D float32 array normalized to [-1.0, 1.0] ready for faster-whisper.
        """
        with self._lock:
            if not self._is_recording:
                logger.warning("Recorder is not recording.")
                return np.empty(0, dtype=np.float32)

            self._is_recording = False
            stream = self._stream
            self._stream = None

            if stream is not None:
                try:
                    stream.stop()
                    stream.close()
                except Exception as e:
                    logger.warning("Error closing audio stream: %s", e)

            if not self._frames:
                audio_data = np.empty(0, dtype=np.float32)
            else:
                audio_data = np.concatenate(self._frames).astype(np.float32)

            self._frames.clear()

        if play_cue:
            self.feedback.play_stop()

        duration = len(audio_data) / self.config.sample_rate if self.config.sample_rate > 0 else 0.0
        logger.debug("Audio recording stopped. Duration: %.2fs (%d samples).", duration, len(audio_data))
        return audio_data

    @staticmethod
    def calculate_rms(audio_data: np.ndarray) -> float:
        """Calculate Root Mean Square (RMS) volume amplitude of audio data."""
        if len(audio_data) == 0:
            return 0.0
        return float(np.sqrt(np.mean(np.square(audio_data))))

    @staticmethod
    def calculate_peak(audio_data: np.ndarray) -> float:
        """Calculate peak absolute amplitude of audio data."""
        if len(audio_data) == 0:
            return 0.0
        return float(np.max(np.abs(audio_data)))

    @staticmethod
    def to_pcm16_bytes(audio_data: np.ndarray) -> bytes:
        """Convert float32 normalized audio array to 16-bit signed integer PCM bytes."""
        clipped = np.clip(audio_data, -1.0, 1.0)
        pcm16 = (clipped * 32767.0).astype(np.int16)
        return pcm16.tobytes()
