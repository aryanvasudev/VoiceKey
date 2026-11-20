"""Wayland clipboard management and /dev/uinput virtual keystroke injection."""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Tuple

logger = logging.getLogger("voicekey.paste")


def check_uinput_accessible() -> tuple[bool, str]:
    """Check if /dev/uinput exists and current user has write access."""
    uinput_path = Path("/dev/uinput")
    if not uinput_path.exists():
        return False, "/dev/uinput device does not exist."

    if not os.access(uinput_path, os.W_OK):
        msg = (
            "Write permission denied for /dev/uinput.\n"
            "To grant user access without sudo:\n"
            "  1. Run: sudo usermod -aG input $USER\n"
            '  2. Run: echo \'KERNEL=="uinput", GROUP="input", MODE="0660"\' | sudo tee /etc/udev/rules.d/99-uinput.rules\n'
            "  3. Log out and back in."
        )
        return False, msg

    return True, "/dev/uinput is accessible."


def copy_to_clipboard(text: str) -> bool:
    """Copy text to system clipboard using wl-copy (Wayland) or xclip (X11)."""
    if not text:
        return True

    text_bytes = text.encode("utf-8")

    # Try Wayland wl-copy first
    wl_copy = shutil.which("wl-copy")
    if wl_copy:
        try:
            res = subprocess.run(
                [wl_copy],
                input=text_bytes,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                timeout=2.0,
            )
            if res.returncode == 0:
                logger.debug("Successfully copied text via wl-copy.")
                return True
            logger.warning("wl-copy returned exit code %d: %s", res.returncode, res.stderr.decode("utf-8", "ignore"))
        except Exception as e:
            logger.warning("Error running wl-copy: %s", e)

    # Fallback to xclip (X11 or XWayland)
    xclip = shutil.which("xclip")
    if xclip:
        try:
            res = subprocess.run(
                [xclip, "-selection", "clipboard"],
                input=text_bytes,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                timeout=2.0,
            )
            if res.returncode == 0:
                logger.debug("Successfully copied text via xclip.")
                return True
        except Exception as e:
            logger.warning("Error running xclip: %s", e)

    logger.error("No working clipboard utility found (neither wl-copy nor xclip succeeded).")
    return False


class PasteInjector:
    """Virtual input device manager to simulate Ctrl+V keystroke via /dev/uinput."""

    def __init__(self, device_name: str = "voicekey-virtual-keyboard") -> None:
        self.device_name = device_name
        self._uinput = None
        self._accessible, self._access_msg = check_uinput_accessible()

    @property
    def is_available(self) -> bool:
        """Check if uinput is ready for key injection."""
        return self._accessible

    def _ensure_device(self):
        """Create or return existing evdev.UInput device."""
        if self._uinput is not None:
            return self._uinput

        if not self._accessible:
            raise PermissionError(self._access_msg)

        try:
            import evdev
            from evdev import UInput, ecodes

            cap = {
                ecodes.EV_KEY: [
                    ecodes.KEY_LEFTCTRL,
                    ecodes.KEY_V,
                    ecodes.KEY_LEFTSHIFT,
                    ecodes.KEY_INSERT,
                ]
            }
            self._uinput = UInput(cap, name=self.device_name)
            # Short sleep to allow the kernel / display server to register the new device
            time.sleep(0.05)
            logger.debug("Created virtual input device: %s", self.device_name)
            return self._uinput
        except Exception as e:
            self._accessible = False
            self._access_msg = f"Failed to initialize UInput device: {e}"
            logger.error("UInput initialization error: %s", e)
            raise

    def inject_ctrl_v(self, debounce_ms: int = 20) -> bool:
        """Simulate Ctrl+V press and release via uinput."""
        try:
            ui = self._ensure_device()
            from evdev import ecodes

            # Press Ctrl + V
            ui.write(ecodes.EV_KEY, ecodes.KEY_LEFTCTRL, 1)
            ui.write(ecodes.EV_KEY, ecodes.KEY_V, 1)
            ui.syn()

            time.sleep(debounce_ms / 1000.0)

            # Release V + Ctrl
            ui.write(ecodes.EV_KEY, ecodes.KEY_V, 0)
            ui.write(ecodes.EV_KEY, ecodes.KEY_LEFTCTRL, 0)
            ui.syn()

            logger.debug("Emitted virtual Ctrl+V key injection.")
            return True
        except Exception as e:
            logger.error("Failed to inject virtual Ctrl+V: %s", e)
            return False

    def close(self) -> None:
        """Close virtual device."""
        if self._uinput is not None:
            try:
                self._uinput.close()
            except Exception:
                pass
            self._uinput = None


def dispatch_paste(
    text: str,
    mode: str = "auto_paste",
    paste_delay_ms: int = 50,
    injector: PasteInjector | None = None,
) -> tuple[bool, str]:
    """Execute complete output workflow: copy to clipboard and optionally emit Ctrl+V.

    Returns:
        (success, result_action) e.g. (True, "pasted") or (True, "copied_to_clipboard")
    """
    if not text:
        return True, "empty"

    copied = copy_to_clipboard(text)
    if not copied:
        return False, "clipboard_failed"

    if mode != "auto_paste":
        return True, "copied_to_clipboard"

    # Small delay so compositor and target window receive clipboard event
    if paste_delay_ms > 0:
        time.sleep(paste_delay_ms / 1000.0)

    inj = injector or PasteInjector()
    if inj.is_available:
        pasted = inj.inject_ctrl_v()
        if pasted:
            return True, "pasted"
        return True, "copied_to_clipboard_fallback"

    logger.warning("Auto-paste requested but uinput unavailable. Falling back to clipboard only.")
    return True, "copied_to_clipboard_fallback"
