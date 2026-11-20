"""Configuration management for VoiceKey."""

from __future__ import annotations

import os
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any
import tomllib

import tomli_w


DEFAULT_CONFIG_DIR_NAME = "voicekey"
DEFAULT_CONFIG_FILE_NAME = "config.toml"
DEFAULT_STATE_DIR_NAME = "voicekey"
DEFAULT_SOCKET_FILE_NAME = "daemon.sock"


def get_default_config_dir() -> Path:
    """Return default configuration directory (~/.config/voicekey or $XDG_CONFIG_HOME/voicekey)."""
    env_dir = os.environ.get("VOICEKEY_CONFIG_DIR")
    if env_dir:
        return Path(env_dir).expanduser().resolve()

    xdg_config_home = os.environ.get("XDG_CONFIG_HOME")
    if xdg_config_home:
        return Path(xdg_config_home).expanduser() / DEFAULT_CONFIG_DIR_NAME

    return Path.home() / ".config" / DEFAULT_CONFIG_DIR_NAME


def get_default_config_path() -> Path:
    """Return default configuration file path."""
    return get_default_config_dir() / DEFAULT_CONFIG_FILE_NAME


def get_default_state_dir() -> Path:
    """Return default state directory (~/.local/state/voicekey or $XDG_STATE_HOME/voicekey)."""
    env_dir = os.environ.get("VOICEKEY_STATE_DIR")
    if env_dir:
        return Path(env_dir).expanduser().resolve()

    xdg_state_home = os.environ.get("XDG_STATE_HOME")
    if xdg_state_home:
        return Path(xdg_state_home).expanduser() / DEFAULT_STATE_DIR_NAME

    return Path.home() / ".local" / "state" / DEFAULT_STATE_DIR_NAME


def get_default_socket_path() -> Path:
    """Return default unix socket file path."""
    return get_default_state_dir() / DEFAULT_SOCKET_FILE_NAME


@dataclass
class TranscriptionConfig:
    """Settings for faster-whisper model and inference."""

    model_size: str = "base.en"
    device: str = "cpu"
    compute_type: str = "int8"
    beam_size: int = 1
    vad_filter: bool = True
    cpu_threads: int = 8


@dataclass
class AudioConfig:
    """Settings for audio input capture and feedback sounds."""

    sample_rate: int = 16000
    channels: int = 1
    sound_feedback: bool = True


@dataclass
class OutputConfig:
    """Settings for clipboard and paste output dispatch."""

    mode: str = "auto_paste"  # "auto_paste" or "clipboard_only"
    paste_delay_ms: int = 50
    notify: bool = True


@dataclass
class DaemonConfig:
    """Settings for the background daemon and IPC."""

    socket_path: str = str(get_default_socket_path())


@dataclass
class VoiceKeyConfig:
    """Root configuration holding all sub-configurations."""

    transcription: TranscriptionConfig = field(default_factory=TranscriptionConfig)
    audio: AudioConfig = field(default_factory=AudioConfig)
    output: OutputConfig = field(default_factory=OutputConfig)
    daemon: DaemonConfig = field(default_factory=DaemonConfig)

    def to_dict(self) -> dict[str, Any]:
        """Convert configuration to dictionary suitable for TOML serialization."""
        return asdict(self)

    def resolved_socket_path(self) -> Path:
        """Return expanded Path object for the daemon socket."""
        return Path(self.daemon.socket_path).expanduser().resolve()


def from_dict(data: dict[str, Any]) -> VoiceKeyConfig:
    """Instantiate VoiceKeyConfig safely from dictionary, falling back to defaults."""
    transcription_data = data.get("transcription", {})
    audio_data = data.get("audio", {})
    output_data = data.get("output", {})
    daemon_data = data.get("daemon", {})

    transcription = TranscriptionConfig(
        model_size=str(transcription_data.get("model_size", "base.en")),
        device=str(transcription_data.get("device", "cpu")),
        compute_type=str(transcription_data.get("compute_type", "int8")),
        beam_size=int(transcription_data.get("beam_size", 1)),
        vad_filter=bool(transcription_data.get("vad_filter", True)),
        cpu_threads=int(transcription_data.get("cpu_threads", 8)),
    )

    audio = AudioConfig(
        sample_rate=int(audio_data.get("sample_rate", 16000)),
        channels=int(audio_data.get("channels", 1)),
        sound_feedback=bool(audio_data.get("sound_feedback", True)),
    )

    output = OutputConfig(
        mode=str(output_data.get("mode", "auto_paste")),
        paste_delay_ms=int(output_data.get("paste_delay_ms", 50)),
        notify=bool(output_data.get("notify", True)),
    )

    default_sock = str(get_default_socket_path())
    daemon = DaemonConfig(
        socket_path=str(daemon_data.get("socket_path", default_sock)),
    )

    return VoiceKeyConfig(
        transcription=transcription,
        audio=audio,
        output=output,
        daemon=daemon,
    )


def validate_config(config: VoiceKeyConfig) -> list[str]:
    """Validate configuration values and return a list of error messages (empty if valid)."""
    errors: list[str] = []

    if not config.transcription.model_size.strip():
        errors.append("transcription.model_size cannot be empty")

    if config.transcription.device not in {"cpu", "cuda", "auto"}:
        errors.append(f"Invalid transcription.device: {config.transcription.device}. Must be 'cpu', 'cuda', or 'auto'")

    if config.transcription.beam_size < 1:
        errors.append("transcription.beam_size must be >= 1")

    if config.transcription.cpu_threads < 1:
        errors.append("transcription.cpu_threads must be >= 1")

    if config.audio.sample_rate <= 0:
        errors.append("audio.sample_rate must be > 0")

    if config.audio.channels not in {1, 2}:
        errors.append(f"audio.channels must be 1 (mono) or 2 (stereo), got {config.audio.channels}")

    if config.output.mode not in {"auto_paste", "clipboard_only"}:
        errors.append(f"Invalid output.mode: {config.output.mode}. Must be 'auto_paste' or 'clipboard_only'")

    if config.output.paste_delay_ms < 0:
        errors.append("output.paste_delay_ms cannot be negative")

    if not config.daemon.socket_path.strip():
        errors.append("daemon.socket_path cannot be empty")

    return errors


def save_config(config: VoiceKeyConfig, path: Path | None = None) -> Path:
    """Save configuration to TOML file, ensuring parent directories exist."""
    target_path = (path or get_default_config_path()).expanduser().resolve()
    target_path.parent.mkdir(parents=True, exist_ok=True)

    data = config.to_dict()
    toml_str = tomli_w.dumps(data)
    target_path.write_text(toml_str, encoding="utf-8")
    return target_path


def load_config(path: Path | None = None, create_if_missing: bool = False) -> VoiceKeyConfig:
    """Load configuration from TOML file.

    If file does not exist:
    - if create_if_missing is True: writes default config to disk and returns it.
    - if create_if_missing is False: returns default config instance without writing.
    """
    target_path = (path or get_default_config_path()).expanduser().resolve()

    if not target_path.exists():
        default_config = VoiceKeyConfig()
        if create_if_missing:
            save_config(default_config, target_path)
        return default_config

    with open(target_path, "rb") as f:
        data = tomllib.load(f)

    config = from_dict(data)
    errors = validate_config(config)
    if errors:
        raise ValueError(f"Configuration validation failed in {target_path}:\n - " + "\n - ".join(errors))

    return config
