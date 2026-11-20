"""Tests for VoiceKey configuration module."""

from pathlib import Path
import pytest

from voicekey.config import (
    AudioConfig,
    DaemonConfig,
    OutputConfig,
    TranscriptionConfig,
    VoiceKeyConfig,
    from_dict,
    get_default_config_dir,
    get_default_state_dir,
    load_config,
    save_config,
    validate_config,
)


def test_default_config_values():
    """Verify default configuration values adhere to system specs."""
    config = VoiceKeyConfig()

    assert config.transcription.backend == "faster-whisper"
    assert config.transcription.model_size == "base.en"
    assert config.transcription.device == "cpu"
    assert config.transcription.compute_type == "int8"
    assert config.transcription.beam_size == 1
    assert config.transcription.vad_filter is True
    assert config.transcription.cpu_threads == 8
    assert config.transcription.initial_prompt == ""
    assert config.transcription.hotwords == []
    assert config.transcription.spoken_punctuation is False
    assert config.transcription.normalize_text is True

    assert config.audio.sample_rate == 16000
    assert config.audio.channels == 1
    assert config.audio.sound_feedback is True

    assert config.output.mode == "auto_paste"
    assert config.output.paste_delay_ms == 50
    assert config.output.notify is True

    assert config.daemon.socket_path.endswith("daemon.sock")


def test_save_and_load_roundtrip(tmp_path: Path):
    """Verify that saving and re-loading TOML configuration preserves all values."""
    config_file = tmp_path / "voicekey" / "config.toml"

    custom_config = VoiceKeyConfig(
        transcription=TranscriptionConfig(
            backend="whisper.cpp",
            model_size="small.en",
            device="cpu",
            compute_type="int8",
            beam_size=2,
            vad_filter=False,
            cpu_threads=4,
            initial_prompt="Code review",
            hotwords=["VoiceKey", "Wayland", "PipeWire"],
            spoken_punctuation=True,
            normalize_text=True,
        ),
        audio=AudioConfig(
            sample_rate=16000,
            channels=1,
            sound_feedback=False,
        ),
        output=OutputConfig(
            mode="clipboard_only",
            paste_delay_ms=100,
            notify=False,
        ),
        daemon=DaemonConfig(
            socket_path=str(tmp_path / "custom.sock"),
        ),
    )

    saved_path = save_config(custom_config, config_file)
    assert saved_path.exists()

    loaded_config = load_config(config_file)
    assert loaded_config == custom_config


def test_load_config_missing_file_no_create(tmp_path: Path):
    """When create_if_missing is False, loading a missing file returns defaults without writing."""
    non_existent = tmp_path / "does_not_exist.toml"
    assert not non_existent.exists()

    config = load_config(non_existent, create_if_missing=False)
    assert isinstance(config, VoiceKeyConfig)
    assert not non_existent.exists()


def test_load_config_missing_file_with_create(tmp_path: Path):
    """When create_if_missing is True, loading a missing file creates default config file."""
    target_path = tmp_path / "sub" / "auto_created.toml"
    assert not target_path.exists()

    config = load_config(target_path, create_if_missing=True)
    assert target_path.exists()
    assert config.transcription.model_size == "base.en"


def test_from_dict_partial_data():
    """Partial dictionary fills in missing fields with sensible defaults."""
    partial = {
        "transcription": {"model_size": "tiny.en"},
        "output": {"mode": "clipboard_only"},
    }

    config = from_dict(partial)
    assert config.transcription.model_size == "tiny.en"
    assert config.transcription.compute_type == "int8"
    assert config.audio.sample_rate == 16000
    assert config.output.mode == "clipboard_only"
    assert config.output.paste_delay_ms == 50


def test_validate_config_valid():
    """Valid config returns no errors."""
    config = VoiceKeyConfig()
    errors = validate_config(config)
    assert errors == []


def test_validate_config_invalid():
    """Invalid configurations produce descriptive validation errors."""
    bad_config = VoiceKeyConfig(
        transcription=TranscriptionConfig(backend="unsupported_backend", model_size="", device="invalid_device", beam_size=0, cpu_threads=0),
        audio=AudioConfig(sample_rate=-1, channels=3),
        output=OutputConfig(mode="unsupported_mode", paste_delay_ms=-10),
        daemon=DaemonConfig(socket_path=""),
    )

    errors = validate_config(bad_config)
    assert any("transcription.backend" in e for e in errors)
    assert any("model_size" in e for e in errors)
    assert any("device" in e for e in errors)
    assert any("beam_size" in e for e in errors)
    assert any("cpu_threads" in e for e in errors)
    assert any("sample_rate" in e for e in errors)
    assert any("channels" in e for e in errors)
    assert any("output.mode" in e for e in errors)
    assert any("paste_delay_ms" in e for e in errors)
    assert any("socket_path" in e for e in errors)


def test_load_config_validation_failure(tmp_path: Path):
    """Loading invalid TOML content raises ValueError."""
    bad_file = tmp_path / "invalid.toml"
    bad_file.write_text('[output]\nmode = "bad_mode"\n', encoding="utf-8")

    with pytest.raises(ValueError, match="Configuration validation failed"):
        load_config(bad_file)


def test_env_var_overrides(monkeypatch: pytest.MonkeyPatch, tmp_path: Path):
    """Environment variables VOICEKEY_CONFIG_DIR and VOICEKEY_STATE_DIR override defaults."""
    custom_conf_dir = tmp_path / "my_config"
    custom_state_dir = tmp_path / "my_state"

    monkeypatch.setenv("VOICEKEY_CONFIG_DIR", str(custom_conf_dir))
    monkeypatch.setenv("VOICEKEY_STATE_DIR", str(custom_state_dir))

    assert get_default_config_dir() == custom_conf_dir
    assert get_default_state_dir() == custom_state_dir


def test_resolved_socket_path():
    """Daemon socket path is expanded to absolute Path."""
    config = VoiceKeyConfig(daemon=DaemonConfig(socket_path="~/test.sock"))
    resolved = config.resolved_socket_path()
    assert resolved.is_absolute()
    assert resolved.name == "test.sock"
