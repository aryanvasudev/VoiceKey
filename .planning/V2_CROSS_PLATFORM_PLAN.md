# VoiceKey v2 Master Roadmap & Implementation Plan: Cross-Platform (Linux, Windows, Android)

This document defines the comprehensive engineering blueprint to evolve VoiceKey from the Milestone 1 Linux Python prototype into a high-performance, native cross-platform ecosystem (Linux, Windows, Android).

---

## 1. High-Level System Architecture

```
                               ┌────────────────────────────────────────────────────────┐
                               │                    voicekey-core                       │
                               │               (Pure Rust Shared Crate)                 │
                               │                                                        │
                               │  • whisper-rs (whisper.cpp GGML/GGUF runtime)          │
                               │  • Resampling (rubato) -> 16kHz mono normalization    │
                               │  • Audio ring buffer & 200ms pre-roll memory           │
                               │  • Dual VAD (energy gating + silence detection)        │
                               │  • Vocabulary biasing & initial prompt conditioning    │
                               │  • Text post-processor (formatting, spoken punct)      │
                               │  • Abstract State Machine (Idle, Record, Process)      │
                               └───────────────────────────┬────────────────────────────┘
                                                           │
                        ┌──────────────────────────────────┴──────────────────────────────────┐
                        │                                                                     │
                        ▼                                                                     ▼
     ┌──────────────────────────────────────┐                              ┌──────────────────────────────────────┐
     │           voicekey-desktop           │                              │           voicekey-android           │
     │          (Tauri v2 + Rust)           │                              │            (Kotlin + JNI)            │
     │                                      │                              │                                      │
     │  Targets: Linux (Wayland/X11) & Win  │                              │  Target: Android 10+ (API 29+, ARM64)│
     │  UI: Svelte / React + Tailwind CSS   │                              │  UI: Jetpack Compose                 │
     │  • Floating recording pill (waveform)│                              │  • Custom Voice Keyboard (IME)       │
     │  • Native System Tray & menu         │                              │  • One-tap mic overlay on IME bar    │
     │  • cpal cross-platform audio capture │                              │  • AudioRecord 16kHz streaming       │
     │  • 3-tier Hotkey Router:             │                              │  • Foreground Service (mic privacy)  │
     │    - Win: WH_KEYBOARD_LL / Register  │                              │  • Direct text commit:               │
     │    - Wayland: XDG Portal / CLI socket│                              │    InputConnection.commitText()      │
     │    - X11: tauri-plugin-global-hotkey │                              │  • UniFFI-generated Kotlin bindings  │
     │  • Auto-Paste & Clipboard Restore:   │                              │  • In-app GGML Model Downloader      │
     │    - Win: SendInput + arboard        │                              │                                      │
     │    - Linux: /dev/uinput or wl-copy   │                              │                                      │
     │  • Model Downloader & Verification   │                              │                                      │
     └──────────────────────────────────────┘                              └──────────────────────────────────────┘
```

---

## 2. Critical Loophole Analysis & Engineering Mitigations

A review of naive cross-platform assumptions revealed several critical edge cases. The following mitigations are engineered into this plan:

### 2.1 The Wayland Global Hotkey Loophole
* **Loophole:** On Wayland, ordinary application windows cannot passively listen for global key presses due to compositor security isolation. Standard crates like `tauri-plugin-global-hotkey` rely on X11 and fail silently under native Wayland.
* **Mitigation:** A **3-tier hotkey architecture**:
  1. *Wayland Tier 1:* Use the **XDG Desktop Portal Global Shortcuts API** (`org.freedesktop.portal.GlobalShortcuts`), supported in modern GNOME (44+), KDE Plasma (5.27+), and wlroots/Hyprland.
  2. *Wayland Tier 2 (Universal Fallback):* Persistent CLI trigger (`voicekey-desktop toggle`) that hooks into native desktop environment shortcut settings (e.g. GNOME Settings, Sway/Hyprland config) via IPC socket.
  3. *Windows & X11:* Win32 low-level hooks / `tauri-plugin-global-hotkey`.

### 2.2 Push-to-Talk (Hold-to-Talk) vs Toggle Detection
* **Loophole:** Most OS-level global hotkey APIs (`RegisterHotKey` on Windows, Portal shortcuts on Linux) only notify on *key-down*. They do not emit *key-up* release events, making "Push-to-Talk" (hold key to speak, release to paste) impossible through standard hotkey listeners.
* **Mitigation:**
  - **Windows:** Implement a dedicated worker thread with a low-level keyboard hook (`WH_KEYBOARD_LL`) and message pump to track key-up and key-down separately.
  - **Linux:** Default to **Toggle Mode** (press once to start, press again to finish) as the universally reliable standard across all compositors, with hold-to-talk enabled when direct device access (`/dev/input`) is available.

### 2.3 Audio Hardware Sample Rate & Format Mismatch
* **Loophole:** `whisper.cpp` requires strictly 16,000 Hz, 1-channel, 32-bit float PCM. Audio input hardware (especially USB microphones and Bluetooth headsets) defaults to 44,100 Hz or 48,000 Hz stereo. `cpal` does not resample audio automatically.
* **Mitigation:** Integrate `rubato` (fast, high-quality SIMD resampler) and an automatic stereo-to-mono downmixer into `voicekey-core`. Handle dynamic audio device switching (e.g., unplugging a headset) gracefully without crashing the recording stream.

### 2.4 Clipboard Clashing & Restoration Race Conditions
* **Loophole:** Auto-paste copies transcribed text to the clipboard and sends `Ctrl+V`. This overwrites whatever the user previously had copied. Furthermore, if VoiceKey immediately restores the original clipboard, target applications (Chrome, VSCode, Slack) may not have read the new text yet, resulting in the old clipboard being pasted instead.
* **Mitigation:**
  - Provide a configurable "Preserve Clipboard" setting.
  - Implement a 250ms debounced asynchronous restore queue:
    1. Read and save existing clipboard content.
    2. Write transcribed text to clipboard.
    3. Inject `Ctrl+V` (or `Ctrl+Shift+V` if a terminal emulator is active).
    4. Wait 250ms for target app event loop consumption.
    5. Restore original clipboard content.

### 2.5 Windows UIPI (User Interface Privilege Isolation)
* **Loophole:** On Windows, an unelevated background app cannot inject `SendInput` keystrokes into elevated admin windows (e.g., Task Manager, elevated terminal).
* **Mitigation:** Graceful detection. If injection is rejected by UIPI, VoiceKey copies the text to the clipboard and displays a subtle notification pill: *"Text copied to clipboard (press Ctrl+V to paste into admin window)"*.

### 2.6 Android Background Microphone & IME Lifecycle
* **Loophole:** On Android 10+ (API 29+), background microphone recording is restricted and can trigger security exceptions if the keyboard UI is dismissed or the user switches apps mid-dictation.
* **Mitigation:** Run the audio recording session bound to a Foreground Service with `foregroundServiceType="microphone"` when active. Direct text insertion uses `InputConnection.commitText(result, 1)` directly from the IME session, completely bypassing clipboard permissions.

### 2.7 Model Delivery, Storage, & Memory Footprint
* **Loophole:** Bundling 100MB-300MB Whisper model weights directly inside app installers bloats downloads and prevents easy upgrades or custom models.
* **Mitigation:**
  - Lightweight initial installer (< 15MB).
  - Built-in Model Manager that downloads quantized GGML models (`tiny.en` Q5_0 ~31MB, `base.en` Q4_0 ~45MB, `small.en` Q5_0 ~150MB) on first run from verified mirror sources with SHA-256 integrity verification.
  - Storage in standard OS cache paths (`%APPDATA%` on Windows, `~/.local/share` on Linux, `context.filesDir` on Android).

---

## 3. Phase-Wise Implementation Plan

### Phase 1: Shared Rust Engine (`voicekey-core`)
- [x] Initialize Cargo workspace with `crates/voicekey-core`.
- [x] Implement audio DSP pipeline:
  - 16kHz resampler via `rubato`.
  - Stereo-to-mono downmixer.
  - Ring buffer with 200ms pre-roll memory to prevent missing initial consonants.
- [x] Integrate `whisper-rs` linking `whisper.cpp`:
  - Support Quantized GGML models (`Q4_0`, `Q5_0`, `Q8_0`).
  - CPU inference with AVX2/FMA (x86_64) and ARM NEON (aarch64).
- [x] Port text formatting and vocabulary biasing:
  - Custom prompt dictionary (`initial_prompt`).
  - Spoken punctuation parsing ("comma", "period", "new line").
  - Whitespace and capitalization cleanup.
- [x] Unit & benchmark test suite comparing outputs against the Python v1 baseline.

### Phase 2: Headless CLI & Cross-Platform Audio Capture
- [x] Build `crates/voicekey-cli` for standalone verification without GUI.
- [x] Audio capture pipeline using `cpal`:
  - Linux: PipeWire / PulseAudio / ALSA.
  - Windows: WASAPI stream.
  - Dynamic device reconnect listener (handling mic unplug/replug).
- [x] End-to-end headless CLI commands:
  - `voicekey-cli record --duration 5 --output test.wav`
  - `voicekey-cli transcribe test.wav --model base.en`

### Phase 3: Cross-Platform Input Injection & Hotkey System
- [x] **Keystroke Simulation & Auto-Paste:**
  - Windows: Win32 `SendInput` hardware scancode injection (`VK_CONTROL` + `VK_V`).
  - Linux: `/dev/uinput` virtual device (`evdev-rs`) with fallback to `wl-copy`.
  - Terminal awareness: Auto-detect terminal windows to emit `Ctrl+Shift+V`.
  - Asynchronous clipboard backup and restore queue.
- [x] **Hotkey Engines:**
  - Windows: Dedicated thread with `WH_KEYBOARD_LL` (supporting both push-to-talk and toggle).
  - Linux: XDG Desktop Portal Global Shortcuts client with IPC socket fallback for desktop environment shortcuts (`gsettings`, `sway`, `hyprland`).

### Phase 4: Tauri v2 Desktop Shell & System Tray
- [ ] Initialize `crates/voicekey-desktop` using Tauri v2.
- [ ] Connect Tauri backend actors to `voicekey-core` state machine:
  - States: `Idle` -> `Recording` -> `Transcribing` -> `Pasting` -> `Idle`.
- [ ] Native System Tray menu:
  - Visual status icon (green idle, red recording, yellow processing).
  - Quick model selector (`tiny.en`, `base.en`, `small.en`).
  - Mute/unmute microphone.
  - Settings and Quit buttons.
- [ ] Autostart integration via `tauri-plugin-autostart` (Windows Registry & Linux `.desktop` / systemd).

### Phase 5: Floating Overlay Pill, Settings UI & Model Manager
- [ ] Lightweight web frontend (Svelte or React + Tailwind CSS).
- [ ] **Floating Recording Pill:**
  - Frameless, transparent, draggable window.
  - Click-through capable when inactive.
  - Real-time audio waveform animation while recording.
- [ ] **Settings Window:**
  - Audio input device selector.
  - Hotkey configuration and mode (Toggle vs Hold-to-Talk).
  - Custom vocabulary dictionary for technical jargon.
  - Paste options (Instant Paste, Clipboard Only, Preserve Previous Clipboard).
- [ ] **Model Manager:**
  - In-app downloader with progress indicators and SHA-256 validation.
  - Automatic hardware benchmark to recommend optimal model.

### Phase 6: Desktop Packaging & Multi-Platform CI/CD
- [ ] **Windows Packaging:**
  - Self-contained `.msi` and NSIS `.exe` installers via `cargo tauri build`.
  - Auto-update support via `tauri-plugin-updater`.
- [ ] **Linux Packaging:**
  - Standalone AppImage, `.deb`, and `.rpm`.
  - Udev rule installer script for non-root `/dev/uinput` access.
- [ ] GitHub Actions CI workflow for automated cross-compilation of Windows and Linux binaries.

### Phase 7: Android Core FFI (`uniffi-rs`)
- [ ] Configure `uniffi-rs` interface definition for `voicekey-core`.
- [ ] Cross-compile `voicekey-core` for `aarch64-linux-android` using Android NDK (`cargo-ndk`).
- [ ] Generate Kotlin bindings and package as an Android `.aar` library.
- [ ] Unit test Kotlin wrapper with mock audio input.

### Phase 8: Android IME & Audio Capture
- [ ] Initialize Android Studio project (`android/`) targeting Android 10+ (API 29+).
- [ ] Implement `InputMethodService` (VoiceKey Keyboard):
  - Accessible from any app via the system keyboard switcher.
  - Clean keyboard toolbar with one-tap mic trigger.
- [ ] Audio recording via Android `AudioRecord`:
  - 16kHz 16-bit mono streaming buffer piped into Rust core.
  - Foreground service notification during recording to respect Android privacy indicators.
- [ ] Text insertion via `currentInputConnection.commitText(transcription, 1)`.

### Phase 9: Android Settings, Model Manager & Distribution
- [ ] Jetpack Compose settings screen:
  - Model download manager (saving `tiny.en` Q5_0 and `base.en` Q4_0 in app storage).
  - Custom vocabulary input.
  - Haptic feedback settings on record/finish.
- [ ] Release signing and packaging:
  - Production APK and AAB builds.
  - Metadata for GitHub Releases and F-Droid packaging.

---

## 4. Key Architectural Decisions Matrix

| Component | Selected Approach | Alternative Rejected | Rationale |
| :--- | :--- | :--- | :--- |
| **Shared Core** | **Rust (`voicekey-core`)** | C++ or Go | Memory safety, zero-cost abstractions, seamless C FFI (`whisper.cpp`), and official UniFFI bindings for Kotlin. |
| **Desktop Shell** | **Tauri v2 + Rust** | Electron or Pure Rust (Iced/Slint) | Tauri delivers 10x smaller binaries (~15MB vs 150MB+ for Electron), uses native OS webviews, and runs Rust directly on the metal. |
| **Desktop Hotkeys (Wayland)** | **XDG Portal + CLI Socket** | `tauri-plugin-global-hotkey` alone | Wayland security blocks global key listening for normal apps. Portal + IPC fallback ensures 100% compatibility across GNOME, KDE, and wlroots. |
| **Audio Resampling** | **`rubato` in Rust** | System resamplers (Pulse/WASAPI) | Guarantees hardware-independent 16kHz mono audio fed into Whisper regardless of the user's mic settings. |
| **Inference Engine** | **`whisper.cpp` (`whisper-rs`)** | PyTorch / ONNX / Candle | Smallest binary size, AVX2/NEON SIMD optimizations, quantized GGML/GGUF models with minimal RAM footprint (< 100MB). |
| **Android Integration** | **Kotlin IME (`InputMethodService`) + UniFFI** | Tauri Mobile / React Native | An IME is the only native mechanism on Android that can inject text into *any* app without Accessibility Service permission overhead. |
