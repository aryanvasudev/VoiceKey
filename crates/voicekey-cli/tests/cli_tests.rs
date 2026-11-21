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

    // 1. Record 0.5s audio
    let rec_output = Command::new(&bin)
        .args([
            "record",
            "--duration",
            "0.5",
            "--output",
            wav_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to record audio via CLI");

    assert!(rec_output.status.success());
    let rec_stdout = String::from_utf8_lossy(&rec_output.stdout);
    assert!(rec_stdout.contains("Recording Saved Successfully"));

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
