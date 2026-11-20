# VoiceKey v2 Master Roadmap & Implementation Plan: Cross-Platform (Linux, Windows, Android)

This document outlines the end-to-end plan to transition VoiceKey from the Milestone 1 Python prototype into a unified, high-performance, cross-platform ecosystem once v1 is complete.

---

## 1. System Architecture Overview

```
                               ┌─────────────────────────────────────────┐
                               │             voicekey-core               │
                               │        (Pure Rust Shared Crate)         │
                               │                                         │
                               │  • whisper-rs (whisper.cpp engine)      │
                               │  • Audio ring buffer & pre-roll math    │
                               │  • Vocabulary biasing & prompt engine   │
                               │  • Punctuation & text post-processing   │
                               │  • Abstract State Machine               │
                               └────────────────────┬────────────────────┘
                                                    │
                   ┌────────────────────────────────┴───────────────────────────────┐
                   │                                                                │
                   ▼                                                                ▼
┌──────────────────────────────────────┐                         ┌──────────────────────────────────────┐
│        voicekey-desktop              │                         │          voicekey-android            │
│       (Tauri v2 + Rust)              │                         │           (Kotlin + JNI)             │
│                                      │                         │                                      │
│  Targets: Windows 10/11 & Linux      │                         │  Target: Android 10+ (ARM64)         │
│  UI: Svelte/React + TailwindCSS      │                         │  UI: Jetpack Compose                 │
│  • Floating recording pill           │                         │  • Custom Keyboard (IME)             │
│  • System Tray Menu                  │                         │  • One-tap mic overlay on keyboard   │
│  • Global hotkeys (tauri-plugin)     │                         │  • AudioRecord permission harness    │
│  • Virtual paste:                    │                         │  • Direct text commit:               │
│    - Windows: Win32 SendInput        │                         │    InputConnection.commitText()      │
│    - Linux: /dev/uinput or wl-copy   │                         │  • UniFFI-generated Kotlin bindings  │
│  • Cross-platform audio: cpal        │                         │                                      │
└──────────────────────────────────────┘                         └──────────────────────────────────────┘
```

---

## 2. Shared Core (`voicekey-core`) Strategy

By extracting all business logic into a platform-agnostic Rust crate (`voicekey-core`), **over 70% of the entire codebase is written once and shared across Linux, Windows, and Android**:

1. **Inference Pipeline:**
   - Wraps `whisper.cpp` via `whisper-rs`.
   - Supports Quantized models (`Q4_0`, `Q5_0`, `Q8_0` for GGML/GGUF), drastically reducing RAM and disk footprint:
     - `tiny.en` (~40 MB)
     - `base.en` (~75 MB)
     - `small.en` (~240 MB)
   - On Desktop (x86_64): compiles with AVX2/FMA and optional Vulkan/CUDA.
   - On Android (aarch64): compiles with ARM NEON SIMD optimizations.
2. **Audio Buffer & Math:**
   - Circular ring buffer for continuous streaming.
   - 16kHz 16-bit Mono PCM normalization.
   - Silence trimming / RMS energy voice activity detection (VAD).
3. **Text Formatting Engine:**
   - Hotword conditioning (preserving technical terms, punctuation commands).
   - Natural spoken formatting ("new line", "comma", "exclamation mark").

---

## 3. Implementation Phases

### Phase 1: Core Rust Crate Extraction (`voicekey-core`)
- [ ] Initialize Cargo workspace with `crates/voicekey-core`.
- [ ] Port audio ring buffer and VAD logic from Python `audio.py`.
- [ ] Integrate `whisper-rs` with static linking to `whisper.cpp`.
- [ ] Port punctuation normalization and vocabulary biasing from Python `transcriber.py`.
- [ ] Add unit tests verifying parity against the Python baseline outputs.

### Phase 2: Desktop App (Tauri v2 for Linux & Windows)
- [ ] Initialize `crates/voicekey-desktop` using Tauri v2.
- [ ] **Audio Capture:** Integrate `cpal` to record microphone input natively (WASAPI on Windows, PipeWire/ALSA on Linux).
- [ ] **Global Hotkeys:** Setup `tauri-plugin-global-hotkey` for push-to-talk (`Super+Space` / `Ctrl+Alt+V`).
- [ ] **Keystroke Simulation & Auto-Paste:**
  - Linux: Port `/dev/uinput` injector (`evdev`) and `wl-copy`.
  - Windows: Implement `SendInput` virtual keystroke synthesis and Win32 clipboard dispatch.
- [ ] **UI Layer:**
  - Build minimal, translucent floating mic pill (waveform animation when recording).
  - System Tray menu for fast model switching, settings, and quitting.
  - Settings window for mic selection, hotkey customization, and custom prompt dictionary.
- [ ] **Packaging:**
  - Windows: Build self-contained `.msi` and `.exe` installers via `cargo tauri build`.
  - Linux: Build `.deb`, `.rpm`, and AppImage.

### Phase 3: Android IME App (`voicekey-android`)
- [ ] **Rust-Kotlin Bridge:**
  - Setup `uniffi-rs` on `voicekey-core` to generate Kotlin bindings.
  - Compile `voicekey-core` for `aarch64-linux-android` using the Android NDK.
- [ ] **Android Native Project:**
  - Create Android Studio project (`android/`) targeting Android 10+ (API level 29+).
  - Implement `InputMethodService` (Custom Voice Keyboard).
- [ ] **Audio Capture:**
  - Setup Android `AudioRecord` at 16kHz mono, streaming chunks directly into the Rust ring buffer.
- [ ] **Direct Text Insertion:**
  - Pass audio to Rust `transcribe()`.
  - Insert text into the target app via `currentInputConnection.commitText(result, 1)`.
- [ ] **Model Management & UI:**
  - Jetpack Compose settings screen for downloading Whisper models (`base.en`, `tiny.en`) into the app's internal cache directory.

---

## 4. Key Decisions & Rationale

| Decision | Chosen Solution | Alternative Considered | Why |
| :--- | :--- | :--- | :--- |
| **Desktop Framework** | **Tauri v2 + Rust** | Electron or Pure Rust (Iced/Slint) | Electron is too heavy (150MB+ RAM); pure Rust GUI frameworks lack polished cross-platform web styling and easy tray integrations. |
| **Android Framework** | **Rust + Kotlin (IME)** | Tauri Mobile / Flutter / React Native | An Android voice tool must be an `InputMethodService` (keyboard) to inject text into all apps without accessibility restrictions. Webview wrappers are too sluggish and memory-heavy for an IME. |
| **Whisper Runtime** | **`whisper.cpp` (`whisper-rs`)** | ONNX Runtime or Candle | `whisper.cpp` has the best optimized C/C++ SIMD kernels for both x86 (AVX2) and ARM64 (NEON), with the smallest static binary size and lowest RAM footprint. |
| **FFI Bridge for Android** | **UniFFI (`uniffi-rs`)** | Manual JNI (`jni-rs`) | Mozilla's UniFFI eliminates hand-written C/JNI boilerplate and guarantees memory-safe bindings between Rust and Kotlin. |
