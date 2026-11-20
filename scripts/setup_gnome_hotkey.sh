#!/usr/bin/env bash
# VoiceKey - GNOME Keyboard Shortcut Setup Script
# Configures a global shortcut for `voicekey toggle` using gsettings.

set -euo pipefail

BINDING_NAME="VoiceKey Dictation"
BINDING_PATH="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voicekey/"
DEFAULT_KEY="<Primary><Alt>space"
KEY_COMBINATION="${1:-$DEFAULT_KEY}"

if ! command -v gsettings >/dev/null 2>&1; then
    echo "Error: 'gsettings' not found. This script requires a GNOME-based desktop environment." >&2
    exit 1
fi

# Locate voicekey binary
VOICEKEY_BIN="$(which voicekey 2>/dev/null || true)"
if [[ -z "$VOICEKEY_BIN" ]]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    if [[ -f "$PROJECT_DIR/.venv/bin/voicekey" ]]; then
        VOICEKEY_BIN="$PROJECT_DIR/.venv/bin/voicekey"
    else
        VOICEKEY_BIN="voicekey"
    fi
fi

if [[ "${1:-}" == "--remove" ]]; then
    echo "Removing VoiceKey GNOME shortcut..."
    python3 - <<EOF
import subprocess, ast

res = subprocess.run(["gsettings", "get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"], capture_output=True, text=True, check=True)
raw = res.stdout.strip()
if raw.startswith("@as"):
    items = []
else:
    items = ast.literal_eval(raw)

items = [p for p in items if "$BINDING_PATH" not in p]
val_str = str(items) if items else "@as []"
subprocess.run(["gsettings", "set", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", val_str], check=True)
print("VoiceKey shortcut removed successfully.")
EOF
    exit 0
fi

echo "Configuring GNOME shortcut for VoiceKey..."
echo "  Action:    $VOICEKEY_BIN toggle"
echo "  Shortcut:  $KEY_COMBINATION"
echo "  Path:      $BINDING_PATH"

# Set binding details
gsettings set "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$BINDING_PATH" name "$BINDING_NAME"
gsettings set "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$BINDING_PATH" command "$VOICEKEY_BIN toggle"
gsettings set "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$BINDING_PATH" binding "$KEY_COMBINATION"

# Append to custom-keybindings list safely using Python
python3 - <<EOF
import subprocess, ast

res = subprocess.run(["gsettings", "get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"], capture_output=True, text=True, check=True)
raw = res.stdout.strip()
if raw.startswith("@as"):
    items = []
else:
    items = ast.literal_eval(raw)

target = "$BINDING_PATH"
if target not in items:
    items.append(target)
    subprocess.run(["gsettings", "set", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", str(items)], check=True)
    print("Added VoiceKey to GNOME custom-keybindings list.")
else:
    print("VoiceKey is already in GNOME custom-keybindings list.")
EOF

echo ""
echo "Successfully configured global hotkey: $KEY_COMBINATION -> 'voicekey toggle'"
echo "Test it now by opening any text field and pressing: $KEY_COMBINATION"
