// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;
mod settings;
mod state;
mod tray;

use tauri::{Emitter, Manager};
use tracing::{error, info};

use crate::state::spawn_actor;
use crate::tray::setup_tray;
use voicekey_core::ipc::{IpcCommand, IpcResponse, IpcServer};

fn main() {
    #[cfg(target_os = "linux")]
    {
        // On Linux GNOME/KDE Wayland, running under XWayland allows:
        // 1. True always-on-top (_NET_WM_STATE_ABOVE) overlay layering
        // 2. Custom window positioning without Wayland compositor centering overrides
        // 3. Undecorated transparent window without window titlebars
        if std::env::var_os("DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    tracing_subscriber::fmt::init();
    info!(
        "Starting VoiceKey Desktop Shell v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Eagerly initialize hardware keystroke virtual keyboard
    voicekey_core::input::KeyInjector::init();

    let engine = spawn_actor();

    let engine_for_single_instance = engine.clone();
    let engine_for_setup = engine.clone();
    let engine_for_ipc = engine.clone();
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
            crate::commands::start_drag,
            crate::commands::get_overlay_position,
            crate::commands::save_overlay_position,
            crate::commands::hide_overlay,
            crate::commands::test_llm_connection,
        ])
        .plugin(tauri_plugin_single_instance::init(
            move |app, _argv, _cwd| {
                info!("Second instance detected! Focusing application and triggering toggle.");
                let engine = engine_for_single_instance.clone();
                tauri::async_runtime::spawn(async move {
                    engine.toggle().await;
                });
                if let Some(win) = app.get_webview_window("overlay") {
                    crate::commands::show_overlay_window(&win);
                }
            },
        ))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            let handle = app.handle();

            if let Some(win) = handle.get_webview_window("overlay") {
                crate::commands::show_overlay_window(&win);
                let _ = win.hide();
            }

            // Background watcher to show/hide overlay window automatically on state transitions
            let app_handle_for_state_watcher = app.handle().clone();
            let mut state_rx = engine_for_setup.subscribe_state();
            tauri::async_runtime::spawn(async move {
                while state_rx.changed().await.is_ok() {
                    let cur_state = *state_rx.borrow();
                    if let Some(win) = app_handle_for_state_watcher.get_webview_window("overlay") {
                        match cur_state {
                            crate::state::EngineState::Recording
                            | crate::state::EngineState::Transcribing
                            | crate::state::EngineState::Pasting => {
                                crate::commands::show_overlay_window(&win);
                            }
                            crate::state::EngineState::Idle => {
                                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                                if *state_rx.borrow() == crate::state::EngineState::Idle {
                                    if let Ok(pos) = win.outer_position() {
                                        if let Ok(scale) = win.scale_factor() {
                                            let logical_x = pos.x as f64 / scale;
                                            let logical_y = pos.y as f64 / scale;
                                            if logical_x > 0.0 && logical_y > 0.0 {
                                                let mut settings = crate::settings::load_settings();
                                                settings.overlay_x = Some(logical_x);
                                                settings.overlay_y = Some(logical_y);
                                                let _ = crate::settings::save_settings(&settings);
                                            }
                                        }
                                    }
                                    let _ = win.hide();
                                }
                            }
                        }
                    }
                }
            });

            // Background watcher for partial streaming transcripts
            let app_handle_for_partial = app.handle().clone();
            let mut partial_rx = engine_for_setup.subscribe_partial_text();
            tauri::async_runtime::spawn(async move {
                while partial_rx.changed().await.is_ok() {
                    let text = partial_rx.borrow().clone();
                    let _ = app_handle_for_partial.emit("stream-partial-transcript", text);
                }
            });

            let eng = engine_for_setup.clone();
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                setup_tray(handle, eng)
            })) {
                Ok(Ok(_tray)) => info!("VoiceKey System Tray initialized successfully."),
                Ok(Err(e)) => tracing::warn!("Failed to initialize system tray: {e}"),
                Err(_) => tracing::warn!("System tray unavailable (libayatana-appindicator3 not found). Continuing with floating overlay."),
            }

            // Start local IPC Server for CLI and shortcut integration
            let app_handle_for_ipc = app.handle().clone();
            let engine_ipc = engine_for_ipc.clone();
            match IpcServer::start(move |cmd| {
                let engine = engine_ipc.clone();
                let app = app_handle_for_ipc.clone();
                match cmd {
                    IpcCommand::Toggle => {
                        tauri::async_runtime::spawn(async move {
                            if let Some(win) = app.get_webview_window("overlay") {
                                crate::commands::show_overlay_window(&win);
                            }
                            engine.toggle().await;
                        });
                        IpcResponse::Ok
                    }
                    IpcCommand::Start => {
                        tauri::async_runtime::spawn(async move {
                            if let Some(win) = app.get_webview_window("overlay") {
                                crate::commands::show_overlay_window(&win);
                            }
                            engine.start().await;
                        });
                        IpcResponse::Ok
                    }
                    IpcCommand::Stop => {
                        tauri::async_runtime::spawn(async move {
                            engine.stop().await;
                        });
                        IpcResponse::Ok
                    }
                    IpcCommand::Cancel => {
                        tauri::async_runtime::spawn(async move {
                            engine.cancel().await;
                        });
                        IpcResponse::Ok
                    }
                    IpcCommand::Status => {
                        IpcResponse::State("running".into())
                    }
                }
            }) {
                Ok(srv) => {
                    info!(
                        "Desktop IPC server listening on: {}",
                        srv.socket_path().display()
                    );
                    Box::leak(Box::new(srv));
                }
                Err(e) => {
                    error!("Failed to start desktop IPC server: {e}");
                }
            };

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running VoiceKey desktop application");
}
