"""Hardware probing and engine recommendation doctor for VoiceKey."""

from __future__ import annotations

import importlib.util
import os
import platform
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class GPUInfo:
    """Information about detected GPU."""
    name: str
    vendor: str  # "nvidia", "amd", "intel", "unknown"
    vram_mb: int
    driver_ready: bool


@dataclass
class HardwareProfile:
    """System hardware capabilities profile."""
    arch: str
    cpu_model: str
    cpu_cores: int
    has_avx2: bool
    has_avx512: bool
    has_neon: bool
    total_ram_gb: float
    gpus: list[GPUInfo] = field(default_factory=list)
    has_whisper_cpp_bin: bool = False
    whisper_cpp_path: str = ""
    installed_backends: list[str] = field(default_factory=list)


@dataclass
class HardwareRecommendation:
    """Recommended transcription configuration for the machine."""
    backend: str
    model_size: str
    device: str
    compute_type: str
    cpu_threads: int
    rationale: str


def _get_cpu_flags() -> set[str]:
    """Read CPU instruction flags from /proc/cpuinfo."""
    cpuinfo_path = Path("/proc/cpuinfo")
    if not cpuinfo_path.exists():
        return set()

    flags: set[str] = set()
    try:
        with open(cpuinfo_path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if line.startswith("flags") or line.startswith("Features"):
                    parts = line.split(":", 1)
                    if len(parts) == 2:
                        flags.update(parts[1].strip().split())
                    break
    except Exception:
        pass
    return flags


def _get_cpu_model() -> str:
    """Read CPU model name from /proc/cpuinfo or platform."""
    cpuinfo_path = Path("/proc/cpuinfo")
    if cpuinfo_path.exists():
        try:
            with open(cpuinfo_path, "r", encoding="utf-8", errors="ignore") as f:
                for line in f:
                    if "model name" in line:
                        parts = line.split(":", 1)
                        if len(parts) == 2:
                            return parts[1].strip()
        except Exception:
            pass
    return platform.processor() or platform.machine()


def _get_total_ram_gb() -> float:
    """Determine total system RAM in gigabytes."""
    meminfo_path = Path("/proc/meminfo")
    if meminfo_path.exists():
        try:
            with open(meminfo_path, "r", encoding="utf-8", errors="ignore") as f:
                for line in f:
                    if line.startswith("MemTotal:"):
                        parts = line.split()
                        if len(parts) >= 2:
                            kb = float(parts[1])
                            return round(kb / (1024 * 1024), 2)
        except Exception:
            pass

    try:
        pages = os.sysconf("SC_PHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
        return round((pages * page_size) / (1024 ** 3), 2)
    except Exception:
        return 8.0  # Conservative fallback


def _probe_nvidia_gpus() -> list[GPUInfo]:
    """Check for active NVIDIA GPU with working driver."""
    nvidia_smi = shutil.which("nvidia-smi")
    if not nvidia_smi:
        return []

    try:
        res = subprocess.run(
            [nvidia_smi, "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"],
            capture_output=True,
            text=True,
            timeout=3,
        )
        if res.returncode != 0:
            return []

        gpus: list[GPUInfo] = []
        for line in res.stdout.strip().splitlines():
            line = line.strip()
            if not line:
                continue
            parts = [p.strip() for p in line.split(",")]
            if len(parts) >= 2:
                name = parts[0]
                try:
                    vram = int(float(parts[1]))
                except ValueError:
                    vram = 0
                gpus.append(GPUInfo(name=name, vendor="nvidia", vram_mb=vram, driver_ready=True))
        return gpus
    except Exception:
        return []


def probe_hardware() -> HardwareProfile:
    """Probe system hardware and installed speech backends."""
    arch = platform.machine().lower()
    flags = _get_cpu_flags()
    cpu_model = _get_cpu_model()
    cpu_cores = os.cpu_count() or 4
    total_ram = _get_total_ram_gb()

    has_avx2 = "avx2" in flags
    has_avx512 = any(f.startswith("avx512") for f in flags)
    has_neon = "asimd" in flags or "neon" in flags or arch in {"aarch64", "arm64"}

    gpus = _probe_nvidia_gpus()

    # Check installed backends
    installed: list[str] = []
    if importlib.util.find_spec("faster_whisper") is not None:
        installed.append("faster-whisper")
    if importlib.util.find_spec("whisper") is not None:
        installed.append("openai-whisper")
    if importlib.util.find_spec("whisperx") is not None:
        installed.append("whisperx")

    whisper_cpp_bin = shutil.which("whisper-cli") or shutil.which("whisper-cpp") or shutil.which("main")
    has_whisper_cpp = whisper_cpp_bin is not None
    if has_whisper_cpp:
        installed.append("whisper.cpp")

    return HardwareProfile(
        arch=arch,
        cpu_model=cpu_model,
        cpu_cores=cpu_cores,
        has_avx2=has_avx2,
        has_avx512=has_avx512,
        has_neon=has_neon,
        total_ram_gb=total_ram,
        gpus=gpus,
        has_whisper_cpp_bin=has_whisper_cpp,
        whisper_cpp_path=whisper_cpp_bin or "",
        installed_backends=installed,
    )


def recommend_setup(profile: HardwareProfile) -> HardwareRecommendation:
    """Evaluate profile and recommend optimal backend, model, and compute settings."""
    active_gpu = next((g for g in profile.gpus if g.driver_ready and g.vram_mb >= 3000), None)

    # 1. High-Performance GPU available
    if active_gpu:
        threads = min(4, profile.cpu_cores)
        if active_gpu.vram_mb >= 8000:
            return HardwareRecommendation(
                backend="faster-whisper",
                model_size="distil-large-v3",
                device="cuda",
                compute_type="float16",
                cpu_threads=threads,
                rationale=(
                    f"Active NVIDIA GPU ({active_gpu.name}, {active_gpu.vram_mb}MB VRAM) detected. "
                    "CUDA float16 provides ultra-low latency (<150ms) with distil-large-v3 accuracy."
                ),
            )
        return HardwareRecommendation(
            backend="faster-whisper",
            model_size="base.en",
            device="cuda",
            compute_type="float16",
            cpu_threads=threads,
            rationale=(
                f"Active NVIDIA GPU ({active_gpu.name}) detected. "
                "CUDA float16 provides instant transcription with base.en model."
            ),
        )

    # 2. Modern CPU with AVX2 or AVX-512 and ample RAM (e.g. >= 6GB)
    threads = min(profile.cpu_cores, 8)
    if profile.has_avx2 and profile.total_ram_gb >= 6.0:
        return HardwareRecommendation(
            backend="faster-whisper",
            model_size="base.en",
            device="cpu",
            compute_type="int8",
            cpu_threads=threads,
            rationale=(
                f"Fast CPU with AVX2/AVX-512 vector acceleration ({profile.cpu_model}, {profile.total_ram_gb:.1f}GB RAM). "
                "CTranslate2 INT8 quantized execution achieves <200ms warm latency with base.en."
            ),
        )

    # 3. Lightweight / Low-spec CPU / Minimal RAM / Embedded ARM
    threads = max(1, min(profile.cpu_cores, 4))
    return HardwareRecommendation(
        backend="whisper.cpp",
        model_size="tiny.en",
        device="cpu",
        compute_type="int8",
        cpu_threads=threads,
        rationale=(
            f"Low-resource environment ({profile.cpu_cores} cores, {profile.total_ram_gb:.1f}GB RAM, no AVX2). "
            "whisper.cpp GGML provides ultra-low memory footprint (~200MB) without Python/PyTorch overhead."
        ),
    )


def format_doctor_report(profile: HardwareProfile, rec: HardwareRecommendation) -> str:
    """Format human-readable diagnostic report for CLI output."""
    lines: list[str] = [
        "===========================================================",
        "           VoiceKey Hardware & Engine Doctor               ",
        "===========================================================",
        f"  Architecture:      {profile.arch}",
        f"  CPU Model:         {profile.cpu_model}",
        f"  CPU Cores:         {profile.cpu_cores} threads",
        f"  Vector Accel:      AVX2: {'YES' if profile.has_avx2 else 'NO'} | AVX-512: {'YES' if profile.has_avx512 else 'NO'} | NEON: {'YES' if profile.has_neon else 'NO'}",
        f"  Total RAM:         {profile.total_ram_gb:.1f} GB",
    ]

    if profile.gpus:
        for gpu in profile.gpus:
            lines.append(f"  GPU Detected:      {gpu.name} ({gpu.vram_mb} MB VRAM, ready={gpu.driver_ready})")
    else:
        lines.append("  GPU Detected:      None (or driver inactive)")

    installed_str = ", ".join(profile.installed_backends) if profile.installed_backends else "None"
    lines.append(f"  Installed Engines: {installed_str}")
    lines.append("-----------------------------------------------------------")
    lines.append("  RECOMMENDED CONFIGURATION FOR THIS MACHINE:")
    lines.append(f"    Backend:         {rec.backend}")
    lines.append(f"    Model Size:      {rec.model_size}")
    lines.append(f"    Device:          {rec.device}")
    lines.append(f"    Compute Type:    {rec.compute_type}")
    lines.append(f"    CPU Threads:     {rec.cpu_threads}")
    lines.append("")
    lines.append(f"  Reason: {rec.rationale}")
    lines.append("===========================================================")
    return "\n".join(lines)
