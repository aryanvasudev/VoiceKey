# VoiceKey Roadmap

## Milestone 1: MVP Core (Hotkey Dictation & Auto-Paste)

- [x] **Phase 1: Project Scaffolding & Configuration**
  - Setup `pyproject.toml`, console entry points (`voicekey`, `voicekeyd`).
  - Implement config manager (`~/.config/voicekey/config.toml`) with sensible defaults (`model="base.en"`, `device="cpu"`, `compute_type="int8"`).
- [x] **Phase 2: Audio Recording & Feedback Chimes**
  - Ring buffer audio recorder via `sounddevice` (16kHz 16-bit mono PCM).
  - Audio start/stop chimes using `pw-play` or synthesized tones.
  - Desktop notifications using `notify-send`.
- [x] **Phase 3: Multi-Backend Transcriber Engine & Hardware Doctor**
  - Modular pluggable transcriber architecture (`BaseTranscriber` interface).
  - Backends: `faster-whisper` (default warm CTranslate2), `whisper.cpp` (ultra-lightweight GGML for low-spec CPUs/ARM/iGPUs), `openai-whisper` (official vanilla PyTorch), and `whisperx` (forced alignment & VAD).
  - Hardware profiler & recommender (`voicekey doctor`) to suggest optimal backend & model per device.
  - Vocabulary biasing / hotword prompting for jargon & programming terms.
  - Text post-processing (whitespace, punctuation, capitalization normalization, and spoken punctuation).
- [x] **Phase 4: Daemon IPC & CLI Client**
  - Unix Domain Socket server (`~/.local/state/voicekey/daemon.sock`).
  - Instant CLI client (`voicekey toggle`, `voicekey start`, `voicekey stop`, `voicekey status`, `voicekey doctor`).
- [x] **Phase 5: Wayland Auto-Paste & Hotkey Integration**
  - Fast clipboard copy using `wl-copy`.
  - Automatic `Ctrl+V` key injection via `/dev/uinput` (`evdev.UInput`).
  - GNOME shortcut binding script (`gsettings`).
- [x] **Phase 6: Verification & Systemd Autostart**
  - End-to-end integration test suite (`tests/test_e2e.py`) verifying full toggle -> record -> transcribe -> postprocess -> paste pipeline.
  - Systemd user service generator and lifecycle management (`voicekey autostart --install`, `--remove`, `--start`, `--stop`, `--status`).
  - Automated installer script (`scripts/install_systemd_service.sh`).
  - Comprehensive environment and installation verification diagnostic (`scripts/verify_installation.py`).

## Milestone 2: Cross-Platform Ecosystem (Linux, Windows, Android)

Detailed technical specifications and loophole mitigations are documented in [.planning/V2_CROSS_PLATFORM_PLAN.md](file:///home/aryan/Documents/Projects/VoiceKey/.planning/V2_CROSS_PLATFORM_PLAN.md).

- [x] **Phase 7: Shared Rust Engine (`voicekey-core`)**
  - Audio pipeline: `rubato` 16kHz resampling, stereo-to-mono downmixing, circular buffer with 200ms pre-roll.
  - Quantized GGML Whisper inference via `whisper-rs` (`whisper.cpp` engine).
  - VAD (energy gating and silence trimming), custom vocabulary biasing, and text post-processing.
  - Parity test suite against Python v1 baseline outputs.
- [x] **Phase 8: Headless CLI & Cross-Platform Audio Capture**
  - Standalone headless verification tool (`voicekey-cli`).
  - Audio capture via `cpal` (WASAPI on Windows, PipeWire/ALSA on Linux) with dynamic device reconnection.
- [x] **Phase 9: Cross-Platform Input Injection & 3-Tier Hotkeys**
  - Windows: Win32 `SendInput` hardware keystroke injection and `WH_KEYBOARD_LL` low-level hook (supporting Push-to-Talk and Toggle).
  - Linux: XDG Desktop Portal Global Shortcuts with IPC socket fallback for desktop shortcuts (`gsettings`, `sway`, `hyprland`).
  - Terminal-aware pasting (`Ctrl+Shift+V`) and 250ms debounced clipboard restoration.
- [ ] **Phase 10: Tauri v2 Desktop Shell & System Tray**
  - Tokio actor state machine (`Idle` <-> `Recording` <-> `Transcribing` <-> `Pasting`).
  - Native System Tray menu with color-coded status, model selection, and mute toggle.
  - Autostart integration via `tauri-plugin-autostart`.
- [ ] **Phase 11: Floating Overlay Pill, Settings UI & Model Manager**
  - Lightweight transparent, draggable floating recording pill with real-time waveform animation.
  - Settings window for device selection, hotkey customization, and custom vocabulary dictionary.
  - Model Manager for background download and SHA-256 verification of quantized GGML models.
- [ ] **Phase 12: Desktop Packaging & Multi-Platform CI/CD**
  - Windows: Self-contained `.msi` and NSIS `.exe` installers with WebView2 verification.
  - Linux: Standalone AppImage, `.deb`, `.rpm`, and udev rule installer.
  - Multi-platform automated GitHub Actions CI/CD matrix.
- [ ] **Phase 13: Android Core FFI (`uniffi-rs`)**
  - UniFFI interface definitions for `voicekey-core`.
  - Android NDK (`cargo-ndk`) cross-compilation for `aarch64-linux-android`.
  - Android `.aar` library packaging and Kotlin test harness.
- [ ] **Phase 14: Android IME & Audio Capture**
  - Android Studio project: `InputMethodService` (VoiceKey custom keyboard).
  - Native `AudioRecord` 16kHz streaming bound to Foreground Service (microphone privacy).
  - Direct text insertion via `InputConnection.commitText()`.
- [ ] **Phase 15: Android Settings, Model Manager & Distribution**
  - Jetpack Compose settings screen and in-app model downloader.
  - Haptic feedback and custom prompt dictionary.
  - Production APK / AAB builds for GitHub Releases and F-Droid.
