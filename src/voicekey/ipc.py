"""Unix Domain Socket IPC protocol and client utilities for VoiceKey."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import socket
from pathlib import Path
from typing import Any, Callable, Coroutine

logger = logging.getLogger("voicekey.ipc")


class IPCError(Exception):
    """Base exception for IPC communication errors."""
    pass


class DaemonNotRunningError(IPCError):
    """Raised when the VoiceKey daemon is not accessible."""
    pass


def is_socket_alive(socket_path: Path) -> bool:
    """Test if a Unix domain socket has an active listening process."""
    if not socket_path.exists():
        return False

    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(0.5)
    try:
        client.connect(str(socket_path))
        return True
    except (ConnectionRefusedError, socket.timeout, OSError):
        return False
    finally:
        client.close()


def clean_stale_socket(socket_path: Path) -> bool:
    """Remove socket file if it exists but no daemon is listening."""
    if not socket_path.exists():
        return False

    if not is_socket_alive(socket_path):
        try:
            socket_path.unlink()
            logger.debug("Removed stale socket file at %s", socket_path)
            return True
        except OSError as e:
            logger.warning("Failed to remove stale socket at %s: %s", socket_path, e)
    return False


def send_daemon_command(
    command: str,
    args: dict[str, Any] | None = None,
    socket_path: Path | None = None,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Synchronously send a command to the daemon Unix socket and return response dictionary.

    Args:
        command: Command name (e.g. 'toggle', 'start', 'stop', 'status', 'ping')
        args: Optional command arguments dictionary
        socket_path: Path to Unix domain socket
        timeout: Read/write timeout in seconds

    Returns:
        Response dictionary from daemon
    """
    if socket_path is None:
        from voicekey.config import get_default_socket_path
        socket_path = get_default_socket_path()

    resolved_path = socket_path.expanduser().resolve()
    if not resolved_path.exists():
        raise DaemonNotRunningError(f"VoiceKey daemon socket not found at '{resolved_path}'.")

    payload = json.dumps({"command": command, "args": args or {}}) + "\n"
    payload_bytes = payload.encode("utf-8")

    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(timeout)

    try:
        client.connect(str(resolved_path))
        client.sendall(payload_bytes)

        # Read newline-delimited response
        buffer = bytearray()
        while True:
            chunk = client.recv(4096)
            if not chunk:
                break
            buffer.extend(chunk)
            if b"\n" in chunk:
                break

        if not buffer:
            raise IPCError("Daemon closed connection without sending a response.")

        line = buffer.decode("utf-8").strip()
        response = json.loads(line)
        if not isinstance(response, dict):
            raise IPCError(f"Invalid daemon response format: expected dict, got {type(response)}")

        return response
    except (ConnectionRefusedError, FileNotFoundError) as e:
        clean_stale_socket(resolved_path)
        raise DaemonNotRunningError(f"Could not connect to VoiceKey daemon at '{resolved_path}': {e}") from e
    except socket.timeout as e:
        raise IPCError(f"Timed out after {timeout:.1f}s waiting for daemon response.") from e
    except json.JSONDecodeError as e:
        raise IPCError(f"Failed to decode daemon JSON response: {e}") from e
    finally:
        client.close()


async def create_ipc_server(
    socket_path: Path,
    handler: Callable[[asyncio.StreamReader, asyncio.StreamWriter], Coroutine[Any, Any, None]],
) -> asyncio.AbstractServer:
    """Create and start an asynchronous Unix domain socket server, cleaning stale sockets if present."""
    resolved_path = socket_path.expanduser().resolve()
    resolved_path.parent.mkdir(parents=True, exist_ok=True)

    clean_stale_socket(resolved_path)
    if resolved_path.exists():
        raise RuntimeError(f"Socket '{resolved_path}' already in use by another active daemon.")

    server = await asyncio.start_unix_server(handler, path=str(resolved_path))
    logger.info("Unix domain socket server listening at %s", resolved_path)
    return server
