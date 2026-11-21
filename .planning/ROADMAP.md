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
- [x] **Phase 10: Tauri v2 Desktop Shell & System Tray**
  - Tokio actor state machine (`Idle` <-> `Recording` <-> `Transcribing` <-> `Pasting`).
  - Native System Tray menu with color-coded status, model selection, and mute toggle.
  - Autostart integration via `tauri-plugin-autostart`.
- [x] **Phase 11: Floating Overlay Pill, Settings UI & Model Manager**
  - Lightweight transparent, draggable floating recording pill with real-time waveform animation.
  - Settings window for device selection, hotkey customization, and custom vocabulary dictionary.
  - Model Manager for background download and SHA-256 verification of quantized GGML models.
- [x] **Phase 12: Desktop Packaging & Multi-Platform CI/CD**
  - Windows: Self-contained `.msi` and NSIS `.exe` installers with WebView2 verification.
  - Linux: Standalone AppImage, `.deb`, `.rpm`, and udev rule installer.
  - Multi-platform automated GitHub Actions CI/CD matrix.
- [x] **Phase 13: Android Core FFI (`uniffi-rs`)**
  - UniFFI interface definitions for `voicekey-core`.
  - Android NDK (`cargo-ndk`) cross-compilation for `aarch64-linux-android`.
  - Android `.aar` library packaging and Kotlin test harness.
- [x] **Phase 14: Android IME & Audio Capture**
  - Android Studio project: `InputMethodService` (VoiceKey custom keyboard).
  - Native `AudioRecord` 16kHz streaming bound to Foreground Service (microphone privacy).
  - Real-time `WaveformView` amplitude visualizer and haptic feedback.
  - Direct text insertion via `InputConnection.commitText()`.
- [x] **Phase 15: Android Settings, Model Manager & Distribution**
  - Settings Activity with in-app model downloader, live progress, and SHA-256 verification.
  - Haptic feedback, spoken punctuation, auto-normalization, and custom vocabulary dictionary.
  - Production APK packaging script (`package_android_apk.sh`) and F-Droid / Fastlane metadata.

## Milestone 3: On-Device AI Commands & Power Ecosystem

Detailed technical specifications and loophole mitigations are documented in [.planning/V2_CROSS_PLATFORM_PLAN.md](file:///home/aryan/Documents/Projects/VoiceKey/.planning/V2_CROSS_PLATFORM_PLAN.md).

- [x] **Phase 16: Micro-LLM Post-Processing & Smart Dictation**
  - Cross-platform micro-LLM inference engine integration (Qwen2.5-0.5B for ultra-fast/Android, Llama-3.2-1B for balanced editing, Qwen2.5-1.5B for code, plus local Ollama endpoint support).
  - High-performance deterministic fast-path (Whisper only, < 150ms) vs. Triggered AI Mode (spoken `"VoiceKey [command]"` prefix or enabled toggle).
  - Core transforms: Bullet point structuring, filler word & stutter removal, tone adjustment (casual/professional), and voice-to-code syntax translation.
  - Zero-latency, zero-dependency offline fallback transformer ensuring voice directives never fail.
  - Model selector, endpoint configuration, and connectivity tester in Desktop Settings UI.
  - CLI `transform` command for direct scripting and verification.
  - Android FFI export `voicekey_transform_text` for instant on-device formatting.
- [x] **Phase 17: Low-Latency Streaming Transcription & Speculative Output**
  - Real-time partial transcript streaming into overlay visualizer with sub-100ms perceived latency.
  - Adaptive VAD silence hysteresis (`AdaptiveVad`) and chunked inference windowing (`StreamingTranscriber`).
  - Hands-free auto-commit: automatically finishes recording and pastes upon detected silence after active speech.
  - Desktop UI: live subtitle card `#streaming-preview` below overlay pill, settings toggles for streaming & silence timeout.
  - Headless CLI: `stream` command for real-time terminal audio transcription.
  - Android FFI: `voicekey_adaptive_vad_check_silence` export for mobile keyboard auto-commit.
- [ ] **Phase 18: Wireless Remote Microphone (Android ↔ Desktop)**
  - Local Wi-Fi / mDNS zero-cloud pairing: use Android device as a low-latency wireless microphone for Linux/Windows desktops.
  - Bidirectional dictionary sync: share custom vocabulary and hotwords across devices.
- [ ] **Phase 19: Multilingual Speech & Instant Translation**
  - Automatic language detection and real-time translation into English using multilingual Whisper and micro-LLM models.
