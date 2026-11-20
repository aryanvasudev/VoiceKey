# VoiceKey (whisper-flow for Linux)

A lightweight, distraction-free voice-to-text dictation tool for Linux (GNOME/Wayland).
Triggered with a global hotkey, it records speech, transcribes using `faster-whisper` (CTranslate2 INT8 on CPU), copies to clipboard via `wl-copy`, and auto-pastes directly into the active window using direct kernel `uinput` virtual keystroke injection.

## Tech Stack
- **Language / Runtime:** Python 3.12 managed via `uv`
- **Transcription Engine:** `faster-whisper` (CTranslate2 v4.9.0, AVX2 INT8 CPU inference)
- **Audio Capture:** `sounddevice` (PipeWire / ALSA 16kHz mono streaming ring buffer)
- **Wayland Clipboard & Paste:**
  - Clipboard: `wl-copy` (system `wl-clipboard`)
  - Auto-paste: Direct `/dev/uinput` virtual keyboard key injection (`evdev.UInput` sending `Ctrl+V`)
- **IPC Architecture:** Unix Domain Socket daemon (`voicekeyd`) + sub-5ms CLI client (`voicekey toggle`)
- **Hotkey Trigger:** Native GNOME custom keyboard shortcut (`gsettings`) bound to `voicekey toggle`
- **Feedback:** Audio cues (`pw-play` / `aplay` chimes) + Desktop notifications (`notify-send`)
- **Configuration:** TOML config in `~/.config/voicekey/config.toml`
