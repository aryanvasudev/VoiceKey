"""Tests for VoiceKey systemd user service manager."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
from unittest.mock import MagicMock, patch

import pytest

from voicekey.service import (
    SERVICE_NAME,
    find_daemon_executable,
    generate_service_unit,
    get_service_file_path,
    get_service_status,
    get_systemd_user_dir,
    install_service,
    uninstall_service,
)


def test_get_systemd_user_dir_custom_xdg(monkeypatch, tmp_path):
    custom_xdg = tmp_path / "custom_config"
    monkeypatch.setenv("XDG_CONFIG_HOME", str(custom_xdg))
    res = get_systemd_user_dir()
    assert res == custom_xdg / "systemd" / "user"


def test_get_systemd_user_dir_default(monkeypatch, tmp_path):
    monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
    monkeypatch.setattr(Path, "home", lambda: tmp_path)
    res = get_systemd_user_dir()
    assert res == tmp_path / ".config" / "systemd" / "user"


def test_find_daemon_executable_venv(monkeypatch, tmp_path):
    mock_bin = tmp_path / "bin" / "voicekeyd"
    mock_bin.parent.mkdir(parents=True)
    mock_bin.touch(mode=0o755)

    with patch("voicekey.service.sys.prefix", str(tmp_path)):
        with patch("os.access", return_value=True):
            cmd = find_daemon_executable()
            assert cmd == str(mock_bin.resolve())


def test_find_daemon_executable_which(monkeypatch, tmp_path):
    with patch("voicekey.service.Path.is_file", return_value=False):
        with patch("shutil.which", return_value="/usr/bin/voicekeyd"):
            cmd = find_daemon_executable()
            assert cmd == "/usr/bin/voicekeyd"


def test_generate_service_unit():
    unit = generate_service_unit("/usr/bin/voicekeyd")
    assert "[Unit]" in unit
    assert "Description=VoiceKey Speech-to-Text Background Daemon" in unit
    assert "ExecStart=/usr/bin/voicekeyd" in unit
    assert "Restart=on-failure" in unit
    assert "[Install]" in unit
    assert "WantedBy=default.target" in unit


def test_install_service(tmp_path, monkeypatch):
    custom_dir = tmp_path / "systemd" / "user"
    monkeypatch.setattr("voicekey.service.get_systemd_user_dir", lambda: custom_dir)

    with patch("voicekey.service._run_systemctl") as mock_systemctl:
        mock_systemctl.return_value = subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")

        unit_path = install_service(enable=True, start=True, exec_cmd="/usr/bin/voicekeyd")

        assert unit_path.is_file()
        assert unit_path.name == SERVICE_NAME
        assert "ExecStart=/usr/bin/voicekeyd" in unit_path.read_text()

        # Check systemctl calls
        calls = [c[0][0] for c in mock_systemctl.call_args_list]
        assert ["daemon-reload"] in calls
        assert ["enable", SERVICE_NAME] in calls
        assert ["start", SERVICE_NAME] in calls


def test_uninstall_service(tmp_path, monkeypatch):
    custom_dir = tmp_path / "systemd" / "user"
    custom_dir.mkdir(parents=True)
    unit_path = custom_dir / SERVICE_NAME
    unit_path.touch()

    monkeypatch.setattr("voicekey.service.get_service_file_path", lambda: unit_path)

    with patch("voicekey.service._run_systemctl") as mock_systemctl:
        mock_systemctl.return_value = subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")

        removed = uninstall_service(stop=True)
        assert removed is True
        assert not unit_path.exists()

        calls = [c[0][0] for c in mock_systemctl.call_args_list]
        assert ["stop", SERVICE_NAME] in calls
        assert ["disable", SERVICE_NAME] in calls
        assert ["daemon-reload"] in calls


def test_get_service_status(tmp_path, monkeypatch):
    unit_path = tmp_path / SERVICE_NAME
    unit_path.touch()
    monkeypatch.setattr("voicekey.service.get_service_file_path", lambda: unit_path)

    with patch("shutil.which", return_value="/bin/systemctl"):
        with patch("voicekey.service._run_systemctl") as mock_systemctl:
            def fake_systemctl(args):
                if args[0] == "is-enabled":
                    return subprocess.CompletedProcess(args=[], returncode=0, stdout="enabled\n", stderr="")
                elif args[0] == "is-active":
                    return subprocess.CompletedProcess(args=[], returncode=0, stdout="active\n", stderr="")
                elif args[0] == "status":
                    return subprocess.CompletedProcess(args=[], returncode=0, stdout="Active: active (running)", stderr="")
                return subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")

            mock_systemctl.side_effect = fake_systemctl

            status = get_service_status()
            assert status["installed"] is True
            assert status["enabled"] is True
            assert status["active"] is True
            assert "running" in status["status_output"]
