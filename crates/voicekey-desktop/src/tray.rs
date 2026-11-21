//! Native System Tray Menu and Icon Management for Tauri v2.

use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;
use tracing::info;

use crate::state::{EngineHandle, EngineState};

const ICON_IDLE_BYTES: &[u8] = include_bytes!("../icons/icon_idle.png");
const ICON_RECORDING_BYTES: &[u8] = include_bytes!("../icons/icon_recording.png");
const ICON_PROCESSING_BYTES: &[u8] = include_bytes!("../icons/icon_processing.png");

/// System tray controller maintaining references to dynamic menu items.
#[allow(dead_code)]
pub struct TrayController {
    tray_icon: TrayIcon<Wry>,
    status_item: MenuItem<Wry>,
    toggle_item: MenuItem<Wry>,
    mute_item: CheckMenuItem<Wry>,
    model_tiny_item: CheckMenuItem<Wry>,
    model_base_item: CheckMenuItem<Wry>,
    model_small_item: CheckMenuItem<Wry>,
    _autostart_item: CheckMenuItem<Wry>,
}

impl TrayController {
    /// Update tray icon and status text according to current engine state.
    pub fn update_state(&self, state: EngineState) {
        let (icon_bytes, label, toggle_label) = match state {
            EngineState::Idle => (ICON_IDLE_BYTES, "VoiceKey: Ready", "Start Recording"),
            EngineState::Recording => (ICON_RECORDING_BYTES, "VoiceKey: Recording...", "Stop & Paste"),
            EngineState::Transcribing => (ICON_PROCESSING_BYTES, "VoiceKey: Transcribing...", "Transcribing..."),
            EngineState::Pasting => (ICON_PROCESSING_BYTES, "VoiceKey: Pasting...", "Pasting..."),
        };

        if let Ok(img) = Image::from_bytes(icon_bytes) {
            let _ = self.tray_icon.set_icon(Some(img));
        }
        let _ = self.status_item.set_text(label);
        let _ = self.toggle_item.set_text(toggle_label);
    }

    /// Update model selection checks.
    #[allow(dead_code)]
    pub fn update_active_model(&self, model: &str) {
        let _ = self.model_tiny_item.set_checked(model == "tiny.en");
        let _ = self.model_base_item.set_checked(model == "base.en");
        let _ = self.model_small_item.set_checked(model == "small.en");
    }

    /// Update mute check item.
    #[allow(dead_code)]
    pub fn update_mute(&self, is_muted: bool) {
        let _ = self.mute_item.set_checked(is_muted);
    }
}

/// Setup the native system tray icon and menu structure.
pub fn setup_tray(app: &AppHandle, engine: EngineHandle) -> Result<Arc<TrayController>, tauri::Error> {
    let status_item = MenuItem::with_id(app, "status", "VoiceKey: Ready", false, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, "toggle", "Toggle Dictation", true, None::<&str>)?;
    let mute_item = CheckMenuItem::with_id(app, "mute", "Mute Microphone", true, false, None::<&str>)?;

    // Whisper Model Submenu
    let model_tiny_item = CheckMenuItem::with_id(app, "model_tiny", "tiny.en (Fastest)", true, false, None::<&str>)?;
    let model_base_item = CheckMenuItem::with_id(app, "model_base", "base.en (Default)", true, true, None::<&str>)?;
    let model_small_item = CheckMenuItem::with_id(app, "model_small", "small.en (Accurate)", true, false, None::<&str>)?;
    let model_submenu = Submenu::with_items(app, "Whisper Model", true, &[&model_tiny_item, &model_base_item, &model_small_item])?;

    // Autostart integration
    let is_autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_item = CheckMenuItem::with_id(app, "autostart", "Launch at Startup", true, is_autostart_enabled, None::<&str>)?;

    let overlay_item = MenuItem::with_id(app, "overlay", "Show Floating Overlay", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit VoiceKey", true, None::<&str>)?;

    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let separator4 = PredefinedMenuItem::separator(app)?;

    let tray_menu = Menu::with_items(
        app,
        &[
            &status_item,
            &separator1,
            &toggle_item,
            &mute_item,
            &separator2,
            &model_submenu,
            &separator3,
            &autostart_item,
            &overlay_item,
            &separator4,
            &quit_item,
        ],
    )?;

    let initial_icon = Image::from_bytes(ICON_IDLE_BYTES)?;

    let engine_clone = engine.clone();
    let app_handle = app.clone();

    let tray_icon = TrayIconBuilder::with_id("main-tray")
        .icon(initial_icon)
        .menu(&tray_menu)
        .tooltip("VoiceKey - Cross-Platform Hotkey Dictation")
        .show_menu_on_left_click(true)
        .on_menu_event(move |_app, event| {
            let id = event.id.as_ref().to_string();
            let engine = engine_clone.clone();
            let app = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                match id.as_str() {
                    "toggle" => {
                        engine.toggle().await;
                    }
                    "mute" => {
                        engine.toggle_mute().await;
                    }
                    "model_tiny" => {
                        engine.set_model("tiny.en".into()).await;
                    }
                    "model_base" => {
                        engine.set_model("base.en".into()).await;
                    }
                    "model_small" => {
                        engine.set_model("small.en".into()).await;
                    }
                    "autostart" => {
                        let manager = app.autolaunch();
                        if let Ok(enabled) = manager.is_enabled() {
                            if enabled {
                                let _ = manager.disable();
                                info!("Disabled application autostart.");
                            } else {
                                let _ = manager.enable();
                                info!("Enabled application autostart.");
                            }
                        }
                    }
                    "overlay" => {
                        if let Some(win) = app.get_webview_window("overlay") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        info!("Quit requested via tray menu.");
                        app.exit(0);
                    }
                    _ => {}
                }
            });
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click { button, .. } = event {
                info!("Tray icon clicked: {:?}", button);
            }
        })
        .build(app)?;

    let controller = Arc::new(TrayController {
        tray_icon,
        status_item,
        toggle_item,
        mute_item,
        model_tiny_item,
        model_base_item,
        model_small_item,
        _autostart_item: autostart_item,
    });

    // Start background watcher to update tray icon on state changes
    let controller_clone = Arc::clone(&controller);
    let mut state_rx = engine.subscribe_state();
    tauri::async_runtime::spawn(async move {
        while state_rx.changed().await.is_ok() {
            let new_state = *state_rx.borrow();
            controller_clone.update_state(new_state);
        }
    });

    Ok(controller)
}
