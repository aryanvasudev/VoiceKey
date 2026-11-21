"""Tests for clipboard copying and virtual keystroke injection."""

from unittest.mock import MagicMock, patch
import pytest

from voicekey.paste import (
    PasteInjector,
    check_uinput_accessible,
    copy_to_clipboard,
    dispatch_paste,
)


def test_copy_to_clipboard_empty():
    """Empty string returns True immediately without launching subprocess."""
    assert copy_to_clipboard("") is True


def test_copy_to_clipboard_wl_copy():
    """copy_to_clipboard invokes wl-copy with encoded text and non-pipe stderr."""
    import subprocess
    mock_run = MagicMock()
    mock_run.return_value.returncode = 0

    with patch("shutil.which", side_effect=lambda bin_name: "/usr/bin/wl-copy" if bin_name == "wl-copy" else None), \
         patch("subprocess.run", mock_run):
        assert copy_to_clipboard("test dictation") is True
        mock_run.assert_called_once()
        cmd = mock_run.call_args[0][0]
        assert cmd[0] == "/usr/bin/wl-copy"
        assert mock_run.call_args[1]["input"] == b"test dictation"
        assert mock_run.call_args[1]["stderr"] != subprocess.PIPE


def test_copy_to_clipboard_xclip_fallback():
    """When wl-copy is absent, copy_to_clipboard falls back to xclip."""
    import subprocess
    mock_run = MagicMock()
    mock_run.return_value.returncode = 0

    with patch("shutil.which", side_effect=lambda bin_name: "/usr/bin/xclip" if bin_name == "xclip" else None), \
         patch("subprocess.run", mock_run):
        assert copy_to_clipboard("x11 text") is True
        mock_run.assert_called_once()
        cmd = mock_run.call_args[0][0]
        assert cmd[0] == "/usr/bin/xclip"
        assert "-selection" in cmd
        assert mock_run.call_args[1]["stderr"] != subprocess.PIPE


def test_copy_to_clipboard_failure():
    """When no clipboard binary is present, copy_to_clipboard returns False."""
    with patch("shutil.which", return_value=None):
        assert copy_to_clipboard("test") is False


def test_check_uinput_accessible_mocked():
    """check_uinput_accessible reports permission error instructions when not writable."""
    with patch("pathlib.Path.exists", return_value=True), patch("os.access", return_value=False):
        ok, msg = check_uinput_accessible()
        assert ok is False
        assert "sudo usermod -aG input" in msg


def test_paste_injector_inject_ctrl_v():
    """PasteInjector sends Ctrl+V key sequence and syn events."""
    injector = PasteInjector()
    mock_ui = MagicMock()
    injector._uinput = mock_ui
    injector._accessible = True

    with patch("time.sleep", return_value=None):
        res = injector.inject_ctrl_v(debounce_ms=10)
        assert res is True
        # At least 2 syn calls (press syn, release syn)
        assert mock_ui.syn.call_count >= 2
        # At least 4 key writes (Ctrl down, V down, V up, Ctrl up)
        assert mock_ui.write.call_count >= 4

    injector.close()
    mock_ui.close.assert_called_once()


def test_dispatch_paste_modes():
    """dispatch_paste respects clipboard_only and auto_paste modes."""
    # 1. Empty text
    ok, action = dispatch_paste("")
    assert ok is True
    assert action == "empty"

    mock_injector = MagicMock(spec=PasteInjector)
    mock_injector.is_available = True
    mock_injector.inject_ctrl_v.return_value = True

    # 2. Clipboard only mode
    with patch("voicekey.paste.copy_to_clipboard", return_value=True):
        ok, action = dispatch_paste("text", mode="clipboard_only", injector=mock_injector)
        assert ok is True
        assert action == "copied_to_clipboard"
        mock_injector.inject_ctrl_v.assert_not_called()

    # 3. Auto paste mode
    with patch("voicekey.paste.copy_to_clipboard", return_value=True), patch("time.sleep", return_value=None):
        ok, action = dispatch_paste("text", mode="auto_paste", injector=mock_injector)
        assert ok is True
        assert action == "pasted"
        mock_injector.inject_ctrl_v.assert_called_once()
