#!/usr/bin/env bash
# ==============================================================================
# VoiceKey Linux Desktop & udev Installer Script
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BOLD}${CYAN}=== VoiceKey Linux Desktop Installer ===${NC}\n"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PREFIX="${HOME}/.local"
BIN_DIR="${PREFIX}/bin"
APP_DIR="${PREFIX}/share/applications"
ICON_DIR="${PREFIX}/share/icons/hicolor/128x128/apps"
ICON_SRC="${ROOT_DIR}/crates/voicekey-desktop/icons/icon.png"

# 1. Build release binaries if not found
echo -e "${CYAN}[1/5] Checking Release Binaries...${NC}"
DESKTOP_BIN="${ROOT_DIR}/target/release/voicekey-desktop"
CLI_BIN="${ROOT_DIR}/target/release/voicekey-cli"

if [[ ! -f "$DESKTOP_BIN" || ! -f "$CLI_BIN" ]]; then
    echo -e "Compiling optimized release binaries with Cargo..."
    cargo build --release -p voicekey-desktop -p voicekey-cli --manifest-path "${ROOT_DIR}/Cargo.toml"
fi
echo -e "${GREEN}✓ Release binaries ready.${NC}"

# 2. Install binaries to ~/.local/bin
echo -e "\n${CYAN}[2/5] Installing Binaries to ${BIN_DIR}...${NC}"
mkdir -p "$BIN_DIR"
cp -f "$DESKTOP_BIN" "$BIN_DIR/voicekey-desktop"
cp -f "$CLI_BIN" "$BIN_DIR/voicekey-cli"
chmod +x "$BIN_DIR/voicekey-desktop" "$BIN_DIR/voicekey-cli"
echo -e "${GREEN}✓ Binaries installed: voicekey-desktop, voicekey-cli${NC}"

# 3. Install Icon and Desktop Launcher
echo -e "\n${CYAN}[3/5] Installing Desktop Integration & Icon...${NC}"
mkdir -p "$ICON_DIR" "$APP_DIR"
if [[ -f "$ICON_SRC" ]]; then
    cp -f "$ICON_SRC" "${ICON_DIR}/voicekey.png"
    echo -e "${GREEN}✓ Application icon installed to ${ICON_DIR}/voicekey.png${NC}"
fi

cp -f "${SCRIPT_DIR}/voicekey.desktop" "${APP_DIR}/voicekey.desktop"
sed -i "s|Exec=voicekey-desktop|Exec=${BIN_DIR}/voicekey-desktop|g" "${APP_DIR}/voicekey.desktop"
sed -i "s|Exec=voicekey-cli toggle|Exec=${BIN_DIR}/voicekey-cli toggle|g" "${APP_DIR}/voicekey.desktop"
update-desktop-database "$APP_DIR" 2>/dev/null || true
echo -e "${GREEN}✓ Desktop launcher installed to ${APP_DIR}/voicekey.desktop${NC}"

# 4. Check udev /dev/uinput rules
echo -e "\n${CYAN}[4/5] Checking /dev/uinput Kernel Device Permissions...${NC}"
UDEV_RULE_SRC="${SCRIPT_DIR}/99-voicekey-uinput.rules"
UDEV_RULE_DEST="/etc/udev/rules.d/99-voicekey-uinput.rules"

if [[ -w "/dev/uinput" ]]; then
    echo -e "${GREEN}✓ Current user already has read/write permissions on /dev/uinput.${NC}"
else
    echo -e "${YELLOW}Notice: Current user lacks write access to /dev/uinput.${NC}"
    echo -e "To grant non-root permissions, install the udev rule:"
    echo -e "  sudo cp ${UDEV_RULE_SRC} ${UDEV_RULE_DEST}"
    echo -e "  sudo udevadm control --reload-rules && sudo udevadm trigger"
    echo -e "  sudo usermod -aG input \$USER"
    if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
        sudo cp "$UDEV_RULE_SRC" "$UDEV_RULE_DEST"
        sudo udevadm control --reload-rules && sudo udevadm trigger || true
        sudo usermod -aG input "$USER" || true
        echo -e "${GREEN}✓ Udev rule installed automatically.${NC}"
    fi
fi

# 5. System autostart setup
echo -e "\n${CYAN}[5/5] Checking Systemd User Service...${NC}"
if command -v systemctl >/dev/null 2>&1; then
    SERVICE_DIR="${HOME}/.config/systemd/user"
    mkdir -p "$SERVICE_DIR"
    cat << EOF > "${SERVICE_DIR}/voicekey.service"
[Unit]
Description=VoiceKey Voice Dictation Background Daemon
After=graphical-session.target sound.target pipewire.service
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=${BIN_DIR}/voicekey-desktop
Restart=on-failure
RestartSec=3

[Install]
WantedBy=graphical-session.target
EOF
    systemctl --user daemon-reload 2>/dev/null || true
    echo -e "${GREEN}✓ Systemd user service installed: ~/.config/systemd/user/voicekey.service${NC}"
    echo -e "  To enable autostart: systemctl --user enable voicekey"
fi

echo -e "\n${BOLD}${GREEN}=== VoiceKey Desktop Installation Complete! ===${NC}"
echo -e "You can now run VoiceKey by typing ${BOLD}voicekey-desktop${NC} or launching it from your application menu.\n"
