#!/usr/bin/env bash
# ==============================================================================
# VoiceKey - Automated All-In-One Installer for Linux Desktop & CLI
# ==============================================================================
set -euo pipefail

BOLD="\033[1m"
GREEN="\033[32m"
BLUE="\033[34m"
YELLOW="\033[33m"
CYAN="\033[36m"
RED="\033[31m"
RESET="\033[0m"

log_step() {
    echo -e "\n${BOLD}${BLUE}==>${RESET} ${BOLD}$1${RESET}"
}
log_info() {
    echo -e "    ${CYAN}ℹ${RESET} $1"
}
log_success() {
    echo -e "    ${GREEN}✔${RESET} $1"
}
log_warn() {
    echo -e "    ${YELLOW}⚠${RESET} $1"
}
log_error() {
    echo -e "    ${RED}✖${RESET} $1"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"
SKIP_SUDO=false
CLI_ONLY=false
ACTION="install"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-sudo)
            SKIP_SUDO=true
            shift
            ;;
        --cli-only)
            CLI_ONLY=true
            shift
            ;;
        --uninstall|-u)
            ACTION="uninstall"
            shift
            ;;
        --help|-h)
            echo "Usage: ./install.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --cli-only        Install only headless CLI binary without desktop UI"
            echo "  --no-sudo         Skip system package installation and udev rules (non-root)"
            echo "  --uninstall, -u   Remove VoiceKey binaries, desktop entry, and service"
            echo "  --help, -h        Display this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Run './install.sh --help' for usage."
            exit 1
            ;;
    esac
done

PREFIX="${HOME}/.local"
BIN_DIR="${PREFIX}/bin"
APP_DIR="${PREFIX}/share/applications"
ICON_DIR="${PREFIX}/share/icons/hicolor/128x128/apps"
SERVICE_DIR="${HOME}/.config/systemd/user"

# ------------------------------------------------------------------------------
# UNINSTALL LOGIC
# ------------------------------------------------------------------------------
if [[ "$ACTION" == "uninstall" ]]; then
    echo -e "${BOLD}${RED}VoiceKey Uninstaller${RESET}"
    log_step "Stopping and removing systemd user service..."
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user stop voicekey.service 2>/dev/null || true
        systemctl --user disable voicekey.service 2>/dev/null || true
        rm -f "${SERVICE_DIR}/voicekey.service"
        systemctl --user daemon-reload 2>/dev/null || true
        log_success "Systemd user service removed."
    fi

    log_step "Removing application desktop entry and icon..."
    rm -f "${APP_DIR}/voicekey.desktop"
    rm -f "${ICON_DIR}/voicekey.png"
    update-desktop-database "$APP_DIR" 2>/dev/null || true
    log_success "Desktop entries removed."

    log_step "Removing installed binaries..."
    rm -f "$BIN_DIR/voicekey-desktop" "$BIN_DIR/voicekey-cli" "$BIN_DIR/voicekey" "$BIN_DIR/voicekeyd"
    log_success "Binaries removed from ${BIN_DIR}."

    echo -e "\n${BOLD}${GREEN}✔ VoiceKey uninstalled successfully.${RESET}\n"
    exit 0
fi

# ------------------------------------------------------------------------------
# INSTALLATION
# ------------------------------------------------------------------------------
echo -e "${BOLD}${CYAN}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║          VoiceKey All-in-One Linux Desktop Installer          ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${RESET}"

if [[ "$(uname -s)" != "Linux" ]]; then
    log_error "This installer is designed for Linux systems."
    exit 1
fi

# 1. System Dependencies
log_step "1/5 Checking & installing system prerequisites..."
if [[ "$SKIP_SUDO" == false ]]; then
    if command -v dnf >/dev/null 2>&1; then
        log_info "Detected Fedora / RHEL package manager (dnf)"
        sudo dnf install -y alsa-lib-devel webkit2gtk4.1-devel libayatana-appindicator-gtk3 libayatana-appindicator-gtk3-devel libdbusmenu-gtk3 openssl-devel wl-clipboard libevdev libnotify 2>/dev/null || true
        log_success "System dependencies configured."
    elif command -v apt-get >/dev/null 2>&1; then
        log_info "Detected Debian / Ubuntu package manager (apt)"
        sudo apt-get update -y
        sudo apt-get install -y libasound2-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libssl-dev librsvg2-dev wl-clipboard libevdev2 libnotify-bin 2>/dev/null || true
        log_success "System dependencies configured."
    elif command -v pacman >/dev/null 2>&1; then
        log_info "Detected Arch Linux package manager (pacman)"
        sudo pacman -S --needed --noconfirm alsa-lib webkit2gtk-4.1 libayatana-appindicator openssl wl-clipboard libevdev libnotify 2>/dev/null || true
        log_success "System dependencies configured."
    fi
else
    log_info "Skipping system package installation (--no-sudo flag)."
fi

# 2. Kernel /dev/uinput Permissions for Hardware Keystroke Auto-Paste
log_step "2/5 Configuring /dev/uinput kernel permissions for auto-paste..."
UDEV_RULE_SRC="${PROJECT_DIR}/scripts/99-voicekey-uinput.rules"
UDEV_RULE_DEST="/etc/udev/rules.d/99-voicekey-uinput.rules"

if [[ -w "/dev/uinput" ]]; then
    log_success "Current user already has read/write permissions on /dev/uinput."
else
    if [[ "$SKIP_SUDO" == false ]] && command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
        sudo cp "$UDEV_RULE_SRC" "$UDEV_RULE_DEST" 2>/dev/null || true
        sudo udevadm control --reload-rules 2>/dev/null || true
        sudo udevadm trigger 2>/dev/null || true
        sudo usermod -aG input "$USER" 2>/dev/null || true
        log_success "Udev rules configured for non-root /dev/uinput access."
    else
        log_warn "User lacks write access to /dev/uinput. For kernel-level auto-paste, run:"
        log_warn "  sudo cp scripts/99-voicekey-uinput.rules /etc/udev/rules.d/ && sudo usermod -aG input \$USER"
    fi
fi

# 3. Build optimized Rust binaries
log_step "3/5 Building release binaries with Cargo..."
mkdir -p "$BIN_DIR"
cd "$PROJECT_DIR"

if [[ "$CLI_ONLY" == true ]]; then
    cargo build --release -p voicekey-cli
    cp -f "${PROJECT_DIR}/target/release/voicekey-cli" "${BIN_DIR}/voicekey-cli"
    ln -sf "${BIN_DIR}/voicekey-cli" "${BIN_DIR}/voicekey"
    log_success "Installed voicekey-cli to ${BIN_DIR}"
else
    cargo build --release -p voicekey-desktop -p voicekey-cli
    cp -f "${PROJECT_DIR}/target/release/voicekey-desktop" "${BIN_DIR}/voicekey-desktop"
    cp -f "${PROJECT_DIR}/target/release/voicekey-cli" "${BIN_DIR}/voicekey-cli"
    ln -sf "${BIN_DIR}/voicekey-cli" "${BIN_DIR}/voicekey"
    chmod +x "${BIN_DIR}/voicekey-desktop" "${BIN_DIR}/voicekey-cli"
    log_success "Installed voicekey-desktop and voicekey-cli to ${BIN_DIR}"
fi

# 4. Desktop Integration (Icon & .desktop Launcher)
if [[ "$CLI_ONLY" == false ]]; then
    log_step "4/5 Installing application icon and desktop launcher..."
    mkdir -p "$ICON_DIR" "$APP_DIR"
    ICON_SRC="${PROJECT_DIR}/crates/voicekey-desktop/icons/icon.png"
    if [[ -f "$ICON_SRC" ]]; then
        cp -f "$ICON_SRC" "${ICON_DIR}/voicekey.png"
    fi
    cp -f "${PROJECT_DIR}/scripts/voicekey.desktop" "${APP_DIR}/voicekey.desktop"
    sed -i "s|Exec=voicekey-desktop|Exec=${BIN_DIR}/voicekey-desktop|g" "${APP_DIR}/voicekey.desktop"
    sed -i "s|Exec=voicekey-cli toggle|Exec=${BIN_DIR}/voicekey-cli toggle|g" "${APP_DIR}/voicekey.desktop"
    update-desktop-database "$APP_DIR" 2>/dev/null || true
    log_success "Desktop launcher installed in application menu: VoiceKey"
fi

# 5. Systemd User Service Setup
log_step "5/5 Configuring systemd user service..."
if command -v systemctl >/dev/null 2>&1; then
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
    log_success "Systemd user service installed: ~/.config/systemd/user/voicekey.service"
    log_info "To enable autostart on login: systemctl --user enable voicekey"
fi

# Make sure PATH has ~/.local/bin
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    log_warn "$HOME/.local/bin is not in your current PATH. Consider adding 'export PATH=\"\$HOME/.local/bin:\$PATH\"' to your ~/.bashrc or ~/.zshrc."
fi

echo -e "\n${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}  🎉 VoiceKey is completely installed and ready to use!        ${RESET}"
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "  ${BOLD}Run Desktop App:${RESET}  ${CYAN}voicekey-desktop${RESET} (or launch from Applications Menu)"
echo -e "  ${BOLD}Run Diagnostics:${RESET}  ${CYAN}voicekey-cli doctor${RESET}"
echo -e "  ${BOLD}Interactive Test:${RESET} ${CYAN}voicekey-cli listen -m tiny.en${RESET}"
echo -e "  ${BOLD}Uninstall:${RESET}        ${CYAN}./install.sh --uninstall${RESET}"
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}\n"
