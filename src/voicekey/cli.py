"""VoiceKey CLI Client - Fast command dispatcher for VoiceKey daemon."""

from __future__ import annotations

import argparse
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

    if args.command in {"toggle", "start", "stop", "status"}:
        socket_path = config.resolved_socket_path()
        if not socket_path.exists():
            print(
                f"[voicekey] Daemon socket not found at '{socket_path}'.\n"
                f"Please start the daemon with: voicekeyd",
                file=sys.stderr,
            )
            return 1

        # Phase 4 will implement the full IPC client socket exchange.
        print(f"[voicekey] Command '{args.command}' sent to {socket_path}")
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
