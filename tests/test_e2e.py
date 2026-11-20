"""End-to-End integration tests for VoiceKey speech-to-text pipeline.

Verifies complete execution:
IPC client (toggle/start/stop) -> Daemon state machine -> Audio recording ->
Transcription -> Text Normalization -> Wayland Clipboard copy & uinput paste.
"""

from __future__ import annotations

import asyncio
import os
import tempfile
import threading
import time
from pathlib import Path
from unittest.mock import MagicMock, patch

import numpy as np
import pytest

from voicekey.config import VoiceKeyConfig
from voicekey.daemon import DaemonState, VoiceKeyDaemon
from voicekey.ipc import send_daemon_command



class MockTranscriber:
    """Deterministic mock transcriber for end-to-end testing."""

    def __init__(self, output_text: str = "hello world comma test dictation"):
        self.output_text = output_text
        self.call_count = 0
        self.is_loaded = True

    def load(self) -> None:
        self.is_loaded = True

    def unload(self) -> None:
        self.is_loaded = False

    def transcribe(self, audio_data: np.ndarray) -> str:
        self.call_count += 1
        return self.output_text

    def is_available(self) -> bool:
        return True


import uuid

@pytest.fixture
def e2e_env():
    """Setup isolated test environment with short socket path."""
    sock_path = Path(tempfile.gettempdir()) / f"vk_e2e_{uuid.uuid4().hex[:8]}.sock"
    if sock_path.exists():
        sock_path.unlink(missing_ok=True)

    conf = VoiceKeyConfig()
    conf.daemon.log_level = "DEBUG"
    conf.daemon.socket_path = str(sock_path)
    conf.transcription.backend = "faster-whisper"
    conf.transcription.model = "tiny"
    conf.transcription.spoken_punctuation = True
    conf.transcription.normalize_text = True
    conf.output.mode = "paste"
    conf.output.paste_delay_ms = 1


    yield sock_path, conf

    if sock_path.exists():
        try:
            sock_path.unlink()
        except OSError:
            pass


def test_e2e_toggle_pipeline(e2e_env):
    """Test full hotkey toggle -> record -> transcribe -> copy -> paste flow."""
    sock_path, config = e2e_env

    # Setup mock audio recorder returning 1 sec of audio
    mock_audio_rec = MagicMock()
    mock_audio_rec.is_recording = False
    fake_audio = np.full(16000, 0.05, dtype=np.float32)
    mock_audio_rec.stop.return_value = fake_audio

    def fake_start(**kwargs):
        mock_audio_rec.is_recording = True

    mock_audio_rec.start.side_effect = fake_start

    mock_rec_cls = MagicMock()
    mock_rec_cls.return_value = mock_audio_rec
    mock_rec_cls.calculate_rms.return_value = 0.05

    mock_transcriber = MockTranscriber("hello world comma testing voice key new line works")

    with patch("voicekey.daemon.AudioRecorder", mock_rec_cls), \
         patch("voicekey.daemon.AudioFeedback") as mock_fb, \
         patch("voicekey.daemon.dispatch_paste") as mock_paste, \
         patch("voicekey.daemon.send_notification") as mock_notify:

        mock_paste.return_value = (True, "pasted")


        daemon = VoiceKeyDaemon(config=config)
        daemon.transcriber = mock_transcriber

        daemon_thread = threading.Thread(target=lambda: asyncio.run(daemon.start()), daemon=True)
        daemon_thread.start()


        # Wait for daemon socket to be ready
        for _ in range(50):
            if sock_path.exists():
                break
            time.sleep(0.05)
        assert sock_path.exists(), "Daemon socket failed to bind"

        # 1. Query initial status
        st = send_daemon_command("status", socket_path=sock_path)
        assert st["status"] == "ok"
        assert st["state"] == "idle"

        # 2. First toggle: Start recording
        res1 = send_daemon_command("toggle", socket_path=sock_path)
        assert res1["status"] == "ok"
        assert res1["state"] == "recording"
        assert mock_audio_rec.is_recording is True

        # Check status during recording
        st_rec = send_daemon_command("status", socket_path=sock_path)
        assert st_rec["state"] == "recording"

        # Simulate small recording duration
        time.sleep(0.1)

        # 3. Second toggle: Stop recording & transcribe
        res2 = send_daemon_command("toggle", socket_path=sock_path)
        assert res2["status"] == "ok"
        assert res2["action"] == "pasted"
        # Spoken punctuation should be converted: "comma" -> ",", "new line" -> "\n"
        assert "Hello world," in res2["text"]
        assert "\nWorks" in res2["text"]

        # 4. Verify post-conditions
        assert mock_transcriber.call_count == 1
        assert mock_paste.called
        paste_text = mock_paste.call_args[1]["text"]
        assert "Hello world," in paste_text


        # 5. Check daemon returned to idle
        st_final = send_daemon_command("status", socket_path=sock_path)
        assert st_final["state"] == "idle"

        # 6. Stop daemon cleanly
        daemon.stop()
        daemon_thread.join(timeout=3.0)


def test_e2e_explicit_start_stop(e2e_env):
    """Test explicit start -> stop sequence with custom hotwords."""
    sock_path, config = e2e_env
    config.transcription.hotwords = ["Kubernetes", "gRPC"]

    mock_audio_rec = MagicMock()
    mock_audio_rec.is_recording = False
    fake_audio = np.full(16000, 0.05, dtype=np.float32)
    mock_audio_rec.stop.return_value = fake_audio

    def fake_start(**kwargs):
        mock_audio_rec.is_recording = True

    mock_audio_rec.start.side_effect = fake_start

    mock_rec_cls = MagicMock()
    mock_rec_cls.return_value = mock_audio_rec
    mock_rec_cls.calculate_rms.return_value = 0.05

    mock_transcriber = MockTranscriber("dictation using Kubernetes and gRPC")

    with patch("voicekey.daemon.AudioRecorder", mock_rec_cls), \
         patch("voicekey.daemon.AudioFeedback"), \
         patch("voicekey.daemon.dispatch_paste") as mock_paste, \
         patch("voicekey.daemon.send_notification"):

        mock_paste.return_value = (True, "pasted")


        daemon = VoiceKeyDaemon(config=config)
        daemon.transcriber = mock_transcriber

        daemon_thread = threading.Thread(target=lambda: asyncio.run(daemon.start()), daemon=True)
        daemon_thread.start()


        for _ in range(50):
            if sock_path.exists():
                break
            time.sleep(0.05)

        # Explicit start
        res_start = send_daemon_command("start", socket_path=sock_path)
        assert res_start["status"] == "ok"

        # Explicit stop
        res_stop = send_daemon_command("stop", socket_path=sock_path)
        assert res_stop["status"] == "ok"
        assert "Kubernetes and gRPC" in res_stop["text"]

        daemon.stop()
        daemon_thread.join(timeout=3.0)
