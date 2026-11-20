"""VoiceKey Daemon Entrypoint (`voicekeyd`)."""

from __future__ import annotations

import argparse
import logging
import sys
from pathlib import Path

from voicekey import __version__
from voicekey.config import (
    VoiceKeyConfig,
    get_default_config_path,
    load_config,
)


def create_parser() -> argparse.ArgumentParser:
    """Construct command-line argument parser for daemon."""
    parser = argparse.ArgumentParser(
        prog="voicekeyd",
        description="Persistent background daemon for VoiceKey speech-to-text dictation.",
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
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging output",
    )
    parser.add_argument(
        "--socket",
        type=Path,
        default=None,
        help="Override daemon Unix domain socket path",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Daemon main entry point."""
    parser = create_parser()
    args = parser.parse_args(argv)

    log_level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(
        level=log_level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )
    logger = logging.getLogger("voicekeyd")

    config = load_config(args.config, create_if_missing=True)
    if args.socket:
        config.daemon.socket_path = str(args.socket)

    socket_path = config.resolved_socket_path()
    logger.info("Starting VoiceKey Daemon v%s", __version__)
    logger.info("Configured model: %s (%s, %s)", config.transcription.model_size, config.transcription.device, config.transcription.compute_type)
    logger.info("IPC Socket target: %s", socket_path)

    # Note: Full persistent async server, audio recorder, and warm transcriber
    # are wired in Phase 2, 3, and 4.
    return 0


if __name__ == "__main__":
    sys.exit(main())
