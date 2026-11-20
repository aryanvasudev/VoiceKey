#!/usr/bin/env bash
# VoiceKey - Systemd User Service Installer
# Installs and enables the background daemon service for the current user.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

SERVICE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
SERVICE_FILE="$SERVICE_DIR/voicekey.service"

if ! command -v systemctl >/dev/null 2>&1; then
    echo "Error: 'systemctl' not found. This system does not support systemd user services." >&2
    exit 1
fi

# Support removal flag
if [[ "${1:-}" == "--remove" || "${1:-}" == "-r" ]]; then
    echo "Stopping and removing VoiceKey systemd user service..."
    systemctl --user stop voicekey.service 2>/dev/null || true
    systemctl --user disable voicekey.service 2>/dev/null || true
    rm -f "$SERVICE_FILE"
    systemctl --user daemon-reload
    echo "VoiceKey systemd service removed successfully."
    exit 0
fi

# Locate voicekeyd binary
VOICEKEYD_BIN="$(which voicekeyd 2>/dev/null || true)"
if [[ -z "$VOICEKEYD_BIN" ]]; then
    if [[ -f "$PROJECT_DIR/.venv/bin/voicekeyd" ]]; then
        VOICEKEYD_BIN="$PROJECT_DIR/.venv/bin/voicekeyd"
    else
        VOICEKEYD_BIN="$(which python3) -m voicekey.daemon"
    fi
fi

mkdir -p "$SERVICE_DIR"

echo "Creating systemd user service: $SERVICE_FILE"
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=VoiceKey Speech-to-Text Background Daemon
Documentation=https://github.com/aryanvasudev/VoiceKey
PartOf=graphical-session.target
After=graphical-session.target pipewire.service sound.target

[Service]
Type=simple
ExecStart=$VOICEKEYD_BIN
Restart=on-failure
RestartSec=3s
Environment=PYTHONUNBUFFERED=1

[Install]
WantedBy=default.target
EOF

echo "Reloading systemd daemon..."
systemctl --user daemon-reload

echo "Enabling VoiceKey user service..."
systemctl --user enable voicekey.service

echo "Starting VoiceKey user service..."
systemctl --user restart voicekey.service

echo ""
echo "✅ VoiceKey service installed, enabled, and started!"
echo "Check status anytime with:"
echo "  systemctl --user status voicekey.service"
echo "  or"
echo "  voicekey autostart --status"
