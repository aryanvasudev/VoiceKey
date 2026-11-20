"""VoiceKey - Lightweight hotkey-driven speech-to-text dictation for Linux Wayland."""

__version__ = "0.1.0"

from voicekey.audio import AudioRecorder
from voicekey.config import (
    AudioConfig,
    DaemonConfig,
    OutputConfig,
    TranscriptionConfig,
    VoiceKeyConfig,
    load_config,
    save_config,
)
from voicekey.feedback import AudioFeedback, SoundCue
from voicekey.notify import send_notification

__all__ = [
    "__version__",
    "AudioConfig",
    "AudioFeedback",
    "AudioRecorder",
    "DaemonConfig",
    "OutputConfig",
    "SoundCue",
    "TranscriptionConfig",
    "VoiceKeyConfig",
    "load_config",
    "save_config",
    "send_notification",
]
