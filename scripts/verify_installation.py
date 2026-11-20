#!/usr/bin/env python3
"""VoiceKey - Complete Environment & Installation Verification Script.

Runs interactive checks across all subsystem requirements:
1. Audio input capture (sounddevice / PipeWire)
2. Sound feedback cues (pw-play)
3. Desktop notifications (notify-send)
4. Wayland clipboard utility (wl-copy)
5. Linux virtual key injector (/dev/uinput)
6. Systemd user autostart service (voicekey.service)
7. GNOME custom shortcut configuration (gsettings)
8. Background daemon process & IPC socket (voicekeyd)
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

# Add project root to sys.path
SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from voicekey.config import load_config
from voicekey.doctor import probe_hardware
from voicekey.ipc import is_socket_alive, send_daemon_command

from voicekey.paste import check_uinput_accessible
from voicekey.service import get_service_status


def check_mark(ok: bool) -> str:
    return "✅ [PASS]" if ok else "⚠️ [FAIL]"


def main() -> int:
    print("=" * 65)
    print(" VoiceKey Installation & Environment Verification")
    print("=" * 65)

    failures = 0
    config = load_config(create_if_missing=False)

    # 1. Audio input
    print("\n1. Audio Input Subsystem:")
    has_audio = False
    try:
        import sounddevice as sd
        devices = sd.query_devices()
        input_devs = [d for d in devices if d.get("max_input_channels", 0) > 0]
        if input_devs:
            has_audio = True
            print(f"  {check_mark(True)} Default input: {input_devs[0].get('name')}")
        else:
            print(f"  {check_mark(False)} No audio input devices detected.")
            failures += 1
    except Exception as e:
        print(f"  {check_mark(False)} sounddevice error: {e}")
        failures += 1

    # 2. Audio Feedback Cue
    print("\n2. Audio Feedback Utility:")
    pw_play = shutil.which("pw-play")
    if pw_play:
        print(f"  {check_mark(True)} PipeWire player found: {pw_play}")
    else:
        print(f"  {check_mark(False)} pw-play not found (will use synthesized fallback)")

    # 3. Desktop Notifications
    print("\n3. Desktop Notification Utility:")
    notify_send = shutil.which("notify-send")
    if notify_send:
        print(f"  {check_mark(True)} notify-send found: {notify_send}")
    else:
        print(f"  {check_mark(False)} notify-send missing (install libnotify-bin or libnotify)")
        failures += 1

    # 4. Wayland Clipboard
    print("\n4. Wayland Clipboard Utility:")
    wl_copy = shutil.which("wl-copy")
    if wl_copy:
        print(f"  {check_mark(True)} wl-copy found: {wl_copy}")
    else:
        print(f"  {check_mark(False)} wl-copy missing (install wl-clipboard)")
        failures += 1

    # 5. /dev/uinput permissions
    print("\n5. Linux Keystroke Injection (/dev/uinput):")
    uinput_ok, uinput_msg = check_uinput_accessible()
    if uinput_ok:
        print(f"  {check_mark(True)} /dev/uinput is readable and writable.")
    else:
        print(f"  {check_mark(False)} {uinput_msg}")
        print("     Tip: Add user to 'input' group: sudo usermod -aG input $USER")
        failures += 1

    # 6. Systemd User Service
    print("\n6. Systemd User Autostart Service:")
    svc = get_service_status()
    if svc["installed"]:
        status_line = f"Installed ({svc['path']}) | Enabled: {svc['enabled']} | Active: {svc['active']}"
        print(f"  {check_mark(True)} {status_line}")
    else:
        print("  ℹ️ [OPTIONAL] Service unit not installed.")
        print("     Tip: Run 'voicekey autostart --install' to enable login autostart.")


    # 7. GNOME Keybinding
    print("\n7. GNOME Global Shortcut:")
    if shutil.which("gsettings"):
        res = subprocess.run(
            ["gsettings", "get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"],
            capture_output=True,
            text=True,
            check=False,
        )
        if "voicekey" in res.stdout:
            print(f"  {check_mark(True)} VoiceKey custom keybinding found in GNOME settings.")
        else:
            print(f"  {check_mark(False)} No VoiceKey keybinding detected in GNOME settings.")
            print("     Tip: Run './scripts/setup_gnome_hotkey.sh' to bind Super+V.")
    else:
        print("  ℹ️ Non-GNOME desktop environment; configure window manager shortcut manually.")

    # 8. Daemon Process & IPC Socket
    print("\n8. Daemon Process & IPC Socket:")
    sock_path = config.resolved_socket_path()
    if is_socket_alive(sock_path):
        resp = send_daemon_command("status", socket_path=sock_path, timeout=2.0)
        print(f"  {check_mark(True)} Daemon alive on {sock_path}")
        print(f"     State: {resp.get('state')} | Backend: {resp.get('backend')} | Model: {resp.get('model')}")
    else:
        print(f"  ℹ️ Daemon not running on {sock_path}. Start with 'voicekeyd' or 'voicekey autostart --start'.")

    # 9. Hardware Doctor Summary
    print("\n9. Hardware & Backend Capabilities:")
    profile = probe_hardware()
    gpu_desc = f"{profile.gpus[0].name} ({profile.gpus[0].vram_total_mb} MB)" if profile.gpus else "None detected"
    print(f"  CPU:       {profile.cpu_model} ({profile.cpu_cores} logical cores)")
    print(f"  RAM:       {profile.total_ram_gb:.1f} GB")
    print(f"  GPU:       {gpu_desc}")
    print(f"  Config:    Backend='{config.transcription.backend}', Model='{config.transcription.model_size}'")




    print("\n" + "=" * 65)
    if failures == 0:
        print("🎉 All core requirements verified! VoiceKey is ready for dictation.")
        return 0
    else:
        print(f"⚠️ Verification finished with {failures} warning(s)/missing requirement(s).")
        return 1


if __name__ == "__main__":
    sys.exit(main())
