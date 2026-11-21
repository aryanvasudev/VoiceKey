//! Cross-platform hardware keystroke injection for auto-pasting.

use crate::error::CoreError;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::thread::sleep;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::time::Duration;

#[cfg(target_os = "linux")]
use evdev::uinput::VirtualDeviceBuilder;
#[cfg(target_os = "linux")]
use evdev::{AttributeSet, EventType, InputEvent, Key};

#[cfg(target_os = "linux")]
static PERSISTENT_UINPUT: std::sync::Mutex<Option<evdev::uinput::VirtualDevice>> =
    std::sync::Mutex::new(None);

/// Keyboard injector simulating hardware keystroke shortcuts (`Ctrl+V` or `Ctrl+Shift+V`).
pub struct KeyInjector;

impl KeyInjector {
    /// Eagerly initialize the hardware keystroke injector virtual device.
    pub fn init() {
        #[cfg(target_os = "linux")]
        {
            let mut guard = match PERSISTENT_UINPUT.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            if guard.is_none() {
                let mut keys = AttributeSet::<Key>::new();
                for code in 1..=127 {
                    keys.insert(Key::new(code));
                }
                if let Ok(builder) = VirtualDeviceBuilder::new() {
                    if let Ok(dev) = builder
                        .name("VoiceKey Virtual Keyboard")
                        .with_keys(&keys)
                        .and_then(|b| b.build())
                    {
                        tracing::info!("Eagerly initialized VoiceKey Virtual Keyboard device.");
                        *guard = Some(dev);
                    }
                }
            }
        }
    }

    /// Inject a paste keystroke sequence.
    ///
    /// If `is_terminal` is true, injects `Ctrl+Shift+V`.
    /// Otherwise, injects `Ctrl+V`.
    pub fn inject_paste(is_terminal: bool) -> Result<(), CoreError> {
        #[cfg(target_os = "linux")]
        {
            Self::inject_linux_uinput(is_terminal)
        }

        #[cfg(target_os = "windows")]
        {
            Self::inject_windows_sendinput(is_terminal)
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = is_terminal;
            Err(CoreError::Audio(
                "Unsupported OS for keystroke injection".into(),
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn inject_linux_uinput(is_terminal: bool) -> Result<(), CoreError> {
        let mut guard = match PERSISTENT_UINPUT.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if guard.is_none() {
            let mut keys = AttributeSet::<Key>::new();
            // Provide full key set 1..=127 so systemd-udevd tags device with ID_INPUT_KEYBOARD=1
            for code in 1..=127 {
                keys.insert(Key::new(code));
            }

            let builder = match VirtualDeviceBuilder::new() {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        "Failed to open /dev/uinput ({e}), falling back to external tool..."
                    );
                    return Self::inject_linux_fallback(is_terminal);
                }
            };

            let dev = match builder
                .name("VoiceKey Virtual Keyboard")
                .with_keys(&keys)
                .and_then(|b| b.build())
            {
                Ok(dev) => dev,
                Err(e) => {
                    tracing::warn!(
                        "Failed to build uinput device ({e}), falling back to external tool..."
                    );
                    return Self::inject_linux_fallback(is_terminal);
                }
            };

            *guard = Some(dev);
            // Allow libinput and the Wayland compositor to recognize the new keyboard
            sleep(Duration::from_millis(150));
        }

        if let Some(ref mut device) = *guard {
            let ctrl_down = InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 1);
            let shift_down = InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.code(), 1);
            let v_down = InputEvent::new(EventType::KEY, Key::KEY_V.code(), 1);
            let v_up = InputEvent::new(EventType::KEY, Key::KEY_V.code(), 0);
            let shift_up = InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.code(), 0);
            let ctrl_up = InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 0);

            if is_terminal {
                device
                    .emit(&[ctrl_down])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[shift_down])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[v_down])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(30));
                device
                    .emit(&[v_up])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[shift_up])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[ctrl_up])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
            } else {
                device
                    .emit(&[ctrl_down])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[v_down])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(30));
                device
                    .emit(&[v_up])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
                sleep(Duration::from_millis(15));
                device
                    .emit(&[ctrl_up])
                    .map_err(|e| CoreError::Audio(format!("uinput emit error: {e}")))?;
            }

            sleep(Duration::from_millis(20));
            Ok(())
        } else {
            Self::inject_linux_fallback(is_terminal)
        }
    }

    #[cfg(target_os = "linux")]
    fn inject_linux_fallback(is_terminal: bool) -> Result<(), CoreError> {
        // Try wtype (Wayland)
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let mut cmd = std::process::Command::new("wtype");
            if is_terminal {
                cmd.args([
                    "-M", "ctrl", "-M", "shift", "-k", "v", "-m", "shift", "-m", "ctrl",
                ]);
            } else {
                cmd.args(["-M", "ctrl", "-k", "v", "-m", "ctrl"]);
            }
            if let Ok(status) = cmd.status() {
                if status.success() {
                    return Ok(());
                }
            }

            // Try ydotool
            let mut ydo = std::process::Command::new("ydotool");
            if is_terminal {
                ydo.args(["key", "29:1", "42:1", "47:1", "47:0", "42:0", "29:0"]);
            } else {
                ydo.args(["key", "29:1", "47:1", "47:0", "29:0"]);
            }
            if let Ok(status) = ydo.status() {
                if status.success() {
                    return Ok(());
                }
            }
        }

        // Try xdotool (X11)
        if std::env::var_os("DISPLAY").is_some() {
            let key = if is_terminal {
                "ctrl+shift+v"
            } else {
                "ctrl+v"
            };
            if let Ok(status) = std::process::Command::new("xdotool")
                .args(["key", key])
                .status()
            {
                if status.success() {
                    return Ok(());
                }
            }
        }

        Err(CoreError::Audio(
            "Could not inject keystroke: /dev/uinput, wtype, ydotool, and xdotool failed".into(),
        ))
    }

    #[cfg(target_os = "windows")]
    fn inject_windows_sendinput(is_terminal: bool) -> Result<(), CoreError> {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_SHIFT,
        };
        const VK_V_CODE: u16 = 0x56;

        unsafe {
            let mut inputs = Vec::new();

            // Ctrl down
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });

            if is_terminal {
                inputs.push(INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_SHIFT,
                            wScan: 0,
                            dwFlags: 0,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                });
            }

            // V down
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V_CODE,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });

            // V up
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V_CODE,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });

            if is_terminal {
                inputs.push(INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_SHIFT,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                });
            }

            // Ctrl up
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });

            let sent = SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );

            if sent != inputs.len() as u32 {
                return Err(CoreError::Audio(
                    "Win32 SendInput failed to inject keystrokes (possibly UIPI elevation restriction)"
                        .into(),
                ));
            }
        }

        Ok(())
    }
}
