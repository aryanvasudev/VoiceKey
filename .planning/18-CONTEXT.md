# Phase 18 Context: Wireless Remote Microphone (Android ↔ Desktop)

## 1. Executive Summary & Goals
Phase 18 extends VoiceKey's cross-platform capabilities by turning an Android mobile device into a high-performance, low-latency wireless microphone and remote dictation controller for Linux and Windows desktops over the local Wi-Fi network.

Key Objectives:
1. **Zero-Cloud Local Streaming:** Direct device-to-device communication with zero external cloud dependencies, keeping all audio strictly on the local network.
2. **Low Latency (<50ms):** Real-time 16kHz PCM audio streaming over local WebSocket, directly interfacing with `voicekey-core`'s `StreamingTranscriber` and `AdaptiveVad`.
3. **Secure Hybrid Discovery & Pairing:**
   - Primary: mDNS service beaconing (`_voicekey._tcp.local`) with a Desktop 1-click "Allow / Deny" authorization dialog.
   - Fallback: Scannable QR code generated in Desktop Settings containing IP, port, and pre-shared pairing token for networks with AP/multicast isolation.
   - Persistent authorization: Paired devices persist across restarts until explicitly revoked in Desktop Settings.
4. **Bidirectional Dictionary Synchronization:** Automatic seamless exchange and merging of custom technical vocabulary, jargon, and hotwords between Android and Desktop.

---

## 2. Architectural Decisions Matrix

| Decision Area | Selected Approach | Rationale |
| :--- | :--- | :--- |
| **Network Transport** | **Local WebSocket (TCP)** | Native bi-directional protocol supported across Rust (`tokio-tungstenite`) and Android (`OkHttp`). Supports binary framing for raw 16kHz 16-bit PCM chunks and JSON framing for control/sync messages. Zero packet loss and sub-10ms local Wi-Fi latency. |
| **Service Discovery** | **mDNS / DNS-SD (`_voicekey._tcp.local`)** | Native zero-configuration discovery via Android `NsdManager` and Rust mDNS responder. Devices find each other automatically without requiring IP lookups. |
| **Pairing & Authentication** | **Hybrid: mDNS Approval Prompt + QR Code Fallback** | User-friendly 1-click prompt on Desktop when Android requests pairing. QR code scan fallback covers restrictive corporate or guest networks where multicast/mDNS is blocked. |
| **Auth Lifespan** | **Persistent Pairing with Revocable Tokens** | Secure random token (256-bit hex) exchanged upon pairing. Saved in Desktop `settings.json` and Android `EncryptedSharedPreferences`. Can be revoked from Desktop UI anytime. |
| **Audio Format** | **16kHz 16-bit Mono PCM (Binary WebSocket Frames)** | Directly matches Whisper STT native input rate, eliminating CPU resampling overhead on both phone and desktop. |
| **Remote Control Model** | **Remote Trigger + Direct Paste** | Mic button on Android acts as a remote controller for Desktop dictation: starts Desktop recording/streaming overlay, feeds PCM samples, and auto-pastes text into active desktop window on completion. |
| **Dictionary Sync** | **Bidirectional Union Merge** | Upon connection handshake, devices exchange vocabulary lists and merge them without duplicates, ensuring hotwords added on either device are accessible everywhere. |

---

## 3. Communication Protocol Specification

### 3.1 mDNS Beacon
- Service type: `_voicekey._tcp.local.`
- Port: Default `54321` (configurable or dynamic fallback).
- TXT Records:
  - `name`: Desktop machine hostname (e.g. `Aryan-ArchLinux`).
  - `version`: `0.2.0`.
  - `pairing`: `open` | `paired`.

### 3.2 Handshake & Pairing JSON Messages
#### 1. Pairing Request (Android -> Desktop):
```json
{
  "type": "pair_request",
  "device_id": "android_98a71b4c",
  "device_name": "Pixel 8 Pro"
}
```

#### 2. Pairing Response (Desktop -> Android):
```json
{
  "type": "pair_response",
  "status": "accepted", // or "rejected"
  "token": "7f8b91c0e3a54d62...",
  "desktop_name": "Aryan-Workstation"
}
```

#### 3. Connection Handshake (Android -> Desktop):
```json
{
  "type": "auth",
  "device_id": "android_98a71b4c",
  "token": "7f8b91c0e3a54d62..."
}
```

#### 4. Dictionary Sync (Bidirectional):
```json
{
  "type": "sync_dictionary",
  "vocabulary": ["VoiceKey", "Tauri", "Tokio", "Wayland", "Whisper"]
}
```

### 3.3 Audio Streaming & Remote Commands
- **Start Dictation (Android -> Desktop):**
  `{"type": "record_start", "mode": "toggle"}`
- **Audio Chunks (Android -> Desktop):**
  Binary WebSocket frames containing raw little-endian PCM16 samples (100ms chunks = 3,200 bytes per frame at 16kHz mono).
- **Stop / Commit Dictation (Android -> Desktop):**
  `{"type": "record_stop"}`
- **Cancel Dictation (Android -> Desktop):**
  `{"type": "record_cancel"}`
- **Desktop Status Broadcast (Desktop -> Android):**
  `{"type": "state_update", "state": "recording" | "transcribing" | "idle", "partial_text": "..."}`

---

## 4. Work Breakdown & Implementation Phases

1. **`voicekey-core` Protocol & Server Primitives:**
   - Remote mic protocol schema and message types (`RemoteMessage`, `PairRequest`, `PairResponse`).
   - Pairing token generator and validator.
   - Remote audio source adapter feeding directly into `AudioRecorder` / `StreamingTranscriber`.
2. **`voicekey-desktop` Remote Server & Pairing UI:**
   - Embedded Tokio WebSocket server + mDNS responder.
   - Tauri dialog / UI alert for pairing requests ("Allow Pixel 8 to connect?").
   - Settings UI tab "Remote Devices": list paired devices, revoke tokens, and display QR code with IP/port/token.
   - Feed incoming binary PCM frames into desktop dictation pipeline and broadcast live partial transcripts back to Android.
3. **Android App Remote Mode (`android/`):**
   - mDNS discovery scanner via `NsdManager`.
   - Camera QR code scanner fallback (ZXing / CameraX).
   - "Remote Microphone" activity / drawer in VoiceKey Android app with a large Push-to-Talk / Toggle mic button, live waveform, and partial transcript display.
   - Two-way dictionary synchronization.
4. **Verification & Testing:**
   - Rust unit & integration tests simulating mock remote client connecting, pairing, syncing dictionary, and streaming PCM audio.
   - CLI remote streaming test command.
