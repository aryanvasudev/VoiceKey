# Detailed Implementation Plan: VoiceKey

**Goal:** Create a lightweight, distraction-free hotkey-driven speech-to-text dictation system for Linux (GNOME on Wayland) using `faster-whisper`.

---

## 1. Architecture Overview

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

## 2. Component Specifications

### 2.1 Configuration (`src/voicekey/config.py`)
File: `~/.config/voicekey/config.toml`
```toml
[transcription]
backend = "faster-whisper"    # "faster-whisper", "whisper.cpp", "openai-whisper", "whisperx"
model_size = "base.en"        # "tiny.en", "base.en", "small.en", "distil-large-v3", or multilingual
device = "cpu"                # "cpu", "cuda", or "auto"
compute_type = "int8"         # "int8", "float16", "int4", etc.
beam_size = 1                 # 1 for maximum speed (greedy search)
vad_filter = true             # filter out background silence
initial_prompt = ""           # Vocabulary biasing for technical jargon/names
cpu_threads = 8               # CPU threads for inference

[audio]
sample_rate = 16000
channels = 1
sound_feedback = true         # Play subtle start/stop audio cues

[output]
mode = "auto_paste"           # "auto_paste" or "clipboard_only"
paste_delay_ms = 50           # Small delay after wl-copy before emitting Ctrl+V
notify = true                 # Send desktop notification with snippet
```

### 2.2 Audio Capture & Chimes (`src/voicekey/audio.py` & `src/voicekey/feedback.py`)
- Captures microphone input asynchronously using `sounddevice.InputStream(samplerate=16000, channels=1, dtype='float32')`.
- Buffers frames in memory during recording into 1D float32 normalized array.
- Emits non-blocking audio cues via PipeWire (`pw-play`) with synthesized fallback tones.

### 2.3 Multi-Backend Transcriber Engine & Hardware Doctor (`src/voicekey/transcriber/` & `src/voicekey/doctor.py`)
- **Pluggable Architecture (`BaseTranscriber`):**
  - Standardized interface: `is_available()`, `initialize()`, `transcribe(audio: np.ndarray) -> str`, `unload()`.
  - **`faster-whisper` backend:** Prewarmed CTranslate2 INT8 model in RAM. High speed and low memory on modern x86/ARM CPUs and NVIDIA GPUs.
  - **`whisper.cpp` backend:** Ultra-lightweight GGML C++ engine for low-spec machines, laptops without dGPUs, Raspberry Pi, and Intel iGPUs with zero PyTorch bloat.
  - **`openai-whisper` backend:** Official reference PyTorch Whisper implementation.
  - **`whisperX` backend:** Enhanced forced alignment and phoneme-level timing.
- **Hardware Doctor (`voicekey doctor`):**
  - Probes system CPU capabilities (AVX2, AVX-512, NEON), RAM size, and GPU availability (NVIDIA CUDA via nvidia-smi, AMD ROCm, Intel Arc/iGPU).
  - Automatically assesses the machine and recommends the optimal backend, model size, and compute type.
- **Vocabulary & Hotword Biasing:**
  - Passes user-defined technical vocabulary / coding jargon in `initial_prompt` to guide transcription accuracy.
- **Text Post-Processing (`src/voicekey/postprocess.py`):**
  - Normalizes whitespace, strips repetitive hallucinations, and applies consistent capitalization and punctuation.

### 2.4 Wayland Clipboard & Paste Injection (`src/voicekey/paste.py`)
- **Clipboard:** Calls `wl-copy` with the transcribed text.
- **Auto-Paste:** Creates a virtual input device via Python `evdev.UInput()` connected directly to `/dev/uinput`.
- Injects `ecodes.KEY_LEFTCTRL` + `ecodes.KEY_V` with a 20ms debounce.
- Fallback: If `uinput` is inaccessible, copies to clipboard and informs the user to paste.

### 2.5 Daemon & IPC Server (`src/voicekey/daemon.py` & `src/voicekey/cli.py`)
- Daemon runs as an `asyncio` server listening on `~/.local/state/voicekey/daemon.sock`.
- Commands:
  - `toggle`: Starts recording if idle; stops and transcribes if recording.
  - `start`: Explicit start.
  - `stop`: Explicit stop & transcribe.
  - `status`: Returns JSON status (`idle`, `recording`, current model, etc.).
  - `config`: View or update settings.
- CLI execution takes < 5ms to send command and return.

### 2.6 System Integration (`scripts/install_hotkey.sh` & systemd)
- Automated helper script to register a GNOME custom keyboard shortcut via `gsettings`:
  - Name: `VoiceKey Dictation`
  - Command: `voicekey toggle`
  - Binding: `Ctrl+Alt+Space` (or user's choice)
- Generates systemd user service file: `~/.config/systemd/user/voicekey.service`.

---

## 3. Directory Layout

```
VoiceKey/
├── .planning/
│   ├── PROJECT.md
│   ├── ROADMAP.md
│   └── PLAN.md
├── pyproject.toml
├── README.md
├── src/
│   └── voicekey/
│       ├── __init__.py
│       ├── cli.py             # Entrypoint: `voicekey`
│       ├── daemon.py          # Entrypoint: `voicekeyd`
│       ├── config.py          # Config management
│       ├── audio.py           # Audio recorder & sound cues
│       ├── transcriber.py     # faster-whisper CTranslate2 worker
│       ├── paste.py           # wl-copy + uinput key injection
│       └── ipc.py             # Unix domain socket protocol
└── scripts/
    ├── setup_gnome_hotkey.sh  # Configures GNOME shortcut
    └── voicekey.service       # Systemd user service
```

---

## 4. Verification & Testing Steps

1. **Unit / Component Check:**
   - Test audio capture (`python -m voicekey.audio --test`).
   - Test `faster-whisper` INT8 transcription latency on a sample audio buffer.
   - Test `uinput` virtual `Ctrl+V` keypress on Wayland.
2. **Daemon IPC Check:**
   - Start daemon in foreground: `voicekeyd --debug`.
   - In another shell, run `voicekey toggle` -> speak -> run `voicekey toggle`.
   - Verify audio chime plays, text appears in clipboard and is pasted into focused cursor location.
3. **GNOME Shortcut Test:**
   - Press configured hotkey in text editor or browser search bar.
   - Confirm seamless speech-to-text workflow.
