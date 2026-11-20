# VoiceKey v2 Architecture & Migration Plan: Python to Tauri + Rust

This plan defines the strategy for transitioning VoiceKey from the Python prototype (Milestone 1) to a native, cross-platform desktop application using **Tauri v2 + Rust** supporting both **Linux (Wayland & X11)** and **Windows 10/11**.

---

## 1. What Changes Between Linux & Windows? (And What Stays the Same?)

The good news: **about 75-80% of the Rust codebase is identical across Linux and Windows** thanks to cross-platform Rust crates. Only 20-25% requires OS-conditional compilation (`#[cfg(target_os = "windows")]` vs `#[cfg(target_os = "linux")]`).

### Comparison Matrix

| Component | Shared / Cross-Platform | Linux Implementation | Windows Implementation |
| :--- | :--- | :--- | :--- |
| **Audio Input** | High-level ring buffer & float-to-int16 conversion (`cpal`) | ALSA / PulseAudio / PipeWire | WASAPI (Windows Audio Session API) |
| **Whisper Inference** | `whisper-rs` (`whisper.cpp` C++ engine linked to Rust) | AVX2 / OpenBLAS / Vulkan | AVX2 / OpenBLAS / DirectCompute / CUDA |
| **Global Hotkey** | Abstract hotkey listener thread | Desktop shortcuts (`gsettings` or X11 grab) or `global-hotkey` | Win32 `RegisterHotKey` / low-level hook (`WH_KEYBOARD_LL`) |
| **Clipboard Copy** | Cross-platform clipboard crate (`arboard`) | `wl-copy` / X11 clipboards | Win32 `OpenClipboard` / `SetClipboardData` |
| **Virtual Paste (`Ctrl+V`)**| Abstract input simulation | `/dev/uinput` or Wayland virtual keyboard protocol | Win32 `SendInput` (hardware virtual scancodes) |
| **UI & System Tray** | Tauri v2 Tray & floating recording pill | WebKitGTK Webview | WebView2 (Microsoft Edge Chromium engine) |
| **Autostart** | Tauri autostart plugin (`tauri-plugin-autostart`) | Systemd user service / XDG autostart `.desktop` | Windows Registry (`HKCU\...\Run`) |

---

## 2. Target Architecture (Tauri v2 + Rust)

```
┌────────────────────────────────────────────────────────────────────────┐
│                        TAURI FRONTEND (UI)                            │
│  • Framework: Svelte or React + TailwindCSS                            │
│  • Floating Recording Pill: Draggable, transparent, waveform animation │
│  • System Tray Menu: Quick model switch, mute, settings                │
│  • Settings Window: Hotkey picker, mic selector, prompt dictionary     │
└───────────────────────────────────▲────────────────────────────────────┘
                                    │ Tauri IPC (Commands & Events)
┌───────────────────────────────────▼────────────────────────────────────┐
│                       RUST BACKEND (Core Engine)                       │
│                                                                        │
│  ┌─────────────────────────┐           ┌─────────────────────────────┐ │
│  │   Global Hotkey Engine  │           │     Audio Stream Capture    │ │
│  │  (tauri-plugin-global-  │           │      (cpal + ring buffer)   │ │
│  │         hotkey)         │           └──────────────┬──────────────┘ │
│  └────────────┬────────────┘                          │                │
│               │                                       │ Raw PCM Audio  │
│               ▼                                       ▼                │
│  ┌───────────────────────────────────────────────────────────────────┐ │
│  │              VoiceKey State Machine (Tokio Actor)                 │ │
│  │        IDLE  <───>  RECORDING  <───>  TRANSCRIBING  <───> PASTE   │ │
│  └────────────────────────────────────┬──────────────────────────────┘ │
│                                       │                                │
│                     ┌─────────────────┴─────────────────┐              │
│                     ▼                                   ▼              │
│       ┌───────────────────────────┐       ┌──────────────────────────┐ │
│       │   Whisper Inference Core  │       │ Auto-Paste Dispatcher    │ │
│       │  (whisper-rs / GGML / GGUF│       │ • Clipboard: arboard     │ │
│       │   with prompt conditioning│       │ • Simulation: enigo /    │ │
│       │   and text normalization) │       │   SendInput / uinput     │ │
│       └───────────────────────────┘       └──────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Migration Roadmap (Phases for v2)

### Phase 1: Tauri v2 Project Scaffolding & System Tray
- Initialize Tauri v2 workspace with Rust backend.
- Implement System Tray with status indicator (Idle, Recording, Transcribing).
- Transparent floating overlay window for the recording indicator (pill with pulsing mic).

### Phase 2: Cross-Platform Audio Capture (`cpal`)
- Port `audio.py` logic to Rust.
- Continuous audio capture on background thread using `cpal`.
- Ring buffer management with pre-roll buffer (captures the first 150ms before the hotkey was pressed).
- Audio chime feedback (native playback via `rodio`).

### Phase 3: Embedded Whisper Inference (`whisper-rs`)
- Link `whisper.cpp` via `whisper-rs`.
- Download/load GGML model weights (e.g., `ggml-base.en.bin`, `ggml-small.en.bin`) stored in OS user data directories (`%APPDATA%` on Windows, `~/.local/share` on Linux).
- Port prompt biasing and punctuation normalization from `transcriber.py`.

### Phase 4: Cross-Platform Keystroke Injection & Auto-Paste
- Clipboard copy using `arboard` (works seamlessly on both Windows and Linux).
- Keystroke injection:
  - **Windows:** Use `windows-rs` / `enigo` to call Win32 `SendInput` with virtual keys (`VK_CONTROL` + `VK_V`).
  - **Linux:** Keep the existing `/dev/uinput` mechanism or X11 `xtest` fallback.
- Restore previous clipboard contents after paste if enabled in settings.

### Phase 5: Global Hotkey Management
- Use `tauri-plugin-global-hotkey` to intercept user-defined key combinations (e.g. `Super+Space` or `Ctrl+Alt+V`) even when another application has focus.
- Support both **Hold-to-Talk** (push down to speak, release to paste) and **Toggle Mode** (press once to start, press again to stop).

### Phase 6: Packaging & Distribution
- **Windows:** Generate self-contained `.msi` and `.exe` installers via `cargo tauri build` with WebView2 auto-detection.
- **Linux:** Generate `.deb`, `.rpm`, and AppImage.
- Auto-updater setup via `tauri-plugin-updater`.
