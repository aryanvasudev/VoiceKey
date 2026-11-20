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
model_size = "base.en"        # "tiny.en", "base.en", "small.en", or multilingual "base"
device = "cpu"                # "cpu"
compute_type = "int8"         # "int8" for lowest latency on i7-13650HX
beam_size = 1                 # 1 for maximum speed (greedy search)
vad_filter = true             # filter out background silence

[audio]
sample_rate = 16000
channels = 1
sound_feedback = true         # Play subtle start/stop audio cues

[output]
mode = "auto_paste"           # "auto_paste" or "clipboard_only"
paste_delay_ms = 50           # Small delay after wl-copy before emitting Ctrl+V
notify = true                 # Send desktop notification with snippet
```

### 2.2 Audio Capture & Chimes (`src/voicekey/audio.py`)
- Captures microphone input asynchronously using `sounddevice.InputStream(samplerate=16000, channels=1, dtype='int16')`.
- Buffers frames in memory during the recording state.
- Emits non-blocking audio feedback using `pw-play` with built-in system sound effects or synthesized click tones.

### 2.3 Transcriber Engine (`src/voicekey/transcriber.py`)
- Preloads `WhisperModel(model_size, device="cpu", compute_type="int8", cpu_threads=8)` at daemon startup.
- In-RAM model eliminates the ~2–3 second model load penalty on every dictation.
- Inference on Intel 13th Gen CPU takes ~100–250ms for a sentence.
- Normalizes output, strips leading/trailing blank artifacts, and applies capitalization.

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
