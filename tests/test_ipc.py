"""Tests for Unix Domain Socket IPC layer."""

import asyncio
import json
from pathlib import Path
import pytest

from voicekey.ipc import (
    DaemonNotRunningError,
    IPCError,
    clean_stale_socket,
    create_ipc_server,
    is_socket_alive,
    send_daemon_command,
)


def test_is_socket_alive_nonexistent(tmp_path: Path):
    """Non-existent socket returns False."""
    sock = tmp_path / "missing.sock"
    assert not is_socket_alive(sock)


def test_clean_stale_socket(tmp_path: Path):
    """Stale socket with no listener is deleted."""
    sock = tmp_path / "stale.sock"
    sock.touch()
    assert sock.exists()

    removed = clean_stale_socket(sock)
    assert removed is True
    assert not sock.exists()


def test_send_daemon_command_missing_socket(tmp_path: Path):
    """Attempting to communicate with non-existent socket raises DaemonNotRunningError."""
    sock = tmp_path / "absent.sock"
    with pytest.raises(DaemonNotRunningError, match="socket not found"):
        send_daemon_command("status", socket_path=sock)


import tempfile
import uuid


@pytest.fixture
def short_sock_path():
    """Generate a short socket path in /tmp to stay under 108 char Linux limit."""
    path = Path(tempfile.gettempdir()) / f"vk_{uuid.uuid4().hex[:8]}.sock"
    yield path
    if path.exists():
        path.unlink(missing_ok=True)


@pytest.mark.anyio
async def test_ipc_roundtrip(short_sock_path: Path):
    """Verify live client-server communication over Unix domain socket."""
    sock_path = short_sock_path

    async def echo_handler(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        data = await reader.readline()
        if not data:
            writer.close()
            await writer.wait_closed()
            return

        req = json.loads(data.decode("utf-8").strip())
        resp = {"status": "ok", "echo_command": req.get("command"), "args": req.get("args")}
        writer.write((json.dumps(resp) + "\n").encode("utf-8"))
        await writer.drain()
        writer.close()
        await writer.wait_closed()

    server = await create_ipc_server(sock_path, echo_handler)
    assert is_socket_alive(sock_path)

    # Run client call in thread/sync executor
    loop = asyncio.get_running_loop()
    resp = await loop.run_in_executor(
        None,
        lambda: send_daemon_command("toggle", args={"flag": True}, socket_path=sock_path, timeout=2.0)
    )

    assert resp["status"] == "ok"
    assert resp["echo_command"] == "toggle"
    assert resp["args"] == {"flag": True}

    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_create_ipc_server_active_conflict(short_sock_path: Path):
    """Starting server on already active socket raises RuntimeError."""
    sock_path = short_sock_path

    async def dummy(r, w):
        pass

    server1 = await create_ipc_server(sock_path, dummy)
    assert sock_path.exists()

    with pytest.raises(RuntimeError, match="already in use"):
        await create_ipc_server(sock_path, dummy)

    server1.close()
    await server1.wait_closed()
