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
