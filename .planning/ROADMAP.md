# VoiceKey Roadmap

## Milestone 1: MVP Core (Hotkey Dictation & Auto-Paste)

- [x] **Phase 1: Project Scaffolding & Configuration**
  - Setup `pyproject.toml`, console entry points (`voicekey`, `voicekeyd`).
  - Implement config manager (`~/.config/voicekey/config.toml`) with sensible defaults (`model="base.en"`, `device="cpu"`, `compute_type="int8"`).
- [x] **Phase 2: Audio Recording & Feedback Chimes**
  - Ring buffer audio recorder via `sounddevice` (16kHz 16-bit mono PCM).
  - Audio start/stop chimes using `pw-play` or synthesized tones.
  - Desktop notifications using `notify-send`.
- [ ] **Phase 3: Multi-Backend Transcriber Engine & Hardware Doctor**
  - Modular pluggable transcriber architecture (`BaseTranscriber` interface).
  - Backends: `faster-whisper` (default warm CTranslate2), `whisper.cpp` (ultra-lightweight GGML for low-spec CPUs/ARM/iGPUs), `openai-whisper` (official vanilla PyTorch), and `whisperx` (forced alignment & VAD).
  - Hardware profiler & recommender (`voicekey doctor`) to suggest optimal backend & model per device.
  - Vocabulary biasing / hotword prompting for jargon & programming terms.
  - Text post-processing (whitespace, punctuation, capitalization normalization).
- [ ] **Phase 4: Daemon IPC & CLI Client**
  - Unix Domain Socket server (`~/.local/state/voicekey/daemon.sock`).
  - Instant CLI client (`voicekey toggle`, `voicekey start`, `voicekey stop`, `voicekey status`, `voicekey doctor`).
- [ ] **Phase 5: Wayland Auto-Paste & Hotkey Integration**
  - Fast clipboard copy using `wl-copy`.
  - Automatic `Ctrl+V` key injection via `/dev/uinput` (`evdev.UInput`).
  - GNOME shortcut binding script (`gsettings`).
- [ ] **Phase 6: Verification & Systemd Autostart**
  - End-to-end integration test (speak -> hotkey -> auto-paste).
  - Generate systemd user unit (`~/.config/systemd/user/voicekey.service`).

