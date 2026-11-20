"""VoiceKey CLI Client - Fast command dispatcher for VoiceKey daemon."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from voicekey import __version__
from voicekey.config import (
    get_default_config_path,
    load_config,
    save_config,
)


def create_parser() -> argparse.ArgumentParser:
    """Construct command-line argument parser."""
    parser = argparse.ArgumentParser(
        prog="voicekey",
        description="Fast hotkey-driven speech-to-text dictation client for Linux Wayland.",
    )
    parser.add_argument(
        "-v", "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    parser.add_argument(
        "-c", "--config",
        type=Path,
        default=None,
        help="Path to custom configuration file (default: ~/.config/voicekey/config.toml)",
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # `voicekey toggle`
    subparsers.add_parser("toggle", help="Toggle dictation recording state (start or stop & transcribe)")

    # `voicekey start`
    subparsers.add_parser("start", help="Explicitly start dictation recording")

    # `voicekey stop`
    subparsers.add_parser("stop", help="Explicitly stop dictation recording and transcribe")

    # `voicekey status`
    subparsers.add_parser("status", help="Query daemon status (idle, recording, offline)")

    # `voicekey config`
    config_parser = subparsers.add_parser("config", help="Manage VoiceKey configuration")
    config_group = config_parser.add_mutually_exclusive_group()
    config_group.add_argument(
        "--init",
        action="store_true",
        help="Generate default configuration file if not already present",
    )
    config_group.add_argument(
        "--path",
        action="store_true",
        help="Print the active configuration file path",
    )
    config_group.add_argument(
        "--show",
        action="store_true",
        help="Display the current active configuration in TOML format",
    )

    # `voicekey test-audio`
    audio_parser = subparsers.add_parser("test-audio", help="Record audio sample to verify microphone input and audio cues")
    audio_parser.add_argument(
        "--seconds",
        type=float,
        default=2.0,
        help="Recording duration in seconds (default: 2.0)",
    )

    # `voicekey doctor`
    doctor_parser = subparsers.add_parser("doctor", help="Inspect device hardware and recommend optimal transcription engine")
    doctor_parser.add_argument(
        "--apply",
        action="store_true",
        help="Automatically apply recommended engine and model settings to config.toml",
    )

    return parser


def handle_config_command(args: argparse.Namespace) -> int:
    """Handle `voicekey config` subcommands."""
    config_path = args.config or get_default_config_path()

    if args.path:
        print(config_path)
        return 0

    if args.init:
        if config_path.exists():
            print(f"Configuration already exists at: {config_path}")
            return 0
        cfg = load_config(config_path, create_if_missing=True)
        print(f"Initialized default configuration at: {config_path}")
        return 0

    if args.show or (not args.init and not args.path):
        cfg = load_config(config_path, create_if_missing=False)
        import tomli_w
        print(tomli_w.dumps(cfg.to_dict()).strip())
        return 0

    return 0


def handle_test_audio_command(args: argparse.Namespace, config) -> int:
    """Handle `voicekey test-audio` command."""
    import time
    from voicekey.audio import AudioRecorder
    from voicekey.notify import send_notification

    print(f"[voicekey] Testing audio recording for {args.seconds:.1f}s...")
    recorder = AudioRecorder(config=config.audio)
    recorder.start(play_cue=True)
    send_notification("VoiceKey", f"Testing audio recording ({args.seconds:.1f}s)...")

    try:
        time.sleep(args.seconds)
    finally:
        audio_data = recorder.stop(play_cue=True)

    rms = AudioRecorder.calculate_rms(audio_data)
    peak = AudioRecorder.calculate_peak(audio_data)
    duration = len(audio_data) / config.audio.sample_rate if config.audio.sample_rate > 0 else 0.0

    print(f"[voicekey] Recording complete!")
    print(f"  Duration:  {duration:.2f}s ({len(audio_data)} samples at {config.audio.sample_rate}Hz)")
    print(f"  RMS Level: {rms:.4f}")
    print(f"  Peak Level:{peak:.4f}")

    if rms > 0.005:
        print("  Status:    Audio input detected successfully! 🎙️")
        send_notification("VoiceKey", f"Audio test passed (RMS: {rms:.4f})")
    else:
        print("  Status:    Very low audio level or silence detected. Check microphone volume.")
        send_notification("VoiceKey", "Audio test completed (silent or low input)")

    return 0


def handle_doctor_command(args: argparse.Namespace, config) -> int:
    """Handle `voicekey doctor` command."""
    from voicekey.doctor import probe_hardware, recommend_setup, format_doctor_report
    from voicekey.config import save_config, get_default_config_path

    profile = probe_hardware()
    rec = recommend_setup(profile)
    print(format_doctor_report(profile, rec))

    if args.apply:
        config.transcription.backend = rec.backend
        config.transcription.model_size = rec.model_size
        config.transcription.device = rec.device
        config.transcription.compute_type = rec.compute_type
        config.transcription.cpu_threads = rec.cpu_threads
        config_path = args.config or get_default_config_path()
        save_config(config, config_path)
        print(f"\n[voicekey] Successfully applied recommended settings to: {config_path}")

    return 0


def main(argv: list[str] | None = None) -> int:
    """CLI main entry point."""
    parser = create_parser()
    args = parser.parse_args(argv)

    if not args.command:
        parser.print_help()
        return 0

    config = load_config(args.config, create_if_missing=False)

    if args.command == "config":
        return handle_config_command(args)

    if args.command == "test-audio":
        return handle_test_audio_command(args, config)

    if args.command == "doctor":
        return handle_doctor_command(args, config)

def handle_daemon_command(command: str, socket_path: Path) -> int:
    """Send command to daemon via Unix domain socket and print formatted response."""
    from voicekey.ipc import DaemonNotRunningError, IPCError, send_daemon_command

    try:
        resp = send_daemon_command(command, socket_path=socket_path)
    except DaemonNotRunningError:
        print(
            f"[voicekey] Daemon is not running (socket '{socket_path}' not accessible).\n"
            f"Please start the daemon with: voicekeyd",
            file=sys.stderr,
        )
        return 1
    except IPCError as e:
        print(f"[voicekey] Communication error: {e}", file=sys.stderr)
        return 1

    if command == "status":
        state = resp.get("state", "unknown").upper()
        backend = resp.get("backend", "unknown")
        model = resp.get("model_size", "unknown")
        device = resp.get("device", "cpu")
        warm = "Yes" if resp.get("model_warm") else "No"
        uptime = resp.get("uptime_seconds", 0)
        ver = resp.get("version", __version__)
        print("VoiceKey Daemon Status:")
        print(f"  State:        {state}")
        print(f"  Backend:      {backend}")
        print(f"  Model:        {model} ({device})")
        print(f"  Model Warm:   {warm}")
        print(f"  Uptime:       {uptime:.1f}s")
        print(f"  Version:      {ver}")
        return 0

    if command == "toggle":
        state = resp.get("state", "")
        if state == "recording":
            print("[voicekey] 🔴 Recording started...")
            return 0
        elif state == "idle":
            dur = resp.get("duration", 0)
            text = resp.get("text", "")
            print(f"[voicekey] ⏹️ Recording stopped ({dur:.1f}s)")
            if text:
                print(f"[voicekey] Transcribed: \"{text}\"")
            return 0
        elif resp.get("status") == "busy":
            print(f"[voicekey] ⏳ {resp.get('message', 'Busy')}")
            return 0

    if command == "start":
        if resp.get("status") == "ok":
            print("[voicekey] 🔴 Recording started")
            return 0
        else:
            print(f"[voicekey] {resp.get('message', 'Could not start')}", file=sys.stderr)
            return 1

    if command == "stop":
        if resp.get("status") == "ok":
            text = resp.get("text", "")
            dur = resp.get("duration", 0)
            print(f"[voicekey] ⏹️ Stopped ({dur:.1f}s)")
            if text:
                print(f"[voicekey] Transcribed: \"{text}\"")
            return 0
        else:
            print(f"[voicekey] {resp.get('message', 'Could not stop')}", file=sys.stderr)
            return 1

    print(json.dumps(resp, indent=2))
    return 0


def main(argv: list[str] | None = None) -> int:
    """CLI main entry point."""
    parser = create_parser()
    args = parser.parse_args(argv)

    if not args.command:
        parser.print_help()
        return 0

    config = load_config(args.config, create_if_missing=False)

    if args.command == "config":
        return handle_config_command(args)

    if args.command == "test-audio":
        return handle_test_audio_command(args, config)

    if args.command == "doctor":
        return handle_doctor_command(args, config)

    if args.command in {"toggle", "start", "stop", "status"}:
        socket_path = config.resolved_socket_path()
        return handle_daemon_command(args.command, socket_path)

    return 0


if __name__ == "__main__":
    sys.exit(main())
