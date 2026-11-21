//! Tauri IPC Commands invoked by the floating overlay and settings UI.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tracing::info;

use voicekey_core::audio::AudioCaptureManager;
use crate::models::{
    check_model_on_disk, download_model_with_progress, get_available_presets, recommend_model,
    ModelInfoDto, RecommendedModelDto,
};
use crate::settings::{load_settings, save_settings as persist_settings, SettingsPayload};
use crate::state::{EngineHandle, EngineStatus};

/// DTO for audio input devices.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDeviceDto {
    pub name: String,
    pub is_default: bool,
    pub channels: u16,
    pub sample_rate: u32,
}

#[tauri::command]
pub async fn get_engine_status(engine: State<'_, EngineHandle>) -> Result<EngineStatus, String> {
    engine
        .get_status()
        .await
        .ok_or_else(|| "Failed to query engine status".into())
}

#[tauri::command]
pub async fn toggle_dictation(engine: State<'_, EngineHandle>) -> Result<(), String> {
    engine.toggle().await;
    Ok(())
}

#[tauri::command]
pub async fn cancel_dictation(engine: State<'_, EngineHandle>) -> Result<(), String> {
    engine.cancel().await;
    Ok(())
}

#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<AudioDeviceDto>, String> {
    let mgr = AudioCaptureManager::new();
    let devices = mgr
        .list_input_devices()
        .map_err(|e| format!("Failed to list devices: {e}"))?;

    Ok(devices
        .into_iter()
        .map(|d| AudioDeviceDto {
            name: d.name,
            is_default: d.is_default,
            channels: d.channels,
            sample_rate: d.default_sample_rate,
        })
        .collect())
}

#[tauri::command]
pub async fn get_models_info(engine: State<'_, EngineHandle>) -> Result<Vec<ModelInfoDto>, String> {
    let presets = get_available_presets();
    let status = engine.get_status().await;
    let active_model = status.as_ref().map(|s| s.model.as_str()).unwrap_or("base.en");
    let rec = recommend_model();

    let mut result = Vec::new();
    for p in presets {
        let (is_downloaded, size_mb) = match check_model_on_disk(&p.filename) {
            Some((_, mb)) => (true, mb),
            None => (false, p.size_bytes as f64 / 1_048_576.0),
        };

        result.push(ModelInfoDto {
            id: p.id.clone(),
            name: p.name,
            filename: p.filename,
            is_downloaded,
            size_mb,
            description: p.description,
            is_active: p.id == active_model,
            is_recommended: p.id == rec.model_id,
        });
    }

    Ok(result)
}

#[tauri::command]
pub async fn download_model(app: AppHandle, model_id: String) -> Result<String, String> {
    let path = download_model_with_progress(app, model_id).await?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn get_recommended_model() -> RecommendedModelDto {
    recommend_model()
}

#[tauri::command]
pub fn get_settings() -> SettingsPayload {
    load_settings()
}

#[tauri::command]
pub async fn save_settings_cmd(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    settings: SettingsPayload,
) -> Result<(), String> {
    // 1. Update active model in engine actor
    engine.set_model(settings.active_model.clone()).await;

    // 2. Update autostart if requested
    let autolaunch = app.autolaunch();
    if settings.launch_at_startup {
        let _ = autolaunch.enable();
    } else {
        let _ = autolaunch.disable();
    }

    // 3. Persist to disk
    persist_settings(&settings)?;
    info!("Saved settings and updated active engine configuration.");
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}
