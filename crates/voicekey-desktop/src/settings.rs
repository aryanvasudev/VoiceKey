//! Desktop and engine configuration settings persistence.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Settings payload passed between frontend and backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    pub audio_device: Option<String>,
    pub active_model: String,
    pub spoken_punctuation: bool,
    pub hotkey_mode: String,
    pub clipboard_preserve: bool,
    pub clipboard_delay_ms: u64,
    pub custom_vocabulary: Vec<String>,
    pub launch_at_startup: bool,
    #[serde(default)]
    pub overlay_x: Option<f64>,
    #[serde(default)]
    pub overlay_y: Option<f64>,
    #[serde(default)]
    pub llm_enabled: bool,
    #[serde(default = "default_llm_endpoint")]
    pub llm_endpoint: String,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default = "default_llm_mode")]
    pub llm_transform_mode: String,
}

fn default_llm_endpoint() -> String {
    "http://127.0.0.1:11434".into()
}

fn default_llm_model() -> String {
    "qwen2.5:0.5b".into()
}

fn default_llm_mode() -> String {
    "auto".into()
}

impl Default for SettingsPayload {
    fn default() -> Self {
        Self {
            audio_device: None,
            active_model: "base.en".into(),
            spoken_punctuation: true,
            hotkey_mode: "toggle".into(),
            clipboard_preserve: true,
            clipboard_delay_ms: 250,
            custom_vocabulary: vec![
                "VoiceKey".into(),
                "Rust".into(),
                "Tauri".into(),
                "API".into(),
                "async".into(),
                "Kubernetes".into(),
            ],
            launch_at_startup: false,
            overlay_x: None,
            overlay_y: None,
            llm_enabled: false,
            llm_endpoint: default_llm_endpoint(),
            llm_model: default_llm_model(),
            llm_transform_mode: default_llm_mode(),
        }
    }
}

/// Resolve the configuration file path on disk (~/.config/voicekey/config.json).
pub fn get_config_path() -> PathBuf {
    if let Some(cfg) = dirs::config_dir() {
        return cfg.join("voicekey").join("desktop_config.json");
    }
    PathBuf::from("/tmp/voicekey_config.json")
}

/// Load settings from disk or return default settings.
pub fn load_settings() -> SettingsPayload {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<SettingsPayload>(&content) {
                return settings;
            }
        }
    }
    SettingsPayload::default()
}

/// Save settings payload to disk.
pub fn save_settings(settings: &SettingsPayload) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    fs::write(&path, json)
        .map_err(|e| format!("Failed to write configuration file {}: {e}", path.display()))?;

    info!("Settings persisted successfully to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_serialization_roundtrip() {
        let original = SettingsPayload::default();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SettingsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.active_model, original.active_model);
        assert_eq!(deserialized.clipboard_delay_ms, 250);
        assert!(deserialized.spoken_punctuation);
        assert!(!deserialized.llm_enabled);
        assert_eq!(deserialized.llm_model, "qwen2.5:0.5b");
        assert_eq!(deserialized.llm_endpoint, "http://127.0.0.1:11434");
    }
}
