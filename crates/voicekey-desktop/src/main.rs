// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;
mod settings;
mod state;
mod tray;

use tauri::Manager;
use tracing::{error, info};

use voicekey_core::ipc::{IpcCommand, IpcResponse, IpcServer};
use crate::state::spawn_actor;
use crate::tray::setup_tray;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting VoiceKey Desktop Shell v{}", env!("CARGO_PKG_VERSION"));

    let engine = spawn_actor();

    // Start local IPC Server for CLI and desktop integration
    let engine_ipc = engine.clone();
    let _ipc_server = match IpcServer::start(move |cmd| {
        let engine = engine_ipc.clone();
        match cmd {
            IpcCommand::Toggle => {
                tokio::spawn(async move {
                    engine.toggle().await;
                });
                IpcResponse::Ok
            }
            IpcCommand::Start => {
                tokio::spawn(async move {
                    engine.start().await;
                });
                IpcResponse::Ok
            }
            IpcCommand::Stop => {
                tokio::spawn(async move {
                    engine.stop().await;
                });
                IpcResponse::Ok
            }
            IpcCommand::Cancel => {
                tokio::spawn(async move {
                    engine.cancel().await;
                });
                IpcResponse::Ok
            }
            IpcCommand::Status => {
                // Synchronous quick query or block on status
                IpcResponse::State("running".into())
            }
        }
    }) {
        Ok(srv) => {
            info!("Desktop IPC server listening on: {}", srv.socket_path().display());
            Some(srv)
        }
        Err(e) => {
            error!("Failed to start desktop IPC server: {e}");
            None
        }
    };

    let engine_for_single_instance = engine.clone();
    let engine_for_setup = engine.clone();
    let engine_for_state = engine.clone();

    tauri::Builder::default()
        .manage(engine_for_state)
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_engine_status,
            crate::commands::toggle_dictation,
            crate::commands::cancel_dictation,
            crate::commands::list_audio_devices,
            crate::commands::get_models_info,
            crate::commands::download_model,
            crate::commands::get_recommended_model,
            crate::commands::get_settings,
            crate::commands::save_settings_cmd,
            crate::commands::open_settings_window,
        ])
        .plugin(tauri_plugin_single_instance::init(move |app, _argv, _cwd| {
            info!("Second instance detected! Focusing application and triggering toggle.");
            let engine = engine_for_single_instance.clone();
            tauri::async_runtime::spawn(async move {
                engine.toggle().await;
            });
            if let Some(win) = app.get_webview_window("overlay") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            let handle = app.handle();
            let _tray = setup_tray(handle, engine_for_setup)?;
            info!("VoiceKey System Tray initialized successfully.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running VoiceKey desktop application");
}
