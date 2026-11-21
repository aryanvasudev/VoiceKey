//! Tauri IPC Commands invoked by the floating overlay and settings UI.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tracing::info;

use crate::models::{
    check_model_on_disk, download_model_with_progress, get_available_presets, recommend_model,
    ModelInfoDto, RecommendedModelDto,
};
use crate::settings::{load_settings, save_settings as persist_settings, SettingsPayload};
use crate::state::{EngineHandle, EngineStatus};
use voicekey_core::audio::AudioCaptureManager;
use voicekey_core::LlmClient;

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
    let active_model = status
        .as_ref()
        .map(|s| s.model.as_str())
        .unwrap_or("base.en");
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
    // 1. Update active model and settings in engine actor
    engine.update_settings(settings.clone()).await;
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

#[tauri::command]
pub fn start_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_overlay_position(window: tauri::WebviewWindow) -> Result<Option<(f64, f64)>, String> {
    if let Ok(pos) = window.outer_position() {
        if let Ok(scale) = window.scale_factor() {
            let logical_x = pos.x as f64 / scale;
            let logical_y = pos.y as f64 / scale;
            return Ok(Some((logical_x, logical_y)));
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn save_overlay_position(x: f64, y: f64) -> Result<(), String> {
    if x >= 0.0 && y >= 0.0 {
        let mut settings = load_settings();
        settings.overlay_x = Some(x);
        settings.overlay_y = Some(y);
        let _ = persist_settings(&settings);
        info!("Saved overlay window position: ({:.1}, {:.1})", x, y);
    }
    Ok(())
}

/// Reliably show and pin the floating overlay window at the user's chosen saved coordinates.
/// Prevents window managers (e.g. GNOME Mutter on Wayland/XWayland) from resetting position near cursor.
pub fn show_overlay_window(win: &tauri::WebviewWindow) {
    let settings = load_settings();
    let (target_x, target_y) = if let (Some(x), Some(y)) = (settings.overlay_x, settings.overlay_y)
    {
        (x.max(0.0), y.max(0.0))
    } else if let Ok(Some(monitor)) = win.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let mon_w = size.width as f64 / scale;
        let win_w = 320.0;
        let x = ((mon_w - win_w) / 2.0).max(0.0);
        let y = 32.0;
        (x, y)
    } else {
        (100.0, 32.0)
    };

    let pos = tauri::Position::Logical(tauri::LogicalPosition {
        x: target_x,
        y: target_y,
    });

    // 1. Pre-configure position prior to show
    let _ = win.set_position(pos);

    // 2. Map window and assert always-on-top
    let _ = win.show();
    let _ = win.set_always_on_top(true);

    // 3. Immediately re-assert position
    let _ = win.set_position(pos);

    // 4. Debounce check after window manager has completed X11/Wayland MapRequest
    let win_clone = win.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let _ = win_clone.set_position(pos);
    });
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("overlay") {
        if let Ok(pos) = win.outer_position() {
            if let Ok(scale) = win.scale_factor() {
                let logical_x = pos.x as f64 / scale;
                let logical_y = pos.y as f64 / scale;
                if logical_x > 0.0 && logical_y > 0.0 {
                    let mut settings = load_settings();
                    settings.overlay_x = Some(logical_x);
                    settings.overlay_y = Some(logical_y);
                    let _ = persist_settings(&settings);
                }
            }
        }
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn test_llm_connection(endpoint: String) -> Result<bool, String> {
    let cfg = voicekey_core::llm::LlmConfig {
        enabled: true,
        endpoint,
        model: "qwen2.5:0.5b".into(),
        api_type: voicekey_core::llm::LlmApiType::OpenAiCompatible,
        timeout_ms: 1000,
        temperature: 0.1,
        max_tokens: 16,
    };
    let client = voicekey_core::llm::HttpLlmClient::new(cfg);
    Ok(client.is_available())
}
