"""Tests for audio feedback and desktop notification modules."""

from pathlib import Path
from unittest.mock import MagicMock, patch
import pytest

from voicekey.feedback import AudioFeedback, SoundCue, _generate_wav_beep
from voicekey.notify import send_notification


def test_audio_feedback_disabled():
    """Disabled AudioFeedback returns False and spawns no subprocesses."""
    feedback = AudioFeedback(enabled=False)
    assert not feedback.play(SoundCue.START)
    assert not feedback.play_start()
    assert not feedback.play_stop()
    assert not feedback.play_error()


def test_audio_feedback_play_invokes_player(monkeypatch: pytest.MonkeyPatch):
    """Playing a sound cue triggers player subprocess."""
    feedback = AudioFeedback(enabled=True)
    feedback.player_bin = "/usr/bin/pw-play"
    mock_file = Path("/tmp/fake_cue.oga")
    feedback._sound_paths[SoundCue.START] = mock_file

    mock_popen = MagicMock()
    with patch("subprocess.Popen", mock_popen), patch.object(Path, "exists", return_value=True):
        res = feedback.play_start()
        assert res is True
        mock_popen.assert_called_once()
        cmd = mock_popen.call_args[0][0]
        assert cmd[0] == "/usr/bin/pw-play"
        assert cmd[1] == str(mock_file)


def test_generate_wav_beep(tmp_path: Path):
    """Synthetic WAV generation produces a valid readable WAV file."""
    wav_path = tmp_path / "beep.wav"
    _generate_wav_beep(wav_path, start_freq=440.0, end_freq=880.0, duration_s=0.05, sample_rate=16000)

    assert wav_path.exists()
    assert wav_path.stat().st_size > 100

    import wave
    with wave.open(str(wav_path), "rb") as w:
        assert w.getnchannels() == 1
        assert w.getsampwidth() == 2
        assert w.getframerate() == 16000
        frames = w.getnframes()
        assert frames == int(16000 * 0.05)


def test_audio_feedback_fallback_synthesis(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """When system sound files do not exist, AudioFeedback generates synthetic fallback tones."""
    monkeypatch.setattr("voicekey.feedback.SYSTEM_SOUND_CANDIDATES", {
        SoundCue.START: [tmp_path / "nonexistent_start.oga"],
        SoundCue.STOP: [tmp_path / "nonexistent_stop.oga"],
        SoundCue.ERROR: [tmp_path / "nonexistent_error.oga"],
    })

    feedback = AudioFeedback(enabled=True)
    for cue in SoundCue:
        cue_path = feedback._sound_paths.get(cue)
        assert cue_path is not None
        assert cue_path.exists()


def test_send_notification_success():
    """send_notification triggers notify-send with expected arguments."""
    mock_popen = MagicMock()
    with patch("shutil.which", return_value="/usr/bin/notify-send"), patch("subprocess.Popen", mock_popen):
        res = send_notification(
            summary="VoiceKey",
            body="Recording started...",
            urgency="low",
            timeout_ms=1500,
        )
        assert res is True
        mock_popen.assert_called_once()
        cmd = mock_popen.call_args[0][0]
        assert cmd[0] == "/usr/bin/notify-send"
        assert "-a" in cmd and "VoiceKey" in cmd
        assert "-t" in cmd and "1500" in cmd
        assert "Recording started..." in cmd


def test_send_notification_binary_missing():
    """send_notification returns False gracefully when notify-send is absent."""
    with patch("shutil.which", return_value=None):
        res = send_notification("Summary")
        assert res is False
