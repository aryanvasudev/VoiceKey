# VoiceKey 🎙️⌨️

[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Platforms: Linux & Windows](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-blue.svg)](https://github.com/aryanvasudev/VoiceKey)
[![CI](https://github.com/aryanvasudev/VoiceKey/actions/workflows/ci.yml/badge.svg)](https://github.com/aryanvasudev/VoiceKey/actions/workflows/ci.yml)
[![Tests Passing](https://img.shields.io/badge/tests-58%20passing-brightgreen.svg)](https://github.com/aryanvasudev/VoiceKey)

> **Blazing fast, 100% offline voice dictation with kernel-level auto-paste for Linux and Windows.**

VoiceKey brings the seamless, distraction-free voice typing experience of tools like Superwhisper and Wispr Flow to your desktop. Built with **Rust** and **Tauri v2**, VoiceKey captures audio through low-latency system drivers, transcribes speech 100% locally using quantized GGML Whisper models with CPU SIMD vector acceleration (AVX2/FMA/NEON), and injects the transcribed text directly into your active window at the cursor.

---

## ⚡ Highlights

- **100% Offline & Private:** Zero audio or text ever leaves your machine. All speech processing runs locally using quantized GGML models (`whisper.cpp`).
- **Blazing Fast (<300ms typical):** Pre-warmed resident inference engine with AVX2/FMA acceleration loads models in under 150ms.
- **Hardware-Level Auto-Paste:**
  - **Linux (Wayland & X11):** Bypasses Wayland security boundaries cleanly by emitting virtual keypresses (`Ctrl+V` or `Ctrl+Shift+V`) via Linux kernel `/dev/uinput` (`evdev`).
  - **Windows:** Hardware key injection via native Win32 `SendInput` and low-level hook listeners.
  - **Debounced Clipboard Preservation:** Automatically backs up your clipboard, pastes the transcription, and restores previous contents seamlessly.
- **Floating Recording Pill & Overlay:** Transparent, draggable floating pill with real-time audio waveform visualization and dictation timer.
- **System Tray & Settings UI:** System tray controller with color-coded status icons (Green = Idle, Red = Recording, Yellow = Transcribing) and multi-tab settings for audio device selection, model downloader, hotkeys, and vocabulary biasing.
- **Headless CLI Companion (`voicekey-cli`):** Standalone diagnostic and scripting tool (`doctor`, `devices`, `transcribe`, `listen`, `paste`, `toggle`).

---

## 🏗️ Architecture

```
                                  +---------------------------------------+
                                  | Global Shortcut / Overlay Mic Trigger |
                                  |  (e.g., F9, Super+Z, or Hold-to-Talk) |
                                  +-------------------+-------------------+
                                                      |
                                                      v
                    +-------------------------------------------------------------------+
                    |                   Tauri v2 Desktop Shell (`voicekey-desktop`)     |
                    |  - Floating Recording Pill (Frameless transparent overlay)         |
                    |  - System Tray Controller (Idle / Recording / Processing states)  |
                    |  - Model Manager (Background downloader & SHA-256 verifier)       |
                    |  - Settings Window & IPC Router                                   |
                    +---------------------------------+---------------------------------+
                                                      |
                                                      v
+-------------------------------------------------------------------------------------------------------+
| Core Engine (`voicekey-core`)                                                                         |
|                                                                                                       |
|  1. Audio Capture (`cpal`): Low-latency PipeWire / ALSA on Linux; WASAPI on Windows                   |
|  2. DSP Pipeline: Rubato resampler (44.1/48kHz -> 16kHz mono) + Circular Ring Buffer with pre-roll    |
|  3. VAD & Energy Gating: RMS/Peak speech detection & leading/trailing silence trimmer                |
|  4. GGML Inference (`whisper-rs`): Native SIMD quantized Whisper (`tiny.en`, `base.en`, `small.en`)   |
|  5. Text Processing: Custom vocabulary biasing, spoken punctuation ("comma" -> ","), normalization   |
|  6. Direct Injection (`injector.rs`): Linux `/dev/uinput` + Win32 `SendInput` hardware keystrokes     |
+-------------------------------------------------------------------------------------------------------+
                                                      |
                                                      v
                                        +---------------------------+
                                        |    Active Window Cursor   |
                                        |   (Text pasted cleanly)   |
                                        +---------------------------+
```

---

## 📦 Releases & Downloads

Every tagged release (`v*`) triggers automated multi-platform builds in [GitHub Actions](https://github.com/aryanvasudev/VoiceKey/releases):

### Windows
| Package | Description |
| :--- | :--- |
| **`VoiceKey_<version>_x64-setup.exe`** | Self-contained NSIS installer with desktop shortcut and uninstaller. |
| **`VoiceKey_<version>_x64_en-US.msi`** | WiX Windows Installer package for enterprise deployment. |
| **`voicekey-cli-<tag>-windows-x86_64.zip`** | Portable standalone CLI binary (`voicekey-cli.exe`) for terminal usage. |

### Linux
| Package | Description |
| :--- | :--- |
| **`voicekey_<version>_amd64.AppImage`** | Universal standalone executable (double-click to run on any Linux distro). |
| **`voicekey_<version>_amd64.deb`** | Debian / Ubuntu native package with automatic dependency resolution. |
| **`voicekey-<version>-1.x86_64.rpm`** | Fedora / RHEL native package. |
| **`voicekey-cli-<tag>-linux-x86_64.tar.gz`** | Standalone headless CLI binary, udev rules, and desktop integration scripts. |

---

## 🚀 How to Install

### Option A: Using Prebuilt Release Packages (Recommended)
- **Windows:** Download and run `VoiceKey_<version>_x64-setup.exe`.
- **Linux (AppImage):** Download `voicekey_<version>_amd64.AppImage`, make it executable (`chmod +x voicekey*.AppImage`), and run it.
- **Linux (Debian/Ubuntu):** `sudo dpkg -i voicekey_<version>_amd64.deb`
- **Linux (Fedora/RHEL):** `sudo rpm -i voicekey-<version>-1.x86_64.rpm`

---

### Option B: Linux One-Command Desktop Installer Script
If you cloned the repository or extracted the release tarball, you can run the automated installer:

```bash
chmod +x scripts/install_linux_desktop.sh
./scripts/install_linux_desktop.sh
```

**What this script does:**
1. Compiles optimized release binaries (`voicekey-desktop` and `voicekey-cli`) if not already built.
2. Installs binaries into `~/.local/bin/`.
3. Installs application icons and registers the XDG desktop entry (`~/.local/share/applications/voicekey.desktop`).
4. Configures `/etc/udev/rules.d/99-voicekey-uinput.rules` for non-root `/dev/uinput` auto-paste permissions.
5. Installs the systemd user service (`~/.config/systemd/user/voicekey.service`) for optional background autostart.

---

### Option C: Building from Source with Cargo

#### 1. System Prerequisites
- **Ubuntu/Debian:**
  ```bash
  sudo apt-get update
  sudo apt-get install -y libasound2-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libssl-dev librsvg2-dev
  ```
- **Fedora/RHEL:**
  ```bash
  sudo dnf install -y alsa-lib-devel webkit2gtk4.1-devel libayatana-appindicator-gtk3 libayatana-appindicator-gtk3-devel libdbusmenu-gtk3 openssl-devel
  ```
- **Windows:**
  No extra C development libraries needed (WASAPI and Win32 `SendInput` are built into the Windows SDK).

#### 2. Build Binaries
```bash
# Clone the repository
git clone https://github.com/aryanvasudev/VoiceKey.git
cd VoiceKey

# Build headless CLI
cargo build --release -p voicekey-cli

# Build desktop application
cargo build --release -p voicekey-desktop

# (Optional) Build all distribution bundles (deb, rpm, appimage, nsis, msi)
cargo install tauri-cli --version "^2.0.0" --locked
cargo tauri build
```

---

## 🧪 How to Test VoiceKey

You can test VoiceKey immediately using either the headless CLI or the Desktop UI.

### 1. Test Hardware & Audio Configuration
Run the built-in diagnostic tool to verify microphones, CPU instruction sets, and Whisper models on your system:
```bash
cargo run -p voicekey-cli -- doctor
```
*Expected output:*
```
==============================================
         VoiceKey Hardware & Engine Doctor    
==============================================
1. Audio Subsystem & Microphones:
   ✓ Found audio input device(s)
2. CPU SIMD Acceleration Features:
   - AVX2:  ✓ Enabled
   - FMA:   ✓ Enabled
3. Quantized GGML Models on Disk:
   ✓ Found model(s): ggml-tiny.en.bin
Diagnosis Summary:
   ✓ Core DSP, VAD & inference engine compiled natively.
```

### 2. List Detected Audio Input Devices
```bash
cargo run -p voicekey-cli -- devices
```

### 3. Test Interactive Microphone Recording & Speech Transcription
Speak into your microphone for a quick live test:
```bash
cargo run -p voicekey-cli -- listen -m tiny.en
```
Press **Enter** to start recording, speak a sentence, and press **Enter** to stop and transcribe.

### 4. Test Auto-Paste Keystroke Injection
Test direct text injection into your active window (or terminal):
```bash
cargo run -p voicekey-cli -- paste "Hello from VoiceKey!"
```
To verify the clipboard content on Linux:
```bash
wl-paste
```

### 5. Launch the Desktop GUI & Overlay
To launch the full desktop application (System Tray + Floating Recording Pill + Settings window):
```bash
cargo run -p voicekey-desktop
```
- Click the tray icon to open **Settings & Models**.
- Download higher-accuracy models (`base.en` or `small.en`) directly within the UI.
- Use the floating pill to click and dictate or configure your preferred global hotkey.

### 6. Android Core FFI & AAR Packaging
To cross-compile the native Rust core libraries for Android (`arm64-v8a` and `x86_64`) using `cargo-ndk` and generate foreign language Kotlin bindings via UniFFI:
```bash
# Build native libraries (.so) and generate UniFFI Kotlin bindings
./scripts/build_android_native.sh arm64-v8a x86_64

# Package the Android Archive (.aar) library
./scripts/package_android_aar.sh
```
The resulting `target/voicekey-core.aar` contains:
- `jni/arm64-v8a/libvoicekey_android_ffi.so` & `jni/x86_64/libvoicekey_android_ffi.so`
- `classes.jar` & ProGuard rules preserving JNA bindings
- `AndroidManifest.xml`

### 7. Android Application (.apk) Packaging
To assemble the complete VoiceKey Android APK distribution bundle with native libraries and resources:
```bash
./scripts/package_android_apk.sh
```
The resulting `target/voicekey-app-release.apk` packages the `InputMethodService` keyboard, Settings Activity, `VoiceKeyModelManager` with SHA-256 verification, and native dual-ABI inference runtimes (`arm64-v8a` and `x86_64`).

### 8. Run Workspace Test Suite
Run the unit and integration tests across all workspace crates:
```bash
cargo test --workspace
```

---

## 📂 Project Structure

```
VoiceKey/
├── .github/workflows/        # CI test matrix & tagged release builder
│   ├── ci.yml                # Linux, Windows & Android FFI test matrix
│   └── release.yml           # Automated release asset packaging & publishing
├── android/                  # Android Gradle project
│   ├── app/                  # VoiceKey IME voice keyboard app, SettingsActivity & ModelManager
│   └── voicekey-core/        # AAR library module, Kotlin wrapper & tests
├── fastlane/                 # App Store & F-Droid distribution metadata
├── fdroid/                   # F-Droid build recipe & metadata
├── crates/
│   ├── voicekey-core/        # Shared cross-platform engine
│   │   ├── src/audio/        # Capture (cpal), downmixing, rubato resampler, buffer
│   │   ├── src/engine/       # whisper.cpp GGML quantized inference binding
│   │   ├── src/vad/          # Energy gating & silence trimming
│   │   ├── src/input/        # Linux /dev/uinput & Windows SendInput auto-paste
│   │   └── src/text/         # Normalizer, spoken punctuation, vocabulary biasing
│   ├── voicekey-android-ffi/ # UniFFI Android FFI cdylib & Kotlin bindgen
│   ├── voicekey-cli/         # Standalone headless CLI tool (doctor, devices, listen)
│   └── voicekey-desktop/     # Tauri v2 desktop shell
│       ├── src/              # Tokio actor state machine, system tray, models manager
│       ├── ui/               # Floating recording pill & settings web interface
│       └── tauri.conf.json   # Multi-platform bundle configuration
└── scripts/
    ├── build_android_native.sh   # Android NDK cross-compilation & bindgen script
    ├── package_android_aar.sh    # Production .aar library packaging script
    ├── package_android_apk.sh    # Production .apk application packaging script
    ├── cmake/                    # Wrapped Android CMake toolchains
    ├── 99-voicekey-uinput.rules  # Linux udev rule for non-root uinput access
    ├── voicekey.desktop          # XDG desktop application entry
    └── install_linux_desktop.sh  # Automated desktop installation script
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

