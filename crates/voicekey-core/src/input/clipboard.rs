//! Cross-platform clipboard manager with 250ms debounced restoration queue.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use arboard::Clipboard;

use crate::error::CoreError;
use crate::input::injector::KeyInjector;
use crate::input::terminal::is_active_window_terminal;

static PERSISTENT_CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Clipboard manager handling copy, paste, and atomic state restoration.
pub struct ClipboardManager;

impl ClipboardManager {
    /// Read the current plain text from the system clipboard.
    pub fn get_text() -> Result<String, CoreError> {
        // Try persistent arboard instance first
        if let Ok(mut guard) = PERSISTENT_CLIPBOARD.lock() {
            if guard.is_none() {
                *guard = Clipboard::new().ok();
            }
            if let Some(ref mut cb) = *guard {
                if let Ok(text) = cb.get_text() {
                    return Ok(text);
                }
            }
        }

        // Wayland fallback via wl-paste
        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                if let Ok(output) = std::process::Command::new("wl-paste")
                    .args(["--no-newline"])
                    .output()
                {
                    if output.status.success() {
                        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                    }
                }
            }
        }

        Err(CoreError::Audio("Failed to read system clipboard".into()))
    }

    /// Set the plain text of the system clipboard.
    pub fn set_text(text: &str) -> Result<(), CoreError> {
        let mut arboard_ok = false;
        if let Ok(mut guard) = PERSISTENT_CLIPBOARD.lock() {
            if guard.is_none() {
                *guard = Clipboard::new().ok();
            }
            if let Some(ref mut cb) = *guard {
                if cb.set_text(text).is_ok() {
                    arboard_ok = true;
                }
            }
        }

        // On Linux Wayland, also use wl-copy to guarantee compositor persistence
        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                if let Ok(status) = std::process::Command::new("wl-copy")
                    .arg(text)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                {
                    if status.success() {
                        return Ok(());
                    }
                }
            }
        }

        if arboard_ok {
            Ok(())
        } else {
            Err(CoreError::Audio("Failed to write to system clipboard".into()))
        }
    }

    /// Copy the transcribed text to clipboard, auto-paste into active target window,
    /// and optionally restore the user's prior clipboard content after a debounce delay.
    ///
    /// The 250ms asynchronous delay ensures that target applications (Chrome, VSCode, Slack)
    /// have completed reading the text from the OS clipboard event loop before the previous
    /// clipboard content is restored.
    pub fn paste_and_restore(
        text: &str,
        preserve_clipboard: bool,
        restore_delay_ms: u64,
    ) -> Result<(), CoreError> {
        if text.is_empty() {
            return Ok(());
        }

        let is_terminal = is_active_window_terminal();

        // 1. Read existing clipboard if preservation is enabled
        let previous_text = if preserve_clipboard {
            Self::get_text().ok()
        } else {
            None
        };

        // 2. Write transcribed text to clipboard
        Self::set_text(text)?;

        // 3. Inject simulated Ctrl+V or Ctrl+Shift+V
        KeyInjector::inject_paste(is_terminal)?;

        // 4. If preservation enabled, restore previous clipboard asynchronously
        if let Some(prev) = previous_text {
            let delay = Duration::from_millis(restore_delay_ms.max(50));
            thread::spawn(move || {
                thread::sleep(delay);
                let _ = Self::set_text(&prev);
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_set_and_get() {
        let test_payload = format!("voicekey_test_token_{}", std::process::id());
        let res = ClipboardManager::set_text(&test_payload);
        if res.is_ok() {
            let read_back = ClipboardManager::get_text();
            assert!(read_back.is_ok(), "Expected get_text() to succeed");
            assert_eq!(read_back.unwrap(), test_payload);
        }
    }
}
