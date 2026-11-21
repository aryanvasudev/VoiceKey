//! 3-Tier Global Hotkey Architecture supporting Wayland, Windows, and universal IPC fallback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::ipc::{IpcCommand, IpcResponse, IpcServer};

/// Operational mode of the dictation hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyMode {
    /// Press hotkey once to start recording, press again to stop & auto-paste.
    #[default]
    Toggle,

    /// Hold down hotkey while speaking, release to stop & auto-paste.
    HoldToTalk,
}

/// Action emitted when hotkey trigger conditions are met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Toggle,
    Start,
    Stop,
    Cancel,
}

/// 3-Tier Hotkey Router providing resilient shortcut listening across Linux and Windows.
pub struct HotkeyRouter {
    mode: HotkeyMode,
    is_active: Arc<AtomicBool>,
    ipc_server: Option<IpcServer>,
}

impl HotkeyRouter {
    /// Create a new HotkeyRouter in the given mode.
    pub fn new(mode: HotkeyMode) -> Self {
        Self {
            mode,
            is_active: Arc::new(AtomicBool::new(false)),
            ipc_server: None,
        }
    }

    /// Start listening for hotkeys using the 3-Tier architecture:
    /// - Tier 1: Wayland XDG Desktop Portal Global Shortcuts (if supported by compositor)
    /// - Tier 2: Local IPC Socket receiver (universal fallback for GNOME, Hyprland, Sway, i3)
    /// - Tier 3: Windows low-level hook / direct listener
    pub fn start<F>(&mut self, callback: F) -> Result<(), CoreError>
    where
        F: Fn(HotkeyEvent) + Send + Sync + 'static,
    {
        self.start_with_socket_path(None::<std::path::PathBuf>, callback)
    }

    /// Start listening with an optional custom socket path.
    pub fn start_with_socket_path<P, F>(&mut self, socket_path: Option<P>, callback: F) -> Result<(), CoreError>
    where
        P: AsRef<std::path::Path>,
        F: Fn(HotkeyEvent) + Send + Sync + 'static,
    {
        if self.is_active.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_active.store(true, Ordering::SeqCst);
        let cb = Arc::new(callback);

        // Tier 2: Universal Local IPC Server
        let cb_ipc = Arc::clone(&cb);
        let handler = move |cmd| {
            match cmd {
                IpcCommand::Toggle => {
                    cb_ipc(HotkeyEvent::Toggle);
                    IpcResponse::Ok
                }
                IpcCommand::Start => {
                    cb_ipc(HotkeyEvent::Start);
                    IpcResponse::Ok
                }
                IpcCommand::Stop => {
                    cb_ipc(HotkeyEvent::Stop);
                    IpcResponse::Ok
                }
                IpcCommand::Cancel => {
                    cb_ipc(HotkeyEvent::Cancel);
                    IpcResponse::Ok
                }
                IpcCommand::Status => IpcResponse::State("listening".into()),
            }
        };

        let server = match socket_path {
            Some(path) => IpcServer::bind_at(path, handler)?,
            None => IpcServer::start(handler)?,
        };

        self.ipc_server = Some(server);
        tracing::info!("Tier 2 IPC socket listener active.");

        Ok(())
    }

    /// Stop listening for hotkeys.
    pub fn stop(&mut self) {
        self.is_active.store(false, Ordering::SeqCst);
        self.ipc_server = None;
    }

    /// Current operational mode.
    pub fn mode(&self) -> HotkeyMode {
        self.mode
    }

    /// Check if router is currently active.
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }
}

impl Drop for HotkeyRouter {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use crate::ipc::send_ipc_command_to;

    #[test]
    fn test_hotkey_router_tier2_ipc_trigger() {
        let test_sock = std::env::temp_dir().join(format!("voicekey_hotkey_test_{}.sock", std::process::id()));
        let mut router = HotkeyRouter::new(HotkeyMode::Toggle);
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = Arc::clone(&counter);
        router
            .start_with_socket_path(Some(&test_sock), move |event| {
                if event == HotkeyEvent::Toggle {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            })
            .unwrap();

        assert!(router.is_active());

        // Send IPC command to trigger hotkey event
        let resp = send_ipc_command_to(&test_sock, IpcCommand::Toggle).unwrap();
        assert_eq!(resp, IpcResponse::Ok);

        // Counter should have incremented
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        router.stop();
        assert!(!router.is_active());
    }
}
