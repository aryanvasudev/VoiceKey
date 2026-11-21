//! Integration tests for voicekey-cli headless tool.

use std::process::Command;
use tempfile::NamedTempFile;

fn get_cli_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_voicekey-cli"))
}

#[test]
fn test_cli_devices_command() {
    let bin = get_cli_bin();
    let output = Command::new(&bin)
        .arg("devices")
        .output()
        .expect("Failed to execute voicekey-cli devices");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Audio Input Devices"));
}

#[test]
fn test_cli_doctor_command() {
    let bin = get_cli_bin();
    let output = Command::new(&bin)
        .arg("doctor")
        .output()
        .expect("Failed to execute voicekey-cli doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VoiceKey Hardware & Engine Doctor"));
    assert!(stdout.contains("Audio Subsystem & Microphones"));
}

#[test]
fn test_cli_record_and_transcribe_e2e() {
    let bin = get_cli_bin();
    let temp_wav = NamedTempFile::new().unwrap();
    let wav_path = temp_wav.path().to_path_buf();

    // 1. Record 0.5s audio (if audio input device is available in environment)
    let rec_output = Command::new(&bin)
        .args([
            "record",
            "--duration",
            "0.5",
            "--output",
            wav_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute voicekey-cli record");

    if rec_output.status.success() {
        let rec_stdout = String::from_utf8_lossy(&rec_output.stdout);
        assert!(rec_stdout.contains("Recording Saved Successfully"));
    } else {
        // In headless CI runners without physical microphone devices, create synthetic 16kHz WAV
        eprintln!("No hardware audio capture device available in test environment; synthesizing WAV.");
        voicekey_core::audio::write_wav(&wav_path, &[0.0f32; 8000], 16000)
            .expect("Failed to write synthetic test WAV");
    }

    // 2. Transcribe the recorded file using tiny.en
    let trans_output = Command::new(&bin)
        .args([
            "transcribe",
            wav_path.to_str().unwrap(),
            "--model",
            "tiny.en",
        ])
        .output()
        .expect("Failed to transcribe audio via CLI");

    assert!(trans_output.status.success());
    let trans_stdout = String::from_utf8_lossy(&trans_output.stdout);
    assert!(trans_stdout.contains("VoiceKey GGML Whisper Transcriber"));
    assert!(trans_stdout.contains("Transcription Result"));
}

#[test]
fn test_cli_help_shows_phase9_commands() {
    let bin = get_cli_bin();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("Failed to execute voicekey-cli --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("toggle"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("paste"));
}

#[test]
fn test_cli_ipc_commands_integration() {
    use voicekey_core::ipc::{IpcCommand, IpcResponse, IpcServer};

    let test_sock =
        std::env::temp_dir().join(format!("voicekey_cli_ipc_{}.sock", std::process::id()));
    let _server = IpcServer::bind_at(&test_sock, |cmd| match cmd {
        IpcCommand::Status => IpcResponse::State("idle".into()),
        _ => IpcResponse::Ok,
    })
    .expect("Failed to start test IPC server");

    let bin = get_cli_bin();

    // Test status
    let status_out = Command::new(&bin)
        .env("VOICEKEY_SOCK", &test_sock)
        .arg("status")
        .output()
        .expect("Failed to execute voicekey-cli status");
    assert!(status_out.status.success());
    let status_str = String::from_utf8_lossy(&status_out.stdout);
    assert!(status_str.contains("State: idle"));

    // Test toggle
    let toggle_out = Command::new(&bin)
        .env("VOICEKEY_SOCK", &test_sock)
        .arg("toggle")
        .output()
        .expect("Failed to execute voicekey-cli toggle");
    assert!(toggle_out.status.success());
    let toggle_str = String::from_utf8_lossy(&toggle_out.stdout);
    assert!(toggle_str.contains("Toggle"));

    // Test start
    let start_out = Command::new(&bin)
        .env("VOICEKEY_SOCK", &test_sock)
        .arg("start")
        .output()
        .expect("Failed to execute voicekey-cli start");
    assert!(start_out.status.success());
    let start_str = String::from_utf8_lossy(&start_out.stdout);
    assert!(start_str.contains("Start"));

    // Test stop
    let stop_out = Command::new(&bin)
        .env("VOICEKEY_SOCK", &test_sock)
        .arg("stop")
        .output()
        .expect("Failed to execute voicekey-cli stop");
    assert!(stop_out.status.success());
    let stop_str = String::from_utf8_lossy(&stop_out.stdout);
    assert!(stop_str.contains("Stop"));
}

#[test]
fn test_cli_paste_command() {
    let bin = get_cli_bin();
    let output = Command::new(&bin)
        .args(["paste", "TestStringFromCli", "--no-restore"])
        .output()
        .expect("Failed to execute voicekey-cli paste");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active Window Target:"));
    assert!(stdout.contains("Payload Size: 17 characters"));
}
