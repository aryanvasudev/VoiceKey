"""VoiceKey Persistent Background Daemon (`voicekeyd`)."""

from __future__ import annotations

import argparse
import asyncio
from concurrent.futures import ThreadPoolExecutor
from enum import Enum
import json
import logging
import signal
import sys
import time
from pathlib import Path
from typing import Any

from voicekey import __version__
from voicekey.audio import AudioRecorder
from voicekey.config import VoiceKeyConfig, get_default_config_path, load_config
from voicekey.feedback import AudioFeedback
from voicekey.ipc import create_ipc_server
from voicekey.notify import send_notification
from voicekey.transcriber import BaseTranscriber, create_transcriber

from voicekey.paste import PasteInjector, dispatch_paste
from voicekey.postprocess import process_transcription

logger = logging.getLogger("voicekeyd")



class DaemonState(str, Enum):
    """Daemon operating states."""
    IDLE = "idle"
    RECORDING = "recording"
    TRANSCRIBING = "transcribing"


class VoiceKeyDaemon:
    """Persistent background dictation daemon managing audio capture and warm transcription."""

    def __init__(
        self,
        config: VoiceKeyConfig,
        recorder: AudioRecorder | None = None,
        transcriber: BaseTranscriber | None = None,
        feedback: AudioFeedback | None = None,
        injector: PasteInjector | None = None,
    ) -> None:
        self.config = config
        self.feedback = feedback or AudioFeedback(enabled=config.audio.sound_feedback)
        self.recorder = recorder or AudioRecorder(config=config.audio, feedback=self.feedback)
        self.transcriber = transcriber  # May be pre-injected or loaded during start()
        self.injector = injector or PasteInjector()
        self.state = DaemonState.IDLE
        self.start_time = time.time()

        self._server: asyncio.AbstractServer | None = None
        self._executor = ThreadPoolExecutor(max_workers=2, thread_name_prefix="voicekey-worker")
        self._stop_event = asyncio.Event()
        self._lock = asyncio.Lock()

    def warm_up_transcriber(self) -> None:
        """Pre-warm and hold the transcription model in RAM."""
        if self.transcriber is None:
            logger.info("Initializing transcription engine: %s...", self.config.transcription.backend)
            self.transcriber = create_transcriber(self.config.transcription)

        if not self.transcriber.is_loaded:
            logger.info("Pre-warming model '%s' in RAM...", self.config.transcription.model_size)
            self.transcriber.initialize()
            logger.info("Model pre-warming complete. Engine ready!")

    async def handle_toggle(self) -> dict[str, Any]:
        """Toggle recording state: IDLE -> RECORDING, or RECORDING -> TRANSCRIBING -> IDLE."""
        async with self._lock:
            if self.state == DaemonState.TRANSCRIBING:
                return {
                    "status": "busy",
                    "state": self.state.value,
                    "message": "Transcription in progress, please wait.",
                }

            if self.state == DaemonState.IDLE:
                # Start recording
                self.recorder.start(play_cue=True)
                self.state = DaemonState.RECORDING
                if self.config.output.notify:
                    send_notification("VoiceKey", "Recording started...", timeout_ms=1500)
                return {
                    "status": "ok",
                    "state": DaemonState.RECORDING.value,
                    "message": "Recording started",
                }

            # State is RECORDING -> stop and transcribe
            self.state = DaemonState.TRANSCRIBING

        # Stop recording outside lock to ensure swift feedback
        audio_data = self.recorder.stop(play_cue=True)
        duration = len(audio_data) / self.config.audio.sample_rate if self.config.audio.sample_rate > 0 else 0.0
        rms = AudioRecorder.calculate_rms(audio_data)

        if self.config.output.notify:
            send_notification("VoiceKey", "Transcribing speech...", timeout_ms=1500)

        # Run transcription in threadpool to keep event loop responsive
        loop = asyncio.get_running_loop()
        text = ""
        paste_action = "none"
        if len(audio_data) > 0 and self.transcriber is not None:
            text = await loop.run_in_executor(self._executor, self.transcriber.transcribe, audio_data)
            if text:
                text = process_transcription(
                    text,
                    enable_normalization=self.config.transcription.normalize_text,
                    enable_spoken_punctuation=self.config.transcription.spoken_punctuation,
                )
                _ok, paste_action = await loop.run_in_executor(
                    self._executor,
                    lambda: dispatch_paste(
                        text=text,
                        mode=self.config.output.mode,
                        paste_delay_ms=self.config.output.paste_delay_ms,
                        injector=self.injector,
                    ),
                )


        async with self._lock:
            self.state = DaemonState.IDLE

        if self.config.output.notify and text:
            if paste_action == "pasted":
                send_notification("VoiceKey", f"Pasted: {text[:60]}...", timeout_ms=2000)
            else:
                send_notification("VoiceKey", f"Copied to clipboard: {text[:60]}...", timeout_ms=2500)

        logger.info("Transcription completed: '%s' (%.2fs, action=%s)", text, duration, paste_action)
        return {
            "status": "ok",
            "state": DaemonState.IDLE.value,
            "text": text,
            "duration": round(duration, 2),
            "rms": round(rms, 4),
            "action": paste_action,
        }

    async def handle_start(self) -> dict[str, Any]:
        """Explicitly start recording if idle."""
        async with self._lock:
            if self.state != DaemonState.IDLE:
                return {
                    "status": "error",
                    "state": self.state.value,
                    "message": f"Cannot start recording: daemon is currently in {self.state.value} state.",
                }

            self.recorder.start(play_cue=True)
            self.state = DaemonState.RECORDING
            if self.config.output.notify:
                send_notification("VoiceKey", "Recording started...", timeout_ms=1500)
            return {
                "status": "ok",
                "state": DaemonState.RECORDING.value,
                "message": "Recording started",
            }

    async def handle_stop(self) -> dict[str, Any]:
        """Explicitly stop recording and transcribe."""
        async with self._lock:
            if self.state != DaemonState.RECORDING:
                return {
                    "status": "error",
                    "state": self.state.value,
                    "message": f"Cannot stop recording: daemon is currently in {self.state.value} state.",
                }
            self.state = DaemonState.TRANSCRIBING

        audio_data = self.recorder.stop(play_cue=True)
        duration = len(audio_data) / self.config.audio.sample_rate if self.config.audio.sample_rate > 0 else 0.0
        rms = AudioRecorder.calculate_rms(audio_data)

        loop = asyncio.get_running_loop()
        text = ""
        paste_action = "none"
        if len(audio_data) > 0 and self.transcriber is not None:
            text = await loop.run_in_executor(self._executor, self.transcriber.transcribe, audio_data)
            if text:
                text = process_transcription(
                    text,
                    enable_normalization=self.config.transcription.normalize_text,
                    enable_spoken_punctuation=self.config.transcription.spoken_punctuation,
                )
                _ok, paste_action = await loop.run_in_executor(
                    self._executor,
                    lambda: dispatch_paste(
                        text=text,
                        mode=self.config.output.mode,
                        paste_delay_ms=self.config.output.paste_delay_ms,
                        injector=self.injector,
                    ),
                )


        async with self._lock:
            self.state = DaemonState.IDLE

        if self.config.output.notify and text:
            if paste_action == "pasted":
                send_notification("VoiceKey", f"Pasted: {text[:60]}...", timeout_ms=2000)
            else:
                send_notification("VoiceKey", f"Copied to clipboard: {text[:60]}...", timeout_ms=2500)

        return {
            "status": "ok",
            "state": DaemonState.IDLE.value,
            "text": text,
            "duration": round(duration, 2),
            "rms": round(rms, 4),
            "action": paste_action,
        }

    def handle_status(self) -> dict[str, Any]:
        """Return current status of daemon, engine, and recorder."""
        model_name = self.config.transcription.model_size
        backend = self.config.transcription.backend
        is_warm = self.transcriber is not None and self.transcriber.is_loaded

        return {
            "status": "ok",
            "state": self.state.value,
            "backend": backend,
            "model_size": model_name,
            "device": self.config.transcription.device,
            "model_warm": is_warm,
            "is_recording": self.recorder.is_recording,
            "paste_mode": self.config.output.mode,
            "uinput_ready": self.injector.is_available,
            "uptime_seconds": round(time.time() - self.start_time, 1),
            "version": __version__,
        }

    async def dispatch_command(self, command: str, args: dict[str, Any]) -> dict[str, Any]:
        """Route incoming IPC command to appropriate handler."""
        cmd = command.lower().strip()
        if cmd == "toggle":
            return await self.handle_toggle()
        elif cmd == "start":
            return await self.handle_start()
        elif cmd == "stop":
            return await self.handle_stop()
        elif cmd == "status":
            return self.handle_status()
        elif cmd == "ping":
            return {"status": "ok", "pong": True}
        else:
            return {"status": "error", "message": f"Unknown command: '{command}'"}

    async def _client_connected_cb(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        """Handle individual client Unix domain socket connection."""
        try:
            line = await reader.readline()
            if not line:
                writer.close()
                await writer.wait_closed()
                return

            req_str = line.decode("utf-8").strip()
            if not req_str:
                writer.close()
                await writer.wait_closed()
                return

            req = json.loads(req_str)
            cmd = req.get("command", "")
            args = req.get("args", {})

            resp = await self.dispatch_command(cmd, args)
            writer.write((json.dumps(resp) + "\n").encode("utf-8"))
            await writer.drain()
        except Exception as e:
            logger.exception("Error handling IPC client request: %s", e)
            try:
                err_resp = {"status": "error", "message": str(e)}
                writer.write((json.dumps(err_resp) + "\n").encode("utf-8"))
                await writer.drain()
            except Exception:
                pass
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

    async def start(self) -> None:
        """Start daemon server and background workers."""
        socket_path = self.config.resolved_socket_path()
        logger.info("Starting VoiceKey Daemon v%s on socket %s...", __version__, socket_path)

        # Pre-warm transcriber in executor so socket server starts immediately
        loop = asyncio.get_running_loop()
        loop.run_in_executor(self._executor, self.warm_up_transcriber)

        self._server = await create_ipc_server(socket_path, self._client_connected_cb)

        # Register termination signals
        for sig in (signal.SIGINT, signal.SIGTERM):
            try:
                loop.add_signal_handler(sig, lambda s=sig: self.stop())
            except (NotImplementedError, RuntimeError):
                pass

        logger.info("VoiceKey Daemon successfully initialized. Listening for commands.")
        try:
            await self._stop_event.wait()
        finally:
            await self._cleanup()

    def stop(self) -> None:
        """Signal daemon to stop."""
        logger.info("Stopping VoiceKey Daemon...")
        self._stop_event.set()

    async def _cleanup(self) -> None:
        """Clean up sockets and unload models."""
        if self._server is not None:
            self._server.close()
            await self._server.wait_closed()
            self._server = None

        socket_path = self.config.resolved_socket_path()
        if socket_path.exists():
            try:
                socket_path.unlink()
                logger.debug("Cleaned up socket file %s", socket_path)
            except Exception as e:
                logger.warning("Failed to remove socket file: %s", e)

        if self.transcriber is not None:
            self.transcriber.unload()

        if self.injector is not None:
            self.injector.close()

        self._executor.shutdown(wait=False)
        logger.info("VoiceKey Daemon stopped cleanly.")


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

    config = load_config(args.config, create_if_missing=True)
    if args.socket:
        config.daemon.socket_path = str(args.socket)

    daemon = VoiceKeyDaemon(config)
    try:
        asyncio.run(daemon.start())
        return 0
    except (KeyboardInterrupt, SystemExit):
        return 0
    except Exception as e:
        logger.error("Daemon crashed with unhandled error: %s", e, exc_info=args.debug)
        return 1


if __name__ == "__main__":
    sys.exit(main())
