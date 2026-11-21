"""Integration tests for VoiceKeyDaemon state machine and IPC dispatch."""

import asyncio
import json
from pathlib import Path
import tempfile
from unittest.mock import MagicMock
import uuid
import numpy as np
import pytest

from voicekey.audio import AudioRecorder
from voicekey.config import DaemonConfig, VoiceKeyConfig
from voicekey.daemon import DaemonState, VoiceKeyDaemon
from voicekey.feedback import AudioFeedback
from voicekey.ipc import is_socket_alive, send_daemon_command
from voicekey.transcriber.base import BaseTranscriber


@pytest.fixture
def short_sock():
    """Short socket path in /tmp under Linux AF_UNIX 108 char limit."""
    p = Path(tempfile.gettempdir()) / f"vk_d_{uuid.uuid4().hex[:8]}.sock"
    yield p
    if p.exists():
        p.unlink(missing_ok=True)


@pytest.fixture
def mock_daemon(short_sock: Path):
    """Create a configured VoiceKeyDaemon instance with mocked hardware components."""
    config = VoiceKeyConfig(daemon=DaemonConfig(socket_path=str(short_sock)))

    mock_rec = MagicMock(spec=AudioRecorder)
    mock_rec.is_recording = False
    mock_rec.stop.return_value = np.zeros(16000, dtype=np.float32)

    mock_trans = MagicMock(spec=BaseTranscriber)
    mock_trans.is_loaded = True
    mock_trans.transcribe.return_value = "Hello world from daemon"

    mock_fb = MagicMock(spec=AudioFeedback)

    from voicekey.paste import PasteInjector
    mock_inj = MagicMock(spec=PasteInjector)
    mock_inj.is_available = True
    mock_inj.inject_ctrl_v.return_value = True

    daemon = VoiceKeyDaemon(
        config=config,
        recorder=mock_rec,
        transcriber=mock_trans,
        feedback=mock_fb,
        injector=mock_inj,
    )
    return daemon


@pytest.mark.anyio
async def test_daemon_state_transitions(mock_daemon: VoiceKeyDaemon):
    """Test state machine transitions between IDLE, RECORDING, and TRANSCRIBING."""
    assert mock_daemon.state == DaemonState.IDLE

    # 1. First toggle -> RECORDING
    resp1 = await mock_daemon.handle_toggle()
    assert resp1["status"] == "ok"
    assert resp1["state"] == "recording"
    assert mock_daemon.state == DaemonState.RECORDING
    mock_daemon.recorder.start.assert_called_once()

    # 2. Second toggle -> IDLE with transcription
    resp2 = await mock_daemon.handle_toggle()
    assert resp2["status"] == "ok"
    assert resp2["state"] == "idle"
    assert resp2["text"] == "Hello world from daemon"
    assert mock_daemon.state == DaemonState.IDLE
    mock_daemon.recorder.stop.assert_called_once()
    mock_daemon.transcriber.transcribe.assert_called_once()


@pytest.mark.anyio
async def test_daemon_start_stop_explicit(mock_daemon: VoiceKeyDaemon):
    """Explicit handle_start and handle_stop validate state constraints."""
    # Stop when idle produces error
    err_stop = await mock_daemon.handle_stop()
    assert err_stop["status"] == "error"
    assert "Cannot stop recording" in err_stop["message"]

    # Start when idle succeeds
    ok_start = await mock_daemon.handle_start()
    assert ok_start["status"] == "ok"
    assert mock_daemon.state == DaemonState.RECORDING

    # Start when already recording produces error
    err_start = await mock_daemon.handle_start()
    assert err_start["status"] == "error"
    assert "Cannot start recording" in err_start["message"]

    # Stop when recording succeeds
    ok_stop = await mock_daemon.handle_stop()
    assert ok_stop["status"] == "ok"
    assert ok_stop["text"] == "Hello world from daemon"
    assert mock_daemon.state == DaemonState.IDLE


@pytest.mark.anyio
async def test_daemon_live_ipc_commands(mock_daemon: VoiceKeyDaemon, short_sock: Path):
    """Test full live IPC socket server dispatching commands to client."""
    from voicekey.ipc import create_ipc_server

    server = await create_ipc_server(short_sock, mock_daemon._client_connected_cb)
    mock_daemon._server = server

    assert is_socket_alive(short_sock)
    loop = asyncio.get_running_loop()

    # Ping
    ping_resp = await loop.run_in_executor(
        None,
        lambda: send_daemon_command("ping", socket_path=short_sock, timeout=2.0)
    )
    assert ping_resp == {"status": "ok", "pong": True}

    # Status
    status_resp = await loop.run_in_executor(
        None,
        lambda: send_daemon_command("status", socket_path=short_sock, timeout=2.0)
    )
    assert status_resp["status"] == "ok"
    assert status_resp["state"] == "idle"
    assert status_resp["backend"] == "faster-whisper"

    # Toggle to start recording
    toggle1 = await loop.run_in_executor(
        None,
        lambda: send_daemon_command("toggle", socket_path=short_sock, timeout=2.0)
    )
    assert toggle1["status"] == "ok"
    assert toggle1["state"] == "recording"

    mock_daemon.config.output.paste_delay_ms = 0

    # Toggle to stop and transcribe
    from unittest.mock import patch
    with patch("voicekey.daemon.dispatch_paste", return_value=(True, "pasted")):
        toggle2 = await loop.run_in_executor(
            None,
            lambda: send_daemon_command("toggle", socket_path=short_sock, timeout=5.0)
        )
    assert toggle2["status"] == "ok"
    assert toggle2["state"] == "idle"
    assert toggle2["text"] == "Hello world from daemon"
    assert toggle2["action"] == "pasted"

    # Bare string command payload (Rust CLI compatibility)
    reader, writer = await asyncio.open_unix_connection(str(short_sock))
    writer.write(b'"ping"\n')
    await writer.drain()
    resp_line = await reader.readline()
    writer.close()
    await writer.wait_closed()
    assert json.loads(resp_line.decode("utf-8")) == {"status": "ok", "pong": True}

    await mock_daemon._cleanup()
    assert not short_sock.exists()
