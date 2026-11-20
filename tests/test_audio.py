"""Tests for audio recording engine."""

from unittest.mock import MagicMock, patch
import numpy as np
import pytest

from voicekey.audio import AudioRecorder
from voicekey.config import AudioConfig
from voicekey.feedback import AudioFeedback


@pytest.fixture
def mock_feedback():
    """Mock AudioFeedback instance."""
    fb = MagicMock(spec=AudioFeedback)
    return fb


def test_audio_recorder_initial_state(mock_feedback):
    """AudioRecorder starts in idle state."""
    recorder = AudioRecorder(feedback=mock_feedback)
    assert not recorder.is_recording
    assert recorder.config.sample_rate == 16000
    assert recorder.config.channels == 1


def test_audio_recorder_start_stop_mocked(mock_feedback):
    """Starting and stopping recorder manages stream and accumulates frames."""
    recorder = AudioRecorder(feedback=mock_feedback)

    mock_stream = MagicMock()
    with patch("sounddevice.InputStream", return_value=mock_stream):
        started = recorder.start(play_cue=True)
        assert started is True
        assert recorder.is_recording
        mock_feedback.play_start.assert_called_once()
        mock_stream.start.assert_called_once()

        # Simulate audio callback delivering 2 frames of 100 samples
        chunk1 = np.ones((100, 1), dtype=np.float32) * 0.5
        chunk2 = np.ones((100, 1), dtype=np.float32) * 0.25
        recorder._audio_callback(chunk1, 100, {}, MagicMock())
        recorder._audio_callback(chunk2, 100, {}, MagicMock())

        # Stop recorder
        audio = recorder.stop(play_cue=True)
        assert not recorder.is_recording
        mock_feedback.play_stop.assert_called_once()
        mock_stream.stop.assert_called_once()
        mock_stream.close.assert_called_once()

        assert isinstance(audio, np.ndarray)
        assert audio.dtype == np.float32
        assert len(audio) == 200
        assert np.isclose(audio[0], 0.5)
        assert np.isclose(audio[150], 0.25)


def test_audio_recorder_double_start(mock_feedback):
    """Calling start() twice returns False without resetting state."""
    recorder = AudioRecorder(feedback=mock_feedback)
    mock_stream = MagicMock()

    with patch("sounddevice.InputStream", return_value=mock_stream):
        assert recorder.start() is True
        assert recorder.start() is False
        assert recorder.is_recording

        recorder.stop()
        assert not recorder.is_recording


def test_audio_recorder_stop_when_idle(mock_feedback):
    """Calling stop() when idle returns an empty float32 array."""
    recorder = AudioRecorder(feedback=mock_feedback)
    audio = recorder.stop()
    assert isinstance(audio, np.ndarray)
    assert len(audio) == 0
    assert audio.dtype == np.float32


def test_audio_recorder_start_error_handling(mock_feedback):
    """Stream initialization error resets is_recording and plays error cue."""
    recorder = AudioRecorder(feedback=mock_feedback)

    with patch("sounddevice.InputStream", side_effect=RuntimeError("Device unavailable")):
        with pytest.raises(RuntimeError, match="Device unavailable"):
            recorder.start(play_cue=True)

        assert not recorder.is_recording
        mock_feedback.play_error.assert_called_once()


def test_audio_metrics_calculation():
    """calculate_rms and calculate_peak correctly evaluate signals."""
    # Silence
    silence = np.zeros(100, dtype=np.float32)
    assert AudioRecorder.calculate_rms(silence) == 0.0
    assert AudioRecorder.calculate_peak(silence) == 0.0

    # Constant signal 0.5
    constant = np.full(100, 0.5, dtype=np.float32)
    assert np.isclose(AudioRecorder.calculate_rms(constant), 0.5)
    assert np.isclose(AudioRecorder.calculate_peak(constant), 0.5)

    # Empty array
    empty = np.empty(0, dtype=np.float32)
    assert AudioRecorder.calculate_rms(empty) == 0.0
    assert AudioRecorder.calculate_peak(empty) == 0.0


def test_to_pcm16_bytes():
    """to_pcm16_bytes maps [-1.0, 1.0] to 16-bit signed PCM."""
    samples = np.array([-1.0, 0.0, 1.0, 1.5, -1.5], dtype=np.float32)
    pcm_bytes = AudioRecorder.to_pcm16_bytes(samples)

    # 5 samples * 2 bytes = 10 bytes
    assert len(pcm_bytes) == 10
    int16_arr = np.frombuffer(pcm_bytes, dtype=np.int16)
    assert int16_arr[0] == -32767
    assert int16_arr[1] == 0
    assert int16_arr[2] == 32767
    assert int16_arr[3] == 32767  # clipped
    assert int16_arr[4] == -32767  # clipped
