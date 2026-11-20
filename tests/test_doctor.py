"""Tests for Hardware Doctor and engine recommendations."""

from unittest.mock import MagicMock, patch
import pytest

from voicekey.doctor import (
    GPUInfo,
    HardwareProfile,
    HardwareRecommendation,
    format_doctor_report,
    probe_hardware,
    recommend_setup,
    _probe_nvidia_gpus,
)


def test_probe_hardware_real_system():
    """Verify hardware probe runs without error on host system."""
    profile = probe_hardware()
    assert isinstance(profile, HardwareProfile)
    assert profile.cpu_cores > 0
    assert profile.total_ram_gb > 0
    assert profile.arch != ""


def test_probe_nvidia_gpus_failure():
    """nvidia-smi returning non-zero exit code yields empty list."""
    mock_run = MagicMock()
    mock_run.return_value.returncode = 9
    mock_run.return_value.stdout = "Driver error"

    with patch("shutil.which", return_value="/usr/bin/nvidia-smi"), patch("subprocess.run", mock_run):
        gpus = _probe_nvidia_gpus()
        assert gpus == []


def test_probe_nvidia_gpus_success():
    """nvidia-smi returning GPU details parses successfully."""
    mock_run = MagicMock()
    mock_run.return_value.returncode = 0
    mock_run.return_value.stdout = "NVIDIA GeForce RTX 4060, 8192\n"

    with patch("shutil.which", return_value="/usr/bin/nvidia-smi"), patch("subprocess.run", mock_run):
        gpus = _probe_nvidia_gpus()
        assert len(gpus) == 1
        assert gpus[0].name == "NVIDIA GeForce RTX 4060"
        assert gpus[0].vram_mb == 8192
        assert gpus[0].vendor == "nvidia"
        assert gpus[0].driver_ready is True


def test_recommend_setup_for_gpu():
    """High-VRAM GPU gets recommended faster-whisper on CUDA with float16."""
    profile = HardwareProfile(
        arch="x86_64",
        cpu_model="Core i7",
        cpu_cores=16,
        has_avx2=True,
        has_avx512=False,
        has_neon=False,
        total_ram_gb=32.0,
        gpus=[GPUInfo(name="RTX 4090", vendor="nvidia", vram_mb=24576, driver_ready=True)],
    )

    rec = recommend_setup(profile)
    assert rec.backend == "faster-whisper"
    assert rec.device == "cuda"
    assert rec.compute_type == "float16"
    assert "RTX 4090" in rec.rationale


def test_recommend_setup_for_modern_cpu():
    """AVX2 CPU without active GPU gets recommended faster-whisper on CPU with int8."""
    profile = HardwareProfile(
        arch="x86_64",
        cpu_model="Core i7-13650HX",
        cpu_cores=14,
        has_avx2=True,
        has_avx512=False,
        has_neon=False,
        total_ram_gb=16.0,
        gpus=[],
    )

    rec = recommend_setup(profile)
    assert rec.backend == "faster-whisper"
    assert rec.device == "cpu"
    assert rec.compute_type == "int8"
    assert rec.model_size == "base.en"


def test_recommend_setup_for_low_spec():
    """Low-spec machine without AVX2 or low RAM gets recommended whisper.cpp."""
    profile = HardwareProfile(
        arch="armv7l",
        cpu_model="Raspberry Pi Cortex-A72",
        cpu_cores=4,
        has_avx2=False,
        has_avx512=False,
        has_neon=True,
        total_ram_gb=2.0,
        gpus=[],
    )

    rec = recommend_setup(profile)
    assert rec.backend == "whisper.cpp"
    assert rec.model_size == "tiny.en"
    assert rec.device == "cpu"


def test_format_doctor_report():
    """format_doctor_report produces all expected diagnostic fields."""
    profile = HardwareProfile(
        arch="x86_64",
        cpu_model="AMD Ryzen 7",
        cpu_cores=8,
        has_avx2=True,
        has_avx512=False,
        has_neon=False,
        total_ram_gb=16.0,
        installed_backends=["faster-whisper"],
    )
    rec = recommend_setup(profile)
    report = format_doctor_report(profile, rec)

    assert "VoiceKey Hardware & Engine Doctor" in report
    assert "AMD Ryzen 7" in report
    assert "AVX2: YES" in report
    assert "RECOMMENDED CONFIGURATION" in report
    assert "faster-whisper" in report
