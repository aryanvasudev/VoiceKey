//! Audio feedback sound cues for recording start, stop, paste, and errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCue {
    Start,
    Stop,
    Complete,
    Error,
}

/// Play an audible cue asynchronously without blocking the caller.
pub fn play_sound_cue(cue: SoundCue) {
    std::thread::spawn(move || {
        #[cfg(target_os = "linux")]
        {
            let name = match cue {
                SoundCue::Start => "audio-volume-change",
                SoundCue::Stop => "message",
                SoundCue::Complete => "complete",
                SoundCue::Error => "dialog-warning",
            };

            // 1. Try canberra-gtk-play (standard GNOME / freedesktop)
            if let Ok(status) = std::process::Command::new("canberra-gtk-play")
                .args(["-i", name])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                if status.success() {
                    return;
                }
            }

            // 2. Fallback to pw-play with system sound file
            let path = format!("/usr/share/sounds/freedesktop/stereo/{name}.oga");
            if std::path::Path::new(&path).exists() {
                let _ = std::process::Command::new("pw-play")
                    .arg(&path)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }

        #[cfg(target_os = "windows")]
        {
            let (freq, dur) = match cue {
                SoundCue::Start => (880, 70),
                SoundCue::Stop => (660, 70),
                SoundCue::Complete => (1046, 100),
                SoundCue::Error => (330, 150),
            };
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &format!("[console]::beep({freq}, {dur})")])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    });
}
