# Phase 18: Wireless Remote Microphone (Android ↔ Desktop) Implementation Plan

## 1. Overview & Objective
Implement zero-cloud wireless remote microphone audio streaming and bidirectional dictionary synchronization between Android devices and Linux/Windows desktops. This enables users to speak into their phone as a high-fidelity wireless microphone that directly transcribes and auto-pastes into active desktop applications (IDEs, browsers, terminals) with sub-50ms network overhead.

---

## 2. Technical Contracts & Architecture

### 2.1 Network Topology
```
+-------------------------------------------------------------+
| Android App (VoiceKey Remote Mode)                          |
|  1. Discovers Desktop via mDNS or QR Code Scan              |
|  2. Authenticates via persistent revocable token            |
|  3. Captures 16kHz 16-bit mono PCM via AudioRecord          |
|  4. Streams binary WebSocket frames directly to Desktop     |
|  5. Displays live partial transcript feedback from Desktop   |
+------------------------------+------------------------------+
                               | Local Wi-Fi (WebSocket TCP)
                               v
+-------------------------------------------------------------+
| Desktop Shell (voicekey-desktop + voicekey-core)            |
|  1. mDNS Responder advertises `_voicekey._tcp.local`        |
|  2. WebSocket Server (tokio-tungstenite) accepts connections|
|  3. Validates token against `settings.json` whitelist       |
|  4. Injects remote PCM frames into StreamingTranscriber      |
|  5. Normalizes, processes with Micro-LLM (if enabled)        |
|  6. Injects keystrokes (Ctrl+V) into active desktop window  |
+-------------------------------------------------------------+
```

### 2.2 Protocol Specification
- **Control Framing:** JSON text WebSocket frames (`RemoteMessage`).
- **Audio Framing:** Binary WebSocket frames consisting of 16-bit signed little-endian PCM samples at 16kHz (e.g. 100ms chunk = 1600 samples = 3200 bytes).
- **Service Discovery:** mDNS service `_voicekey._tcp.local` on port `54321` with TXT records for hostname, version, and pairing state.
- **Pairing Scheme:**
  - mDNS 1-click Approval: Android sends `pair_request`, Desktop UI pops up confirmation, returns persistent secret token.
  - QR Code Fallback: Desktop Settings renders QR code containing `voicekey://pair?host=<IP>&port=54321&token=<PRE_SHARED_KEY>&name=<HOST>`.

---

## 3. Step-by-Step Execution Plan

### Task 1: Core Remote Protocol & Audio Adapter (`voicekey-core`)
- **Files to Modify/Create:**
  - `crates/voicekey-core/Cargo.toml`
  - `crates/voicekey-core/src/remote/mod.rs`
  - `crates/voicekey-core/src/remote/protocol.rs`
  - `crates/voicekey-core/src/lib.rs`
- **Implementation Details:**
  1. Define serializable enum `RemoteMessage`:
     - `PairRequest { device_id: String, device_name: String }`
     - `PairResponse { status: String, token: Option<String>, desktop_name: String }`
     - `Auth { device_id: String, token: String }`
     - `SyncDictionary { vocabulary: Vec<String> }`
     - `RecordControl { action: RecordAction }` where `RecordAction` is `Start`, `Stop`, `Cancel`.
     - `StateBroadcast { state: String, partial_transcript: String }`
  2. Implement helper `decode_pcm16_chunk(bytes: &[u8]) -> Result<Vec<f32>, CoreError>` to convert incoming raw network bytes to normalized `f32` samples.
  3. Implement `generate_pairing_token() -> String` generating 32-byte cryptographic hex tokens.
- **Verification:** Unit tests in `crates/voicekey-core/src/remote/mod.rs` testing JSON message serialization, byte-to-float PCM conversion, and token generation.

---

### Task 2: Desktop WebSocket Server & mDNS Advertiser (`voicekey-desktop`)
- **Files to Modify/Create:**
  - `crates/voicekey-desktop/Cargo.toml` (add `tokio-tungstenite`, `mdns-sd`)
  - `crates/voicekey-desktop/src/remote/server.rs`
  - `crates/voicekey-desktop/src/remote/mdns.rs`
  - `crates/voicekey-desktop/src/remote/mod.rs`
  - `crates/voicekey-desktop/src/settings.rs`
  - `crates/voicekey-desktop/src/main.rs`
- **Implementation Details:**
  1. Extend `SettingsPayload` with:
     - `remote_mic_enabled: bool` (default true)
     - `remote_port: u16` (default 54321)
     - `paired_devices: Vec<PairedDevice>` (`id`, `name`, `token`, `paired_at`)
  2. Implement `MdnsAdvertiser` advertising `_voicekey._tcp.local` on LAN.
  3. Implement `RemoteServer`:
     - Listens on `0.0.0.0:remote_port`.
     - Handles unauthenticated pairing requests by emitting Tauri event `remote-pair-request` to frontend.
     - Handles authenticated streaming sessions: forwards binary PCM frames into `voicekey_core` recorder/transcriber actor.
     - Broadcasts live speculative transcripts and actor state back down the WebSocket.
  4. Expose Tauri commands:
     - `get_remote_status() -> RemoteStatus` (active connections, port, IP)
     - `approve_remote_pair(device_id: String, approve: bool)`
     - `revoke_remote_device(device_id: String)`
- **Verification:** Integration test simulating client socket connecting, authenticating with token, and streaming audio.

---

### Task 3: Desktop UI: Pairing Approval, QR Code & Device Management
- **Files to Modify/Create:**
  - `crates/voicekey-desktop/ui/index.html`
- **Implementation Details:**
  1. Add "Remote Devices" tab to Settings modal:
     - Server status indicator (Running on port 54321 / Stopped).
     - QR Code generator card using lightweight inline JS/canvas for quick Android pairing.
     - List of paired mobile devices with "Revoke" button.
  2. Implement floating pairing prompt modal:
     - Appears when a new Android device requests pairing: `"Pixel 8 Pro wants to connect as a wireless microphone. [Allow / Deny]"`.
     - Clicking "Allow" generates token, stores it in settings, and returns `PairResponse::Accepted`.
- **Verification:** Manual and UI DOM verification of the pairing modal, settings tab, and revocation flow.

---

### Task 4: Android App Remote Microphone Mode (`android/`)
- **Files to Modify/Create:**
  - `android/app/build.gradle.kts` (add `okhttp` for WebSocket)
  - `android/app/src/main/AndroidManifest.xml` (permissions: `INTERNET`, `ACCESS_NETWORK_STATE`)
  - `android/app/src/main/java/com/voicekey/app/remote/VoiceKeyRemoteClient.kt`
  - `android/app/src/main/java/com/voicekey/app/remote/RemoteMicActivity.kt`
  - `android/app/src/main/res/layout/activity_remote_mic.xml`
- **Implementation Details:**
  1. Add `NsdManager` discovery listener in `VoiceKeyRemoteClient` discovering `_voicekey._tcp.local` services.
  2. Implement WebSocket client with automatic reconnection and token storage via `SharedPreferences`.
  3. Implement `RemoteMicActivity`:
     - Clean, distraction-free screen with large microphone button (supports Push-to-Talk and Toggle).
     - Haptic feedback on start/stop.
     - Live waveform and mirrored partial transcription card.
     - Auto-syncs custom vocabulary dictionary with desktop on connect.
- **Verification:** Unit tests for `VoiceKeyRemoteClient` message handling and connection lifecycle.

---

### Task 5: Headless CLI & End-to-End Cross-Platform Verification
- **Files to Modify/Create:**
  - `crates/voicekey-cli/src/main.rs`
  - `crates/voicekey-cli/tests/cli_tests.rs`
- **Implementation Details:**
  1. Add `voicekey-cli remote` subcommands:
     - `voicekey-cli remote listen [--port 54321]`
     - `voicekey-cli remote send --file audio.wav [--host 127.0.0.1:54321] [--token <key>]`
  2. Write integration tests in `cli_tests.rs`:
     - Start mock remote server.
     - Connect CLI client, authenticate, exchange dictionary, and stream audio chunks.
     - Verify received audio chunks match input and transcribe accurately.
  3. Validate full workspace: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check`.
- **Verification:** 100% green tests across Linux, Windows, and Android CI runners.

---

## 4. Definition of Done
1. Remote WebSocket server running seamlessly inside `voicekey-desktop` with zero performance impact on local hotkey dictation.
2. Android discovery via mDNS works out of the box with Desktop 1-click approval prompt.
3. QR code fallback works for networks blocking multicast.
4. Bidirectional custom dictionary sync synchronizes technical hotwords between Android and Desktop.
5. All workspace unit, integration, and cross-platform CI tests pass with 0 errors and 0 warnings.
