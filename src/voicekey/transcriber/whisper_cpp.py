"""Ultra-lightweight whisper.cpp (GGML) transcription backend."""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import tempfile
import urllib.request
import wave
from pathlib import Path
import numpy as np

from voicekey.config import TranscriptionConfig
from voicekey.transcriber.base import BaseTranscriber

logger = logging.getLogger("voicekey.transcriber.whisper_cpp")

DEFAULT_DATA_DIR = Path.home() / ".local" / "share" / "voicekey"
WHISPER_CPP_DIR = DEFAULT_DATA_DIR / "whisper.cpp"
MODELS_DIR = DEFAULT_DATA_DIR / "models"


def get_ggml_model_url(model_size: str) -> str:
    """Return HuggingFace download URL for standard whisper.cpp GGML model."""
    clean_size = model_size.lower().removeprefix("ggml-")
    return f"https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{clean_size}.bin"


def ensure_ggml_model(model_size: str, models_dir: Path | None = None) -> Path:
    """Ensure GGML model file is present locally, downloading if necessary."""
    target_dir = models_dir or MODELS_DIR
    target_dir.mkdir(parents=True, exist_ok=True)

    clean_size = model_size.lower().removeprefix("ggml-")
    model_path = target_dir / f"ggml-{clean_size}.bin"
    if model_path.exists() and model_path.stat().st_size > 1024 * 1024:
        return model_path

    url = get_ggml_model_url(clean_size)
    logger.info("Downloading whisper.cpp GGML model from %s to %s...", url, model_path)
    tmp_path = model_path.with_suffix(".download")

    try:
        urllib.request.urlretrieve(url, tmp_path)
        tmp_path.rename(model_path)
        logger.info("Successfully downloaded GGML model '%s'.", clean_size)
        return model_path
    except Exception as e:
        if tmp_path.exists():
            tmp_path.unlink()
        raise RuntimeError(f"Failed to download whisper.cpp model '{clean_size}': {e}") from e


def find_whisper_cpp_binary(custom_path: str = "") -> Path | None:
    """Locate whisper.cpp executable on system or local install."""
    if custom_path:
        p = Path(custom_path).expanduser().resolve()
        if p.is_file() and os.access(p, os.X_OK):
            return p

    # Local built binaries
    local_candidates = [
        WHISPER_CPP_DIR / "build" / "bin" / "whisper-cli",
        WHISPER_CPP_DIR / "build" / "bin" / "main",
        WHISPER_CPP_DIR / "main",
    ]
    for candidate in local_candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate

    # System PATH candidates
    for name in ["whisper-cli", "whisper-cpp", "main"]:
        sys_bin = shutil.which(name)
        if sys_bin:
            return Path(sys_bin)

    return None


def ensure_whisper_cpp_binary(target_dir: Path | None = None) -> Path:
    """Ensure whisper.cpp executable is available, cloning and building if needed."""
    existing = find_whisper_cpp_binary()
    if existing:
        return existing

    repo_dir = target_dir or WHISPER_CPP_DIR
    logger.info("whisper.cpp executable not found. Cloning and compiling to %s...", repo_dir)

    git_bin = shutil.which("git")
    if not git_bin:
        raise RuntimeError("git is required to automatically clone whisper.cpp.")

    repo_dir.parent.mkdir(parents=True, exist_ok=True)

    if not repo_dir.exists():
        subprocess.run(
            [git_bin, "clone", "--depth", "1", "https://github.com/ggml-org/whisper.cpp.git", str(repo_dir)],
            check=True,
            capture_output=True,
        )

    # Build using cmake or make
    cmake_bin = shutil.which("cmake")
    if cmake_bin:
        build_dir = repo_dir / "build"
        subprocess.run([cmake_bin, "-B", str(build_dir)], cwd=repo_dir, check=True, capture_output=True)
        subprocess.run([cmake_bin, "--build", str(build_dir), "-j"], cwd=repo_dir, check=True, capture_output=True)
    else:
        make_bin = shutil.which("make")
        if not make_bin:
            raise RuntimeError("Neither cmake nor make found to compile whisper.cpp.")
        subprocess.run([make_bin, "-j"], cwd=repo_dir, check=True, capture_output=True)

    built = find_whisper_cpp_binary()
    if not built:
        raise RuntimeError(f"whisper.cpp build completed but executable was not found in {repo_dir}.")

    logger.info("Successfully built whisper.cpp binary: %s", built)
    return built


class WhisperCppTranscriber(BaseTranscriber):
    """Ultra-lightweight standalone C++ transcription backend using whisper.cpp."""

    name = "whisper.cpp"

    def __init__(self, config: TranscriptionConfig) -> None:
        super().__init__(config)
        self.bin_path: Path | None = None
        self.model_path: Path | None = None

    @classmethod
    def is_available(cls) -> bool:
        """Return True if binary exists or build tools exist."""
        if find_whisper_cpp_binary():
            return True
        return bool(shutil.which("git") and (shutil.which("cmake") or shutil.which("make")))

    def initialize(self) -> None:
        """Resolve whisper.cpp binary and GGML model file."""
        if self._is_loaded and self.bin_path and self.model_path:
            return

        self.bin_path = find_whisper_cpp_binary(self.config.whisper_cpp_path)
        if not self.bin_path:
            self.bin_path = ensure_whisper_cpp_binary()

        self.model_path = ensure_ggml_model(self.config.model_size)
        self._is_loaded = True
        logger.info("whisper.cpp ready (bin=%s, model=%s).", self.bin_path, self.model_path)

    def transcribe_raw(self, audio_data: np.ndarray) -> str:
        """Transcribe audio by invoking whisper.cpp CLI."""
        if not self._is_loaded or not self.bin_path or not self.model_path:
            self.initialize()

        assert self.bin_path is not None
        assert self.model_path is not None

        # Write audio to temporary 16kHz 16-bit mono WAV file
        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp_wav:
            tmp_wav_path = Path(tmp_wav.name)

        try:
            # Convert float32 [-1.0, 1.0] to int16 PCM
            clipped = np.clip(audio_data, -1.0, 1.0)
            pcm16 = (clipped * 32767.0).astype(np.int16)

            with wave.open(str(tmp_wav_path), "wb") as wf:
                wf.setnchannels(1)
                wf.setsampwidth(2)
                wf.setframerate(16000)
                wf.writeframes(pcm16.tobytes())

            cmd = [
                str(self.bin_path),
                "-m", str(self.model_path),
                "-f", str(tmp_wav_path),
                "-nt",  # No timestamps
                "-np",  # No prints/progress
                "-t", str(self.config.cpu_threads),
            ]

            prompt = self.build_prompt()
            if prompt:
                cmd.extend(["--prompt", prompt])

            res = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=False,
            )

            if res.returncode != 0:
                logger.error("whisper.cpp execution failed (code %d): %s", res.returncode, res.stderr)
                return ""

            return res.stdout.strip()
        finally:
            if tmp_wav_path.exists():
                tmp_wav_path.unlink()

    def unload(self) -> None:
        """Unload paths and mark uninitialized."""
        self.bin_path = None
        self.model_path = None
        self._is_loaded = False
