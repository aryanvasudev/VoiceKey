"""Subtle audio feedback chimes for recording start/stop transitions."""

from __future__ import annotations

import logging
import math
import shutil
import struct
import subprocess
import tempfile
import wave
from enum import Enum
from pathlib import Path

logger = logging.getLogger("voicekey.feedback")


class SoundCue(str, Enum):
    """Audio cue types."""
    START = "start"
    STOP = "stop"
    ERROR = "error"


# Common standard sound cues on Linux freedesktop / GNOME systems
SYSTEM_SOUND_CANDIDATES: dict[SoundCue, list[Path]] = {
    SoundCue.START: [
        Path("/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga"),
        Path("/usr/share/sounds/gnome/default/alerts/click.ogg"),
        Path("/usr/share/sounds/freedesktop/stereo/camera-shutter.oga"),
    ],
    SoundCue.STOP: [
        Path("/usr/share/sounds/freedesktop/stereo/complete.oga"),
        Path("/usr/share/sounds/freedesktop/stereo/message.oga"),
        Path("/usr/share/sounds/freedesktop/stereo/bell.oga"),
    ],
    SoundCue.ERROR: [
        Path("/usr/share/sounds/freedesktop/stereo/suspend-error.oga"),
        Path("/usr/share/sounds/freedesktop/stereo/dialog-warning.oga"),
    ],
}


def _generate_wav_beep(
    path: Path,
    start_freq: float,
    end_freq: float,
    duration_s: float = 0.06,
    sample_rate: int = 16000,
) -> None:
    """Generate a tiny sine wave audio beep and save as WAV file."""
    num_samples = int(sample_rate * duration_s)
    path.parent.mkdir(parents=True, exist_ok=True)

    with wave.open(str(path), "wb") as wav_file:
        wav_file.setnchannels(1)  # Mono
        wav_file.setsampwidth(2)  # 16-bit
        wav_file.setframerate(sample_rate)

        frames = bytearray()
        for i in range(num_samples):
            t = i / sample_rate
            # Linear frequency sweep with smooth cosine fade in/out
            freq = start_freq + (end_freq - start_freq) * (i / num_samples)
            phase = 2.0 * math.pi * freq * t
            envelope = math.sin(math.pi * i / num_samples)  # Smooth fade-in and fade-out
            amplitude = int(12000 * envelope * math.sin(phase))
            frames.extend(struct.pack("<h", amplitude))

        wav_file.writeframes(frames)


class AudioFeedback:
    """Non-blocking audio feedback player using PipeWire pw-play, aplay, or synthetic beeps."""

    def __init__(self, enabled: bool = True) -> None:
        self.enabled = enabled
        self.player_bin = shutil.which("pw-play") or shutil.which("aplay")
        self._custom_tones_dir = Path(tempfile.gettempdir()) / "voicekey_sounds"
        self._sound_paths: dict[SoundCue, Path | None] = {}
        self._initialize_sounds()

    def _initialize_sounds(self) -> None:
        """Resolve available audio files for each sound cue or synthesize fallback WAVs."""
        for cue, candidates in SYSTEM_SOUND_CANDIDATES.items():
            found: Path | None = None
            for candidate in candidates:
                if candidate.exists():
                    found = candidate
                    break

            if found is None:
                # Generate synthetic fallback tone
                wav_path = self._custom_tones_dir / f"{cue.value}.wav"
                try:
                    if not wav_path.exists():
                        if cue == SoundCue.START:
                            _generate_wav_beep(wav_path, start_freq=587.33, end_freq=880.0, duration_s=0.05)  # D5 -> A5 chirp
                        elif cue == SoundCue.STOP:
                            _generate_wav_beep(wav_path, start_freq=880.0, end_freq=659.25, duration_s=0.06)  # A5 -> E5 chime
                        else:
                            _generate_wav_beep(wav_path, start_freq=350.0, end_freq=220.0, duration_s=0.1)   # Low error buzz
                    found = wav_path
                except Exception as e:
                    logger.warning("Failed to synthesize tone for %s: %s", cue, e)

            self._sound_paths[cue] = found

    def play(self, cue: SoundCue) -> bool:
        """Play sound cue asynchronously without blocking the caller."""
        if not self.enabled:
            return False

        if not self.player_bin:
            logger.debug("No audio player found (pw-play or aplay); skipping sound cue.")
            return False

        sound_path = self._sound_paths.get(cue)
        if not sound_path or not sound_path.exists():
            logger.debug("Sound file for cue %s not found.", cue)
            return False

        try:
            subprocess.Popen(
                [self.player_bin, str(sound_path)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            return True
        except Exception as e:
            logger.warning("Failed to play sound cue %s: %s", cue, e)
            return False

    def play_start(self) -> bool:
        """Play recording started cue."""
        return self.play(SoundCue.START)

    def play_stop(self) -> bool:
        """Play recording stopped / transcription begun cue."""
        return self.play(SoundCue.STOP)

    def play_error(self) -> bool:
        """Play error cue."""
        return self.play(SoundCue.ERROR)
