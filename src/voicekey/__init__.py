"""VoiceKey - Lightweight hotkey-driven speech-to-text dictation for Linux Wayland."""

__version__ = "0.1.0"

from voicekey.config import (
    AudioConfig,
    DaemonConfig,
    OutputConfig,
    TranscriptionConfig,
    VoiceKeyConfig,
    load_config,
    save_config,
)

__all__ = [
    "__version__",
    "AudioConfig",
    "DaemonConfig",
    "OutputConfig",
    "TranscriptionConfig",
    "VoiceKeyConfig",
    "load_config",
    "save_config",
]
