# VoiceKey 🎙️⌨️

[![Python 3.12+](https://img.shields.io/badge/python-3.12+-blue.svg)](https://www.python.org/downloads/)
[![Linux Wayland](https://img.shields.io/badge/platform-Linux%20Wayland-green.svg)](https://wayland.freedesktop.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests Passing](https://img.shields.io/badge/tests-passing-brightgreen.svg)](tests/)

> **Distraction-free, warm-model voice dictation with kernel-level auto-paste for modern Linux desktops (GNOME on Wayland).**

VoiceKey brings the lightning-fast, seamless voice typing experience of tools like Superwhisper and Wispr Flow to modern Linux desktops. Triggered by a global keyboard shortcut, it records audio, transcribes speech 100% locally using an in-RAM pre-warmed Whisper model, copies the result to your Wayland clipboard, and automatically types it into your active window via Linux kernel `/dev/uinput` emulation.

---

## ⚡ Highlights

- **Near-Zero Latency (<300ms typical):** Keeps the Whisper engine permanently warm in memory so there is zero cold-start delay per dictation.
- **Native Wayland Auto-Paste:** Bypasses Wayland's strict security boundaries cleanly by emitting virtual keypresses directly through Linux kernel `/dev/uinput`.
- **Sub-5ms CLI Dispatch:** High-performance Unix domain socket client (`voicekey toggle`) engineered for instant hotkey response.
- **Hardware Doctor & Auto-Tuning:** Built-in profiler probes your CPU (AVX-512/VNNI) or NVIDIA GPU (CUDA/VRAM) and automatically applies the optimal model and quantization.
- **Multi-Backend Speech Engine:** Pluggable architecture supporting `faster-whisper`, `whisper.cpp`, `openai-whisper`, and `whisperx`.
- **Subtle Audio Feedback:** Non-blocking start/stop sound cues via PipeWire so you know exactly when recording begins and ends.
- **Built-in Systemd Lifecycle:** Manage background autostart directly with `voicekey autostart --install`.

---

## 🏗️ Architecture

```
                                  +--------------------------+
                                  | Global Keyboard Shortcut |
                                  |  (e.g., Super+V or F9)   |
                                  +-------------+------------+
                                                |
                                                v
                                  +--------------------------+
                                  |  CLI: `voicekey toggle`  |
                                  +-------------+------------+
                                                | (Unix Domain Socket IPC)
                                                v
+-------------------------------------------------------------------------------+
| `voicekeyd` Background Daemon (Resident in RAM)                                |
|                                                                               |
|  1. State Machine: IDLE <---> RECORDING                                       |
|  2. Audio Engine (`sounddevice`): Streams 16 kHz mono audio into RAM buffer   |
|  3. PipeWire Chimes: Start cue (recording) / Stop cue (transcribing)          |
|  4. Transcriber (`faster-whisper`): Preloaded CTranslate2 INT8 model          |
|  5. Text Processing: Normalize whitespace, hallucination filtering, hotwords  |
|  6. Dispatcher:                                                               |
|     - Copies text to system clipboard via `wl-copy`                           |
|     - Injects virtual `Ctrl+V` keystroke via `/dev/uinput` (`evdev`)          |
|     - Emits desktop notification (`notify-send`)                              |
+-------------------------------------------------------------------------------+
                                                |
                                                v
                                  +--------------------------+
                                  |   Active Window Cursor   |
                                  |   (Text pasted cleanly)  |
                                  +--------------------------+
```

---

## 🚀 One-Command Install

VoiceKey comes with an all-in-one setup script. It automatically detects your package manager, installs system prerequisites (`wl-clipboard`, `pipewire`, `libevdev`), sets up `/dev/uinput` auto-paste permissions, auto-tunes the Whisper engine for your CPU/GPU, registers the global hotkey, and starts the systemd background daemon:

```bash
git clone https://github.com/aryanvasudev/VoiceKey.git
cd VoiceKey
./install.sh
```

*(Or to bind a custom hotkey instead of the default `Super+Z` and `F9`:)*
```bash
./install.sh --hotkey "<Super>v"
```

**That's it!** Press your hotkey (`Super+Z` or `F9`), speak, and press it again. Your transcription pastes directly at your cursor.

<details>
<summary><b>🛠️ Manual Step-by-Step Installation (Click to expand)</b></summary>

### 1. Install System Packages
```bash
# Fedora / RHEL
sudo dnf install wl-clipboard pipewire-utils libevdev libnotify

# Ubuntu / Debian
sudo apt-get install wl-clipboard pipewire-bin libevdev2 libnotify-bin

# Arch Linux
sudo pacman -S wl-clipboard pipewire-utils libevdev libnotify
```

### 2. Permissions for /dev/uinput Auto-Paste
```bash
sudo usermod -aG input $USER
echo 'KERNEL=="uinput", SUBSYSTEM=="misc", GROUP="input", MODE="0660", TAG+="uaccess", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### 3. Sync & Build Virtual Environment
```bash
git clone https://github.com/aryanvasudev/VoiceKey.git
cd VoiceKey
uv sync
```

### 4. Setup, Tune, and Start
```bash
# Run all-in-one internal setup command:
uv run voicekey setup
```

</details>

---

## ⚙️ Configuration

VoiceKey reads configuration from `~/.config/voicekey/config.toml`. It is created automatically with sensible defaults on first run, or by running `voicekey config --init`.

```toml
[transcription]
backend = "faster-whisper"      # "faster-whisper", "whisper.cpp", "openai-whisper", "whisperx"
model_size = "base.en"          # "tiny.en", "base.en", "small.en", "distil-large-v3", etc.
device = "cpu"                  # "cpu", "cuda", or "auto"
compute_type = "int8"           # "int8", "float16", "int4"
beam_size = 1                   # 1 for lowest latency (greedy search)
vad_filter = true               # Filter out background silence automatically
cpu_threads = 8                 # Number of threads for CPU inference
initial_prompt = ""             # Optional context prompt for Whisper
hotwords = ["VoiceKey", "Wayland", "Kubernetes"] # Vocabulary biasing
spoken_punctuation = false      # Convert spoken words ("comma", "new line") to symbols
normalize_text = true           # Auto-clean whitespace and capitalization

[audio]
sample_rate = 16000             # Standard Whisper 16 kHz sample rate
channels = 1                    # Mono
sound_feedback = true           # Play non-blocking start/stop audio cues

[output]
mode = "auto_paste"             # "auto_paste" or "clipboard_only"
paste_delay_ms = 50             # Delay in ms after wl-copy before injecting Ctrl+V
notify = true                   # Send desktop notification on completion

[daemon]
socket_path = "~/.local/state/voicekey/daemon.sock"
```

### Configuration Helper Commands

```bash
# Display the active configuration path
voicekey config --path

# Display the current loaded configuration
voicekey config --show

# Force-reinitialize default configuration
voicekey config --init
```

---

## 🩺 Hardware Profiler (`voicekey doctor`)

VoiceKey includes a hardware detection suite that analyzes your CPU vector extensions (AVX-512, VNNI, AVX2), GPU capability (CUDA compute capability and VRAM), memory headroom, and system permissions.

```bash
# Inspect system capabilities and recommendations
voicekey doctor

# Automatically write recommended engine settings to config.toml
voicekey doctor --apply
```

| Hardware Profile | Auto-Selected Backend | Default Model | Compute Type |
| :--- | :--- | :--- | :--- |
| **NVIDIA GPU (>= 6 GB VRAM)** | `faster-whisper` | `distil-large-v3` / `small.en` | `float16` |
| **Modern CPU (AVX-512 / VNNI)** | `faster-whisper` | `base.en` | `int8` |
| **Constrained CPU (Low RAM)** | `whisper.cpp` | `base.en` (Q5_1) | `int8` |

---

## ⌨️ Hotkey Setup Across Desktops

### GNOME (Automated)
Run the bundled script to bind the toggle action to global shortcuts (defaults to `Super+Z` and `F9`):
```bash
./scripts/setup_gnome_hotkey.sh
```

### Hyprland
Add to `~/.config/hypr/hyprland.conf`:
```ini
bind = SUPER, V, exec, voicekey toggle
```

### Sway
Add to `~/.config/sway/config`:
```ini
bindsym $mod+v exec voicekey toggle
```

### KDE Plasma (Wayland)
Go to **System Settings > Shortcuts > Custom Shortcuts > Edit > New > Global Shortcut > Command/URL**, set trigger to `Meta+V`, and command to:
```bash
voicekey toggle
```

---

## 💻 CLI Reference

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `voicekey setup` | `[--hotkey KEY]` | Automated setup: tune hardware, bind GNOME shortcut, start systemd service |
| `voicekey toggle` | — | Primary hotkey action: begins recording if idle, transcribes and pastes if recording |
| `voicekey start` | — | Explicitly start dictation recording |
| `voicekey stop` | — | Explicitly stop dictation recording and trigger transcription & paste |
| `voicekey status` | — | Query the active state of `voicekeyd` (IDLE, RECORDING, TRANSCRIBING) |
| `voicekey doctor` | `[--apply]` | Probe CPU/GPU capabilities and display (or apply) hardware recommendations |
| `voicekey config` | `[--init\|--path\|--show]` | Manage and inspect configuration settings |
| `voicekey autostart` | `--install\|--start\|--stop\|--status\|--remove` | Manage systemd user service lifecycle |
| `voicekey test-audio` | — | Record a brief sample to verify microphone capture and feedback chimes |
| `voicekey test-paste` | — | Copy test text and simulate virtual `Ctrl+V` through `/dev/uinput` |
| `voicekeyd` | — | Start the persistent background daemon directly in the foreground |

---

## 🛠️ Verification & Diagnostics

VoiceKey provides an end-to-end environment verification script that validates all dependencies, audio capture, PipeWire playback, desktop notifications, Wayland clipboard, uinput kernel access, and systemd units:

```bash
# Run comprehensive environment diagnostic
uv run python scripts/verify_installation.py
```

### Run Test Suite
```bash
# Run all unit and integration tests
uv run pytest
```

---

## 🧭 Planned Enhancements

- [ ] **System Tray & Status Indicator:** Visual indicator in GNOME TopBar / Waybar showing recording status.
- [ ] **Floating Waveform HUD:** Optional minimalist overlay showing real-time audio input levels.
- [ ] **Streaming Transcription:** Early partial transcription for long-form dictation.
- [ ] **Multi-Language Auto-Detection:** Switch languages on the fly without changing configuration.
- [ ] **LLM Post-Processing Filter:** Optional local LLM filter (via Ollama or llama.cpp) to rephrase notes or fix domain-specific phrasing.

---

## 📄 License

Distributed under the [MIT License](LICENSE). Built for high-efficiency, privacy-first local voice computing on Linux.
