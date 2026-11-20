"""Systemd user service integration for VoiceKey daemon (voicekeyd)."""

from __future__ import annotations

import logging
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any, Optional

logger = logging.getLogger("voicekey.service")

SERVICE_NAME = "voicekey.service"


def get_systemd_user_dir() -> Path:
    """Return user systemd service directory (~/.config/systemd/user)."""
    xdg_config = os.environ.get("XDG_CONFIG_HOME")
    if xdg_config:
        base = Path(xdg_config)
    else:
        base = Path.home() / ".config"
    return base / "systemd" / "user"


def get_service_file_path() -> Path:
    """Return the full path to the voicekey.service unit file."""
    return get_systemd_user_dir() / SERVICE_NAME


def find_daemon_executable() -> str:
    """Determine the optimal command string to launch voicekeyd."""
    # 1. Check virtualenv bin directory
    venv_bin = Path(sys.prefix) / "bin" / "voicekeyd"
    if venv_bin.is_file() and os.access(venv_bin, os.X_OK):
        return str(venv_bin.resolve())

    # 2. Check system PATH
    which_path = shutil.which("voicekeyd")
    if which_path:
        return str(Path(which_path).resolve())

    # 3. Fallback to current python module execution
    return f"{sys.executable} -m voicekey.daemon"


def generate_service_unit(exec_cmd: Optional[str] = None) -> str:
    """Generate systemd user unit file contents."""
    command = exec_cmd or find_daemon_executable()
    return f"""[Unit]
Description=VoiceKey Speech-to-Text Background Daemon
Documentation=https://github.com/aryanvasudev/VoiceKey
PartOf=graphical-session.target
After=graphical-session.target pipewire.service sound.target

[Service]
Type=simple
ExecStart={command}
Restart=on-failure
RestartSec=3s
Environment=PYTHONUNBUFFERED=1

[Install]
WantedBy=default.target
"""


def _run_systemctl(args: list[str]) -> subprocess.CompletedProcess[str]:
    """Execute a systemctl --user command."""
    cmd = ["systemctl", "--user"] + args
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def install_service(
    enable: bool = True,
    start: bool = False,
    exec_cmd: Optional[str] = None,
) -> Path:
    """Install voicekey.service to the user systemd directory."""
    unit_dir = get_systemd_user_dir()
    unit_dir.mkdir(parents=True, exist_ok=True)

    unit_path = get_service_file_path()
    content = generate_service_unit(exec_cmd=exec_cmd)
    unit_path.write_text(content, encoding="utf-8")
    logger.info("Wrote systemd service to %s", unit_path)

    # Reload systemd manager configuration
    _run_systemctl(["daemon-reload"])

    if enable:
        res = _run_systemctl(["enable", SERVICE_NAME])
        if res.returncode != 0:
            logger.warning("systemctl --user enable failed: %s", res.stderr)

    if start:
        res = _run_systemctl(["start", SERVICE_NAME])
        if res.returncode != 0:
            logger.warning("systemctl --user start failed: %s", res.stderr)

    return unit_path


def uninstall_service(stop: bool = True) -> bool:
    """Stop, disable, and remove voicekey.service."""
    unit_path = get_service_file_path()

    if stop:
        _run_systemctl(["stop", SERVICE_NAME])

    _run_systemctl(["disable", SERVICE_NAME])

    removed = False
    if unit_path.exists():
        try:
            unit_path.unlink()
            removed = True
        except OSError as e:
            logger.error("Failed to remove unit file %s: %s", unit_path, e)

    _run_systemctl(["daemon-reload"])
    return removed


def get_service_status() -> dict[str, Any]:
    """Inspect and return current status of voicekey.service."""
    unit_path = get_service_file_path()
    installed = unit_path.is_file()

    enabled = False
    active = False
    status_output = ""

    if shutil.which("systemctl"):
        is_enabled = _run_systemctl(["is-enabled", SERVICE_NAME])
        enabled = is_enabled.returncode == 0 and "enabled" in is_enabled.stdout.lower()

        is_active = _run_systemctl(["is-active", SERVICE_NAME])
        active = is_active.returncode == 0 and "active" in is_active.stdout.lower()

        status_res = _run_systemctl(["status", SERVICE_NAME])
        status_output = status_res.stdout.strip() or status_res.stderr.strip()

    return {
        "installed": installed,
        "path": str(unit_path),
        "enabled": enabled,
        "active": active,
        "status_output": status_output,
    }
