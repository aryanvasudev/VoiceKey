//! Active window terminal emulator detection.

use std::env;

/// Known terminal emulator process and window names.
pub const KNOWN_TERMINAL_CLASSES: &[&str] = &[
    "alacritty",
    "kitty",
    "foot",
    "gnome-terminal",
    "ptyxis",
    "konsole",
    "wezterm",
    "xterm",
    "ghostty",
    "terminator",
    "tilix",
    "rxvt",
    "urxvt",
    "st",
    "windowsterminal.exe",
    "cmd.exe",
    "powershell.exe",
];

/// Check whether the currently focused window is a terminal emulator.
///
/// When active, text pasting should use `Ctrl+Shift+V` instead of standard `Ctrl+V`.
pub fn is_active_window_terminal() -> bool {
    // 1. Check Hyprland active window via HYPRLAND_INSTANCE_SIGNATURE
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        if let Ok(output) = std::process::Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
        {
            if output.status.success() {
                let json = String::from_utf8_lossy(&output.stdout).to_lowercase();
                for &term in KNOWN_TERMINAL_CLASSES {
                    if json.contains(term) {
                        return true;
                    }
                }
            }
        }
    }

    // 2. Check Sway / wlroots active window via SWAYSOCK
    if env::var_os("SWAYSOCK").is_some() {
        if let Ok(output) = std::process::Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
        {
            if output.status.success() {
                let tree = String::from_utf8_lossy(&output.stdout).to_lowercase();
                // Check if focused window app_id matches
                for &term in KNOWN_TERMINAL_CLASSES {
                    if tree.contains(&format!("\"app_id\": \"{term}\""))
                        || tree.contains(&format!("\"class\": \"{term}\""))
                    {
                        return true;
                    }
                }
            }
        }
    }

    // 3. Check X11 via xdotool if available
    if env::var_os("DISPLAY").is_some() {
        if let Ok(output) = std::process::Command::new("xdotool")
            .args(["getactivewindow", "getwindowclassname"])
            .output()
        {
            if output.status.success() {
                let class_name = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
                for &term in KNOWN_TERMINAL_CLASSES {
                    if class_name.contains(term) {
                        return true;
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, query active foreground window process
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd != 0 {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid > 0 {
                    // Inspect process name if needed
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_terminals_contains_common_clients() {
        assert!(KNOWN_TERMINAL_CLASSES.contains(&"alacritty"));
        assert!(KNOWN_TERMINAL_CLASSES.contains(&"kitty"));
        assert!(KNOWN_TERMINAL_CLASSES.contains(&"gnome-terminal"));
        assert!(KNOWN_TERMINAL_CLASSES.contains(&"ptyxis"));
        assert!(KNOWN_TERMINAL_CLASSES.contains(&"ghostty"));
    }

    #[test]
    fn test_is_active_window_terminal_does_not_panic() {
        let _ = is_active_window_terminal();
    }
}
