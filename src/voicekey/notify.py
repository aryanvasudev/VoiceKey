"""Desktop notification dispatcher using notify-send."""

from __future__ import annotations

import logging
import shutil
import subprocess

logger = logging.getLogger("voicekey.notify")


def send_notification(
    summary: str,
    body: str = "",
    urgency: str = "low",
    timeout_ms: int = 2500,
    app_name: str = "VoiceKey",
    synchronous_tag: str | None = "voicekey-dictation",
) -> bool:
    """Dispatch a desktop notification asynchronously via notify-send.

    Returns True if notify-send command was successfully launched, False otherwise.
    """
    notify_send_bin = shutil.which("notify-send")
    if not notify_send_bin:
        logger.debug("notify-send not found on system; skipping notification.")
        return False

    cmd = [
        notify_send_bin,
        "-a", app_name,
        "-u", urgency,
        "-t", str(timeout_ms),
        "-i", "input-keyboard",
    ]

    if synchronous_tag:
        cmd.extend(["-h", f"string:x-canonical-private-synchronous:{synchronous_tag}"])

    cmd.append(summary)
    if body:
        cmd.append(body)

    try:
        subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return True
    except Exception as e:
        logger.warning("Failed to trigger notify-send: %s", e)
        return False
