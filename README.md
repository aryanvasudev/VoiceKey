# VoiceKey 🎙️⌨️

> **Distraction-free, warm-model voice-to-text dictation with kernel-level auto-paste for Linux (GNOME on Wayland).**

VoiceKey brings the lightning-fast, seamless dictation experience of tools like Wispr Flow to modern Linux desktops. Triggered via a global keyboard shortcut, it records audio, transcribes speech locally using a pre-warmed INT8 model via `faster-whisper`, copies the text to your Wayland clipboard via `wl-copy`, and automatically simulates `Ctrl+V` into your active window via Linux kernel `/dev/uinput` device emulation.

---

## Key Features

- **Blazing Fast (<300ms typical latency):** Keeps `faster-whisper` permanently loaded in RAM so there is zero model warm-up penalty per dictation.
- **Native Wayland Auto-Paste:** Bypasses Wayland security restrictions by emitting simulated keypresses via Linux `/dev/uinput`.
- **Sub-5ms CLI Dispatch:** Lightning-fast socket IPC client (`voicekey toggle`) that GNOME keyboard shortcuts can trigger instantly.
- **Subtle Audio Feedback:** Non-blocking start/stop sound cues via PipeWire so you know exactly when speech is being recorded.
- **Configurable TOML Settings:** Human-readable configuration at `~/.config/voicekey/config.toml`.

---

## Architecture Overview

```
                                      +--------------------------+
                                      | GNOME Keyboard Shortcut  |
                                      | (e.g., Ctrl+Alt+Space)   |
                                      +-------------+------------+
                                                    |
                                                    v
                                      +--------------------------+
                                      |  CLI: `voicekey toggle`  |
                                      +-------------+------------+
                                                    | (Unix Domain Socket)
                                                    v
+-------------------------------------------------------------------------------+
| `voicekeyd` Background Daemon (Persistent in RAM)                             |
|                                                                               |
|  1. State Machine: IDLE <---> RECORDING                                       |
|  2. Audio Engine (`sounddevice`): Streams 16kHz mono audio into RAM buffer   |
|  3. Audio Chimes: Start beep (record began) / Stop chime (transcribing)      |
|  4. Transcriber (`faster-whisper`): Preloaded CTranslate2 INT8 model          |
|  5. Text Processing: Clean whitespace, filter hallucinations, format text      |
|  6. Output Dispatcher:                                                        |
|     - Copies text to system clipboard via `wl-copy`                           |
|     - Injects virtual `Ctrl+V` key via `/dev/uinput` (`evdev.UInput`)        |
|     - Emits desktop notification (`notify-send`)                              |
+-------------------------------------------------------------------------------+
```

---

## Installation & Setup

### Prerequisites
- Python 3.12+
- [`uv`](https://github.com/astral-sh/uv) (fast Python package manager)
- PipeWire / ALSA audio
- `wl-clipboard` (`wl-copy`)
- Write access to `/dev/uinput` (or membership in `input` group with udev rule)

### Clone & Install
```bash
git clone https://github.com/aryanvasudev/VoiceKey.git
cd VoiceKey
uv sync
```

---

## Configuration

VoiceKey automatically creates and reads `~/.config/voicekey/config.toml`:

```toml
[transcription]
backend = "faster-whisper"    # "faster-whisper", "whisper.cpp", "openai-whisper", "whisperx"
model_size = "base.en"        # "tiny.en", "base.en", "small.en", "distil-large-v3", etc.
device = "cpu"                # "cpu", "cuda", or "auto"
compute_type = "int8"         # "int8", "float16", "int4"
beam_size = 1                 # 1 for maximum speed (greedy search)
vad_filter = true             # filter out background silence
cpu_threads = 8               # CPU threads for inference
initial_prompt = ""           # Prompt context
hotwords = ["Kubernetes", "VoiceKey", "Wayland"] # Jargon / name biasing
spoken_punctuation = false    # Toggle: convert spoken words like "comma", "new line"
normalize_text = true         # Toggle: clean whitespace & capitalization

[audio]
sample_rate = 16000
channels = 1
sound_feedback = true         # Play subtle start/stop audio cues

[output]
mode = "auto_paste"           # "auto_paste" or "clipboard_only"
paste_delay_ms = 50           # Delay after wl-copy before emitting Ctrl+V
notify = true                 # Send desktop notification

[daemon]
socket_path = "~/.local/state/voicekey/daemon.sock"
```

To inspect hardware recommendations or auto-apply optimal settings:
```bash
# Analyze hardware and recommend optimal backend and model
voicekey doctor

# Automatically write recommended engine settings to config.toml
voicekey doctor --apply
```

To initialize or view the configuration:
```bash
# Print config path
voicekey config --path

# Initialize default configuration if not present
voicekey config --init

# Print active configuration
voicekey config --show
```

---

## CLI Usage

| Command | Description |
| :--- | :--- |
| `voicekey toggle` | Primary hotkey action: starts recording if idle, stops & transcribes if recording |
| `voicekey start` | Explicitly start dictation recording |
| `voicekey stop` | Explicitly stop recording and trigger transcription & paste |
| `voicekey test-audio` | Record audio sample to verify microphone input and sound cues |
| `voicekey doctor` | Probe CPU/GPU hardware and recommend optimal backend & model |
| `voicekey doctor --apply` | Automatically tune and write optimal settings to `config.toml` |
| `voicekey status` | Query current status of `voicekeyd` |
| `voicekey config` | View or initialize configuration |
| `voicekeyd` | Start the persistent background daemon |

---

## Testing

Run unit tests with pytest:
```bash
uv run pytest
```

---

## Roadmap

- [x] **Phase 1: Project Scaffolding & Configuration**
  - Setup `pyproject.toml`, console entry points (`voicekey`, `voicekeyd`).
  - Typed configuration manager (`config.py`) and TOML serializer/deserializer.
  - Comprehensive unit test suite.
- [x] **Phase 2: Audio Recording & Feedback Chimes**
  - Ring buffer audio recorder via `sounddevice` (16kHz mono float32 RAM buffer).
  - Low-latency non-blocking audio start/stop cues via `pw-play` with synthesized fallback.
  - Desktop notification integration via `notify-send`.
- [x] **Phase 3: Multi-Backend Transcriber Engine & Hardware Doctor**
  - Pluggable transcriber interface (`BaseTranscriber`).
  - Backends: `faster-whisper`, `whisper.cpp` (ultra-lightweight with on-demand build/download), `openai-whisper`, and `whisperx`.
  - Hardware profiler (`voicekey doctor`) with `--apply` auto-tuning.
  - Hotword vocabulary biasing & clean-state feature toggles (`spoken_punctuation`, `normalize_text`).
- [x] **Phase 4: Daemon IPC & CLI Client**
  - Unix Domain Socket server (`~/.local/state/voicekey/daemon.sock`) with automatic stale socket cleanup.
  - Sub-5ms CLI client dispatch for `voicekey toggle`, `voicekey status`, `voicekey start`, and `voicekey stop`.
  - Non-blocking threadpool transcription worker to ensure immediate IPC responsiveness.
- [ ] **Phase 5: Wayland Auto-Paste & Hotkey Integration**
  - Clipboard copy via `wl-copy` and `/dev/uinput` virtual `Ctrl+V`.
- [ ] **Phase 6: Systemd Autostart & End-to-End Verification**
