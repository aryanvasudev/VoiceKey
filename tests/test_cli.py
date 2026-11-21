"""Tests for VoiceKey CLI and Daemon entry points."""

from pathlib import Path
import pytest

from voicekey import cli, daemon


def test_cli_version(capsys: pytest.CaptureFixture[str]):
    """CLI prints version and exits."""
    with pytest.raises(SystemExit) as exc:
        cli.main(["--version"])
    assert exc.value.code == 0
    captured = capsys.readouterr()
    assert "voicekey 0.1.0" in captured.out


def test_daemon_version(capsys: pytest.CaptureFixture[str]):
    """Daemon prints version and exits."""
    with pytest.raises(SystemExit) as exc:
        daemon.main(["--version"])
    assert exc.value.code == 0
    captured = capsys.readouterr()
    assert "voicekeyd 0.1.0" in captured.out


def test_cli_no_args_prints_help(capsys: pytest.CaptureFixture[str]):
    """Running CLI with no arguments prints help text and exits cleanly."""
    exit_code = cli.main([])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "usage: voicekey" in captured.out


def test_cli_config_path(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """`voicekey config --path` prints the active configuration path."""
    conf = tmp_path / "custom.toml"
    exit_code = cli.main(["--config", str(conf), "config", "--path"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert str(conf) in captured.out


def test_cli_config_init_and_show(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """`voicekey config --init` creates config and `--show` outputs valid TOML."""
    conf = tmp_path / "sub" / "config.toml"
    assert not conf.exists()

    # Initialize
    exit_code = cli.main(["--config", str(conf), "config", "--init"])
    assert exit_code == 0
    assert conf.exists()

    captured = capsys.readouterr()
    assert "Initialized default configuration" in captured.out

    # Re-running init informs user it exists
    exit_code = cli.main(["--config", str(conf), "config", "--init"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "already exists" in captured.out

    # Show config
    exit_code = cli.main(["--config", str(conf), "config", "--show"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert 'model_size = "base.en"' in captured.out
    assert 'mode = "auto_paste"' in captured.out


def test_cli_socket_missing_error(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """Invoking daemon commands when daemon socket is absent exits with code 1."""
    conf = tmp_path / "test_config.toml"
    sock = tmp_path / "nonexistent.sock"
    conf.write_text(f'[daemon]\nsocket_path = "{sock}"\n', encoding="utf-8")

    exit_code = cli.main(["--config", str(conf), "toggle"])
    assert exit_code == 1
    captured = capsys.readouterr()
    assert "Daemon is not running" in captured.err


def test_cli_daemon_commands_mocked(capsys: pytest.CaptureFixture[str]):
    """CLI formats status, toggle, start, and stop responses cleanly."""
    from unittest.mock import patch

    # Status
    with patch("voicekey.ipc.send_daemon_command", return_value={
        "status": "ok",
        "state": "idle",
        "backend": "faster-whisper",
        "model_size": "base.en",
        "device": "cpu",
        "model_warm": True,
        "uptime_seconds": 15.5,
        "version": "0.1.0",
    }):
        assert cli.main(["status"]) == 0
        captured = capsys.readouterr()
        assert "VoiceKey Daemon Status:" in captured.out
        assert "IDLE" in captured.out
        assert "faster-whisper" in captured.out

    # Toggle (start recording)
    with patch("voicekey.ipc.send_daemon_command", return_value={"status": "ok", "state": "recording"}):
        assert cli.main(["toggle"]) == 0
        captured = capsys.readouterr()
        assert "Recording started" in captured.out

    # Toggle (stop recording and transcribed)
    with patch("voicekey.ipc.send_daemon_command", return_value={
        "status": "ok",
        "state": "idle",
        "text": "dictated sentence",
        "duration": 2.1,
    }):
        assert cli.main(["toggle"]) == 0
        captured = capsys.readouterr()
        assert "Recording stopped (2.1s)" in captured.out
        assert "dictated sentence" in captured.out

    # Start
    with patch("voicekey.ipc.send_daemon_command", return_value={"status": "ok"}):
        assert cli.main(["start"]) == 0
        captured = capsys.readouterr()
        assert "Recording started" in captured.out

    # Stop
    with patch("voicekey.ipc.send_daemon_command", return_value={"status": "ok", "text": "result", "duration": 1.0}):
        assert cli.main(["stop"]) == 0
        captured = capsys.readouterr()
        assert "Stopped (1.0s)" in captured.out


def test_daemon_main_initializes(tmp_path: Path):
    """Daemon parses parameters and runs initialization without crashing."""
    from unittest.mock import patch
    conf = tmp_path / "daemon_config.toml"
    sock = tmp_path / "daemon.sock"

    with patch("voicekey.daemon.VoiceKeyDaemon.start", return_value=None):
        exit_code = daemon.main(["--config", str(conf), "--socket", str(sock), "--debug"])
        assert exit_code == 0
        assert conf.exists()


def test_cli_test_audio(capsys: pytest.CaptureFixture[str]):
    """`voicekey test-audio` runs audio test routine and outputs metrics."""
    from unittest.mock import MagicMock, patch
    import numpy as np

    mock_rec = MagicMock()
    mock_rec.stop.return_value = np.full(16000, 0.05, dtype=np.float32)

    mock_cls = MagicMock()
    mock_cls.return_value = mock_rec
    mock_cls.calculate_rms.return_value = 0.05
    mock_cls.calculate_peak.return_value = 0.1

    with patch("voicekey.audio.AudioRecorder", mock_cls), \
         patch("time.sleep", return_value=None), \
         patch("voicekey.notify.send_notification", return_value=True):
        exit_code = cli.main(["test-audio", "--seconds", "0.1"])
        assert exit_code == 0

    captured = capsys.readouterr()
    assert "Testing audio recording" in captured.out
    assert "RMS Level:" in captured.out
    assert "Audio input detected successfully" in captured.out


def test_cli_doctor(capsys: pytest.CaptureFixture[str]):
    """`voicekey doctor` prints system hardware analysis and recommendations."""
    exit_code = cli.main(["doctor"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "VoiceKey Hardware & Engine Doctor" in captured.out
    assert "RECOMMENDED CONFIGURATION" in captured.out


def test_cli_doctor_apply(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """`voicekey doctor --apply` writes recommendations to config."""
    conf = tmp_path / "doctor_config.toml"
    exit_code = cli.main(["--config", str(conf), "doctor", "--apply"])
    assert exit_code == 0
    assert conf.exists()

    captured = capsys.readouterr()
    assert "Successfully applied recommended settings" in captured.out


def test_cli_test_paste(capsys: pytest.CaptureFixture[str]):
    """`voicekey test-paste` runs paste dispatch routine and prints results."""
    from unittest.mock import patch

    with patch("time.sleep", return_value=None), \
         patch("voicekey.paste.check_uinput_accessible", return_value=(True, "OK")), \
         patch("voicekey.paste.dispatch_paste", return_value=(True, "pasted")), \
         patch("voicekey.notify.send_notification", return_value=True):
        exit_code = cli.main(["test-paste", "--text", "sample text"])
        assert exit_code == 0

    captured = capsys.readouterr()
    assert "Testing output dispatch with text: 'sample text'" in captured.out
    assert "Action taken:     pasted" in captured.out


def test_cli_autostart_install(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """`voicekey autostart --install` invokes install_service and reports path."""
    from unittest.mock import patch

    with patch("voicekey.service.install_service", return_value=tmp_path / "voicekey.service"):
        exit_code = cli.main(["autostart", "--install"])
        assert exit_code == 0

    captured = capsys.readouterr()
    assert "Systemd user service installed to:" in captured.out
    assert "Enabled autostart on user login" in captured.out


def test_cli_autostart_remove(capsys: pytest.CaptureFixture[str]):
    """`voicekey autostart --remove` invokes uninstall_service and reports outcome."""
    from unittest.mock import patch

    with patch("voicekey.service.uninstall_service", return_value=True):
        exit_code = cli.main(["autostart", "--remove"])
        assert exit_code == 0

    captured = capsys.readouterr()
    assert "Systemd user service stopped, disabled, and removed" in captured.out


def test_cli_autostart_start_stop(capsys: pytest.CaptureFixture[str]):
    """`voicekey autostart --start` and `--stop` invoke systemctl."""
    import subprocess
    from unittest.mock import patch

    mock_ok = subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")
    with patch("voicekey.service._run_systemctl", return_value=mock_ok):
        assert cli.main(["autostart", "--start"]) == 0
        captured = capsys.readouterr()
        assert "Started voicekey.service" in captured.out

        assert cli.main(["autostart", "--stop"]) == 0
        captured = capsys.readouterr()
        assert "Stopped voicekey.service" in captured.out


def test_cli_autostart_status(capsys: pytest.CaptureFixture[str]):
    """`voicekey autostart [--status]` displays service status information."""
    from unittest.mock import patch

    mock_status = {
        "installed": True,
        "path": "/home/user/.config/systemd/user/voicekey.service",
        "enabled": True,
        "active": False,
        "status_output": "Inactive",
    }
    with patch("voicekey.service.get_service_status", return_value=mock_status):
        assert cli.main(["autostart", "--status"]) == 0

    captured = capsys.readouterr()
    assert "Systemd User Service Status:" in captured.out
    assert "Found" in captured.out
    assert "Yes (starts on login)" in captured.out


def test_cli_setup(capsys: pytest.CaptureFixture[str], tmp_path: Path):
    """`voicekey setup` runs automated hardware tuning, service install, and reports completion."""
    from unittest.mock import patch

    conf = tmp_path / "config.toml"
    with patch("voicekey.service.install_service", return_value=conf):
        exit_code = cli.main(["--config", str(conf), "setup", "--skip-hotkey"])
        assert exit_code == 0

    captured = capsys.readouterr()
    assert "Running automated setup" in captured.out
    assert "Hardware auto-tuned" in captured.out
    assert "Systemd user service installed and started" in captured.out
    assert "Setup complete" in captured.out


