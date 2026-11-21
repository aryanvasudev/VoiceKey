//! Cross-platform Unix Domain Socket and Local IPC system for instant hotkey triggers.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};
use crate::error::CoreError;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// Available IPC commands for controlling VoiceKey dictation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IpcCommand {
    Toggle,
    Start,
    Stop,
    Cancel,
    Status,
}

/// Server response to an IPC command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IpcResponse {
    Ok,
    State(String),
    Error(String),
}

/// Resolve the default IPC socket path on disk.
pub fn get_socket_path() -> PathBuf {
    if let Some(sock) = std::env::var_os("VOICEKEY_SOCK") {
        return PathBuf::from(sock);
    }

    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("voicekey.sock");
    }

    if let Some(state_dir) = dirs::state_dir() {
        return state_dir.join("voicekey").join("daemon.sock");
    }

    PathBuf::from("/tmp/voicekey.sock")
}

/// Send an IPC command to a specific socket path on disk.
pub fn send_ipc_command_to<P: AsRef<std::path::Path>>(
    socket_path: P,
    cmd: IpcCommand,
) -> Result<IpcResponse, CoreError> {
    let socket_path = socket_path.as_ref();
    if !socket_path.exists() {
        return Err(CoreError::Audio(format!(
            "VoiceKey IPC socket does not exist at {}. Is the daemon running?",
            socket_path.display()
        )));
    }

    #[cfg(unix)]
    {
        let mut stream = UnixStream::connect(socket_path)
            .map_err(|e| CoreError::Audio(format!("Failed to connect to IPC socket: {e}")))?;

        let req_json = serde_json::to_string(&cmd)
            .map_err(|e| CoreError::Audio(format!("Serialization error: {e}")))?;
        writeln!(stream, "{req_json}")
            .map_err(|e| CoreError::Audio(format!("Failed to write to IPC socket: {e}")))?;

        let mut reader = BufReader::new(stream);
        let mut resp_str = String::new();
        reader
            .read_line(&mut resp_str)
            .map_err(|e| CoreError::Audio(format!("Failed to read from IPC socket: {e}")))?;

        let resp: IpcResponse = serde_json::from_str(&resp_str)
            .map_err(|e| CoreError::Audio(format!("Invalid response from IPC socket: {e}")))?;

        Ok(resp)
    }

    #[cfg(not(unix))]
    {
        Err(CoreError::Audio("IPC not supported on this platform yet".into()))
    }
}

/// Send an IPC command to a running VoiceKey daemon or desktop application on default path.
pub fn send_ipc_command(cmd: IpcCommand) -> Result<IpcResponse, CoreError> {
    send_ipc_command_to(get_socket_path(), cmd)
}

/// An active local IPC server receiving commands and dispatching actions.
pub struct IpcServer {
    socket_path: PathBuf,
    is_running: Arc<AtomicBool>,
}

impl IpcServer {
    /// Bind and start an IPC server on a custom socket path.
    pub fn bind_at<P, F>(socket_path: P, handler: F) -> Result<Self, CoreError>
    where
        P: AsRef<std::path::Path>,
        F: Fn(IpcCommand) -> IpcResponse + Send + Sync + 'static,
    {
        let socket_path = socket_path.as_ref().to_path_buf();
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if socket_path.exists() {
            let _ = fs::remove_file(&socket_path);
        }

        let is_running = Arc::new(AtomicBool::new(true));

        #[cfg(unix)]
        {
            let listener = UnixListener::bind(&socket_path)
                .map_err(|e| CoreError::Audio(format!("Failed to bind IPC socket: {e}")))?;

            let is_running_clone = Arc::clone(&is_running);
            let handler = Arc::new(handler);

            thread::spawn(move || {
                for stream in listener.incoming() {
                    if !is_running_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    if let Ok(mut stream) = stream {
                        let handler = Arc::clone(&handler);
                        thread::spawn(move || {
                            let mut reader = BufReader::new(&mut stream);
                            let mut line = String::new();
                            if reader.read_line(&mut line).is_ok() {
                                let resp = match serde_json::from_str::<IpcCommand>(&line) {
                                    Ok(cmd) => handler(cmd),
                                    Err(e) => IpcResponse::Error(format!("Malformed command: {e}")),
                                };
                                if let Ok(resp_json) = serde_json::to_string(&resp) {
                                    let _ = writeln!(stream, "{resp_json}");
                                }
                            }
                        });
                    }
                }
            });
        }

        Ok(Self {
            socket_path,
            is_running,
        })
    }

    /// Bind and start an IPC server on the default socket path.
    pub fn start<F>(handler: F) -> Result<Self, CoreError>
    where
        F: Fn(IpcCommand) -> IpcResponse + Send + Sync + 'static,
    {
        Self::bind_at(get_socket_path(), handler)
    }

    /// Path of the active socket on disk.
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        let _ = fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_roundtrip() {
        let test_sock = std::env::temp_dir().join(format!("voicekey_ipc_test_{}.sock", std::process::id()));
        let server = IpcServer::bind_at(&test_sock, |cmd| match cmd {
            IpcCommand::Toggle => IpcResponse::Ok,
            IpcCommand::Status => IpcResponse::State("idle".into()),
            _ => IpcResponse::Ok,
        })
        .unwrap();

        let resp = send_ipc_command_to(&test_sock, IpcCommand::Toggle).unwrap();
        assert_eq!(resp, IpcResponse::Ok);

        let resp_status = send_ipc_command_to(&test_sock, IpcCommand::Status).unwrap();
        assert_eq!(resp_status, IpcResponse::State("idle".into()));

        drop(server);
    }
}
