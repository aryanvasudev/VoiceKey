//! Tokio actor state machine for VoiceKey desktop lifecycle.
//!
//! Lifecycle States:
//! Idle <-> Recording <-> Transcribing <-> Pasting

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{error, info, warn};

use voicekey_core::audio::{AudioRecorder, AudioRecorderConfig};
use voicekey_core::engine::{WhisperConfig, WhisperEngine};
use voicekey_core::input::ClipboardManager;
use voicekey_core::vad::EnergyVad;

/// Current operational state of the VoiceKey engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    #[default]
    Idle,
    Recording,
    Transcribing,
    Pasting,
}

impl std::fmt::Display for EngineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Recording => write!(f, "Recording..."),
            Self::Transcribing => write!(f, "Transcribing..."),
            Self::Pasting => write!(f, "Pasting..."),
        }
    }
}

/// Status payload returned by the actor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineStatus {
    pub state: EngineState,
    pub model: String,
    pub is_muted: bool,
    pub active_device: Option<String>,
}

/// Commands accepted by the VoiceKey Tokio actor.
#[derive(Debug)]
pub enum ActorCommand {
    Toggle,
    Start,
    Stop,
    Cancel,
    SetModel(String),
    ToggleMute,
    SetMute(bool),
    GetStatus(oneshot::Sender<EngineStatus>),
}

/// Handle to communicate with the background state machine actor.
#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: mpsc::Sender<ActorCommand>,
    state_rx: watch::Receiver<EngineState>,
}

impl EngineHandle {
    pub async fn toggle(&self) {
        let _ = self.cmd_tx.send(ActorCommand::Toggle).await;
    }

    pub async fn start(&self) {
        let _ = self.cmd_tx.send(ActorCommand::Start).await;
    }

    pub async fn stop(&self) {
        let _ = self.cmd_tx.send(ActorCommand::Stop).await;
    }

    pub async fn cancel(&self) {
        let _ = self.cmd_tx.send(ActorCommand::Cancel).await;
    }

    pub async fn set_model(&self, model: String) {
        let _ = self.cmd_tx.send(ActorCommand::SetModel(model)).await;
    }

    pub async fn toggle_mute(&self) {
        let _ = self.cmd_tx.send(ActorCommand::ToggleMute).await;
    }

    #[allow(dead_code)]
    pub async fn set_mute(&self, muted: bool) {
        let _ = self.cmd_tx.send(ActorCommand::SetMute(muted)).await;
    }

    #[allow(dead_code)]
    pub async fn get_status(&self) -> Option<EngineStatus> {
        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send(ActorCommand::GetStatus(tx)).await.is_ok() {
            rx.await.ok()
        } else {
            None
        }
    }

    pub fn subscribe_state(&self) -> watch::Receiver<EngineState> {
        self.state_rx.clone()
    }
}

/// Resolve path to GGML model file or preset.
pub fn resolve_model_path(model_name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(model_name);
    if p.exists() {
        return Some(p);
    }

    let mut search_dirs = Vec::new();
    if let Some(d) = dirs::data_dir() {
        search_dirs.push(d.join("voicekey").join("models"));
    }
    if let Some(c) = dirs::cache_dir() {
        search_dirs.push(c.join("voicekey").join("models"));
    }
    search_dirs.push(PathBuf::from("/tmp/voicekey/models"));

    let candidates = [
        format!("ggml-{}.bin", model_name),
        format!("ggml-{}-q5_0.bin", model_name),
        format!("{}.bin", model_name),
        model_name.to_string(),
    ];

    for dir in &search_dirs {
        for candidate in &candidates {
            let candidate_path = dir.join(candidate);
            if candidate_path.exists() {
                return Some(candidate_path);
            }
        }
    }

    // Fallback: check if any valid GGML .bin model exists in the search directories
    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("bin") {
                    tracing::info!(
                        "Requested model '{}' not found, falling back to available model: {}",
                        model_name,
                        path.display()
                    );
                    return Some(path);
                }
            }
        }
    }

    None
}

/// Spawn the Tokio actor state machine task.
pub fn spawn_actor() -> EngineHandle {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<ActorCommand>(32);
    let (state_tx, state_rx) = watch::channel(EngineState::Idle);

    let actor_cmd_tx = cmd_tx.clone();
    tauri::async_runtime::spawn(async move {
        let mut state = EngineState::Idle;
        let mut model_name = "base.en".to_string();
        let mut is_muted = false;
        let mut active_recorder: Option<AudioRecorder> = None;
        let mut cached_engine: Option<(String, Arc<WhisperEngine>)> = None;

        info!("VoiceKey Tokio State Machine Actor started in Idle state.");

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                ActorCommand::Toggle => {
                    match state {
                        EngineState::Idle => {
                            if is_muted {
                                info!("Dictation toggle ignored: microphone is muted.");
                                continue;
                            }
                            info!("State transition: Idle -> Recording");
                            voicekey_core::audio::play_sound_cue(
                                voicekey_core::audio::SoundCue::Start,
                            );
                            let rec_config = AudioRecorderConfig {
                                device_name: None,
                                pre_roll_ms: 200,
                                target_sample_rate: 16000,
                            };
                            let mut recorder = AudioRecorder::new(rec_config);
                            if let Err(e) = recorder.start() {
                                error!("Failed to start audio recording: {e}");
                                voicekey_core::audio::play_sound_cue(
                                    voicekey_core::audio::SoundCue::Error,
                                );
                                continue;
                            }
                            active_recorder = Some(recorder);
                            state = EngineState::Recording;
                            let _ = state_tx.send(state);
                        }
                        EngineState::Recording => {
                            info!("State transition: Recording -> Transcribing");
                            voicekey_core::audio::play_sound_cue(
                                voicekey_core::audio::SoundCue::Stop,
                            );
                            state = EngineState::Transcribing;
                            let _ = state_tx.send(state);

                            let samples = if let Some(mut rec) = active_recorder.take() {
                                rec.stop().unwrap_or_default()
                            } else {
                                Vec::new()
                            };

                            if samples.is_empty() {
                                warn!("No audio samples captured.");
                                state = EngineState::Idle;
                                let _ = state_tx.send(state);
                                continue;
                            }

                            // Voice Activity Detection Check
                            let rms = voicekey_core::vad::EnergyVad::calculate_rms(&samples);
                            let peak = voicekey_core::vad::EnergyVad::calculate_peak(&samples);
                            let duration_s = samples.len() as f64 / 16000.0;
                            info!(
                                "Captured {} samples ({:.2}s). RMS: {:.6}, Peak: {:.6}",
                                samples.len(),
                                duration_s,
                                rms,
                                peak
                            );

                            let vad = EnergyVad::default();
                            let has_voice = vad.has_speech(&samples);
                            if !has_voice && duration_s < 0.5 {
                                info!(
                                    "No speech detected (RMS: {:.6}, Peak: {:.6}, Duration: {:.2}s), returning to Idle.",
                                    rms, peak, duration_s
                                );
                                voicekey_core::audio::play_sound_cue(
                                    voicekey_core::audio::SoundCue::Error,
                                );
                                state = EngineState::Idle;
                                let _ = state_tx.send(state);
                                continue;
                            }

                            // Load or retrieve cached Whisper engine
                            let engine = if let Some((ref cached_name, ref eng)) = cached_engine {
                                if cached_name == &model_name {
                                    Some(Arc::clone(eng))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let engine = match engine {
                                Some(eng) => eng,
                                None => {
                                    if let Some(path) = resolve_model_path(&model_name) {
                                        let cfg = WhisperConfig {
                                            model_path: path,
                                            language: Some("en".into()),
                                            threads: 4,
                                            temperature: 0.0,
                                            translate: false,
                                            initial_prompt: None,
                                            enable_normalization: true,
                                            enable_spoken_punctuation: true,
                                        };
                                        match WhisperEngine::load_from_file(cfg) {
                                            Ok(eng) => {
                                                let arc_eng = Arc::new(eng);
                                                cached_engine = Some((
                                                    model_name.clone(),
                                                    Arc::clone(&arc_eng),
                                                ));
                                                arc_eng
                                            }
                                            Err(e) => {
                                                error!("Failed to load Whisper model: {e}");
                                                state = EngineState::Idle;
                                                let _ = state_tx.send(state);
                                                continue;
                                            }
                                        }
                                    } else {
                                        warn!("Whisper model '{}' not found on disk.", model_name);
                                        state = EngineState::Idle;
                                        let _ = state_tx.send(state);
                                        continue;
                                    }
                                }
                            };

                            // Transcribe in worker thread
                            let t0 = Instant::now();
                            let result = match tokio::task::spawn_blocking(move || {
                                engine.transcribe(&samples)
                            })
                            .await
                            {
                                Ok(Ok(res)) => res,
                                Ok(Err(e)) => {
                                    error!("Transcription failed: {e}");
                                    state = EngineState::Idle;
                                    let _ = state_tx.send(state);
                                    continue;
                                }
                                Err(e) => {
                                    error!("Task join error: {e}");
                                    state = EngineState::Idle;
                                    let _ = state_tx.send(state);
                                    continue;
                                }
                            };
                            info!(
                                "Transcription completed in {:.2}s: '{}'",
                                t0.elapsed().as_secs_f64(),
                                result.text
                            );

                            if !result.text.is_empty() {
                                state = EngineState::Pasting;
                                let _ = state_tx.send(state);
                                voicekey_core::audio::play_sound_cue(
                                    voicekey_core::audio::SoundCue::Complete,
                                );

                                // Load current user settings for debounce delay & LLM configuration
                                let current_settings = crate::settings::load_settings();

                                // Smart LLM Post-Processing (0ms fast path if disabled and no spoken command keyword)
                                let text_to_transform = result.text.clone();
                                let settings_clone = current_settings.clone();
                                let text_to_paste = tokio::task::spawn_blocking(move || {
                                    let forced_mode = if settings_clone.llm_enabled {
                                        match settings_clone.llm_transform_mode.as_str() {
                                            "bullets" => Some(voicekey_core::llm::LlmTransformMode::BulletPoints),
                                            "clean" => Some(voicekey_core::llm::LlmTransformMode::CleanTranscript),
                                            "professional" => Some(voicekey_core::llm::LlmTransformMode::ProfessionalTone),
                                            "casual" => Some(voicekey_core::llm::LlmTransformMode::CasualTone),
                                            "code" => Some(voicekey_core::llm::LlmTransformMode::VoiceToCode),
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    };

                                    let client = if settings_clone.llm_enabled
                                        || voicekey_core::llm::parse_spoken_command(&text_to_transform).is_some()
                                    {
                                        let cfg = voicekey_core::llm::LlmConfig {
                                            enabled: settings_clone.llm_enabled,
                                            endpoint: settings_clone.llm_endpoint,
                                            model: settings_clone.llm_model,
                                            api_type: voicekey_core::llm::LlmApiType::OpenAiCompatible,
                                            timeout_ms: 2000,
                                            temperature: 0.1,
                                            max_tokens: 256,
                                        };
                                        Some(voicekey_core::llm::HttpLlmClient::new(cfg))
                                    } else {
                                        None
                                    };

                                    voicekey_core::llm::process_with_llm(
                                        &text_to_transform,
                                        forced_mode,
                                        client.as_ref().map(|c| c as &dyn voicekey_core::llm::LlmClient),
                                    )
                                    .unwrap_or(text_to_transform)
                                })
                                .await
                                .unwrap_or_else(|_| result.text.clone());

                                let paste_text = text_to_paste.clone();
                                let delay_ms = current_settings.clipboard_delay_ms;
                                let paste_res = tokio::task::spawn_blocking(move || {
                                    ClipboardManager::paste_and_restore(
                                        &paste_text,
                                        false, // Keep transcribed text in clipboard
                                        delay_ms,
                                    )
                                })
                                .await;
                                info!("Auto-paste dispatch result: {:?}", paste_res);

                                #[cfg(target_os = "linux")]
                                {
                                    let preview = if result.text.len() <= 50 {
                                        result.text.clone()
                                    } else {
                                        format!("{}...", &result.text[..47])
                                    };
                                    let _ = std::process::Command::new("notify-send")
                                        .args([
                                            "-a",
                                            "VoiceKey",
                                            "-t",
                                            "2000",
                                            "VoiceKey",
                                            &format!("✅ Pasted: \"{}\"", preview),
                                        ])
                                        .spawn();
                                }
                            }

                            state = EngineState::Idle;
                            let _ = state_tx.send(state);
                        }
                        _ => {
                            warn!("Toggle ignored during {:?} state.", state);
                        }
                    }
                }
                ActorCommand::Start => {
                    if state == EngineState::Idle && !is_muted {
                        info!("State transition: Idle -> Recording (Explicit Start)");
                        let rec_config = AudioRecorderConfig {
                            device_name: None,
                            pre_roll_ms: 200,
                            target_sample_rate: 16000,
                        };
                        let mut recorder = AudioRecorder::new(rec_config);
                        if let Ok(()) = recorder.start() {
                            active_recorder = Some(recorder);
                            state = EngineState::Recording;
                            let _ = state_tx.send(state);
                        }
                    }
                }
                ActorCommand::Stop => {
                    if state == EngineState::Recording {
                        // Re-trigger Toggle logic to complete transcription & auto-paste
                        let _ = actor_cmd_tx.send(ActorCommand::Toggle).await;
                    }
                }
                ActorCommand::Cancel => {
                    if state == EngineState::Recording {
                        info!("Recording cancelled by user.");
                        if let Some(mut rec) = active_recorder.take() {
                            let _ = rec.stop();
                        }
                        state = EngineState::Idle;
                        let _ = state_tx.send(state);
                    }
                }
                ActorCommand::SetModel(new_model) => {
                    info!("Switched active Whisper model to: {}", new_model);
                    model_name = new_model;
                    cached_engine = None; // Reset cached model
                }
                ActorCommand::ToggleMute => {
                    is_muted = !is_muted;
                    info!("Microphone mute state changed: is_muted={}", is_muted);
                    if is_muted && state == EngineState::Recording {
                        if let Some(mut rec) = active_recorder.take() {
                            let _ = rec.stop();
                        }
                        state = EngineState::Idle;
                        let _ = state_tx.send(state);
                    }
                }
                ActorCommand::SetMute(muted) => {
                    is_muted = muted;
                    info!("Microphone mute explicitly set to: {}", is_muted);
                    if is_muted && state == EngineState::Recording {
                        if let Some(mut rec) = active_recorder.take() {
                            let _ = rec.stop();
                        }
                        state = EngineState::Idle;
                        let _ = state_tx.send(state);
                    }
                }
                ActorCommand::GetStatus(reply_tx) => {
                    let _ = reply_tx.send(EngineStatus {
                        state,
                        model: model_name.clone(),
                        is_muted,
                        active_device: None,
                    });
                }
            }
        }
    });

    EngineHandle { cmd_tx, state_rx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_machine_initial_status() {
        let handle = spawn_actor();
        let status = handle.get_status().await.expect("Expected status");
        assert_eq!(status.state, EngineState::Idle);
        assert_eq!(status.model, "base.en");
        assert!(!status.is_muted);
    }

    #[tokio::test]
    async fn test_state_machine_mute_toggle() {
        let handle = spawn_actor();
        handle.toggle_mute().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let status = handle.get_status().await.expect("Expected status");
        assert!(status.is_muted);

        handle.toggle_mute().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let status2 = handle.get_status().await.expect("Expected status");
        assert!(!status2.is_muted);
    }

    #[tokio::test]
    async fn test_state_machine_model_selection() {
        let handle = spawn_actor();
        handle.set_model("tiny.en".into()).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let status = handle.get_status().await.expect("Expected status");
        assert_eq!(status.model, "tiny.en");
    }
}
