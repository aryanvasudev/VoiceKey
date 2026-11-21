#!/usr/bin/env bash
# ==============================================================================
# VoiceKey - Automated All-In-One Installer for Linux Wayland
# ==============================================================================
# Automates entire setup in one step:
# 1. System packages (wl-clipboard, pipewire, libevdev, notify-send)
# 2. uv package manager & Python dependencies
# 3. Kernel /dev/uinput permissions (udev rules + instant ACL)
# 4. Binary symlinks in ~/.local/bin
# 5. Hardware profiling & auto-tuning (voicekey doctor --apply)
# 6. Global GNOME hotkey setup
# 7. Systemd background daemon service autostart
# 8. End-to-end environment verification
# ==============================================================================

set -euo pipefail

# Text formatting
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
DEFAULT_HOTKEY="<Super>z"
HOTKEY="$DEFAULT_HOTKEY"
ACTION="install"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-sudo)
            SKIP_SUDO=true
            shift
            ;;
        --hotkey)
            HOTKEY="${2:-$DEFAULT_HOTKEY}"
            shift 2
            ;;
        --uninstall|-u)
            ACTION="uninstall"
            shift
            ;;
        --help|-h)
            echo "Usage: ./install.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --hotkey <KEY>    Custom GNOME hotkey combination (default: '<Super>z' and 'F9')"
            echo "  --no-sudo         Skip system package installation and udev rules (non-root)"
            echo "  --uninstall, -u   Remove VoiceKey service, keybinding, and CLI symlinks"
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

# ------------------------------------------------------------------------------
# UNINSTALL LOGIC
# ------------------------------------------------------------------------------
if [[ "$ACTION" == "uninstall" ]]; then
    echo -e "${BOLD}${RED}VoiceKey Uninstaller${RESET}"
    log_step "Stopping and removing systemd user service..."
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user stop voicekey.service 2>/dev/null || true
        systemctl --user disable voicekey.service 2>/dev/null || true
        rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/voicekey.service"
        systemctl --user daemon-reload
        log_success "Systemd user service removed."
    fi

    log_step "Removing GNOME keybinding..."
    if [[ -f "$PROJECT_DIR/scripts/setup_gnome_hotkey.sh" ]]; then
        "$PROJECT_DIR/scripts/setup_gnome_hotkey.sh" --remove 2>/dev/null || true
        log_success "GNOME keybinding removed."
    fi

    log_step "Removing binary symlinks..."
    rm -f "$HOME/.local/bin/voicekey" "$HOME/.local/bin/voicekeyd"
    log_success "Symlinks removed from ~/.local/bin."

    echo -e "\n${BOLD}${GREEN}✔ VoiceKey uninstalled successfully.${RESET}"
    exit 0
fi

# ------------------------------------------------------------------------------
# INSTALLATION
# ------------------------------------------------------------------------------
echo -e "${BOLD}${CYAN}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║             VoiceKey All-in-One Linux Installer               ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${RESET}"

# Check Linux OS
if [[ "$(uname -s)" != "Linux" ]]; then
    log_error "VoiceKey requires Linux with Wayland and PipeWire."
    exit 1
fi

# 1. System Dependencies
log_step "1/7 Checking & installing system prerequisites..."
if [[ "$SKIP_SUDO" == false ]]; then
    if command -v dnf >/dev/null 2>&1; then
        log_info "Detected Fedora / RHEL package manager (dnf)"
        sudo dnf install -y wl-clipboard pipewire-utils libevdev libnotify
        log_success "System packages installed via dnf."
    elif command -v apt-get >/dev/null 2>&1; then
        log_info "Detected Debian / Ubuntu package manager (apt)"
        sudo apt-get update -y
        sudo apt-get install -y wl-clipboard pipewire-bin libevdev2 libnotify-bin
        log_success "System packages installed via apt."
    elif command -v pacman >/dev/null 2>&1; then
        log_info "Detected Arch Linux package manager (pacman)"
        sudo pacman -S --needed --noconfirm wl-clipboard pipewire-utils libevdev libnotify
        log_success "System packages installed via pacman."
    elif command -v zypper >/dev/null 2>&1; then
        log_info "Detected openSUSE package manager (zypper)"
        sudo zypper install -y wl-clipboard pipewire-tools libevdev2 libnotify4
        log_success "System packages installed via zypper."
    else
        log_warn "Could not identify system package manager. Please ensure wl-clipboard, pipewire, libevdev, and libnotify are installed."
    fi
else
    log_info "Skipping system package installation (--no-sudo flag set)."
fi

# 2. Kernel /dev/uinput Permissions
log_step "2/7 Configuring Linux /dev/uinput permissions for kernel auto-paste..."
if [[ "$SKIP_SUDO" == false ]]; then
    # Add user to input group
    sudo usermod -aG input "$USER" 2>/dev/null || true

    # Install udev rule granting uaccess tag (immediate active session access without re-login)
    if [[ -d /etc/udev/rules.d ]]; then
        echo 'KERNEL=="uinput", SUBSYSTEM=="misc", GROUP="input", MODE="0660", TAG+="uaccess", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules >/dev/null
        sudo udevadm control --reload-rules 2>/dev/null || true
        sudo udevadm trigger 2>/dev/null || true
    fi

    # Grant immediate ACL permissions if setfacl is present
    if command -v setfacl >/dev/null 2>&1; then
        sudo setfacl -m u:"$USER":rw /dev/uinput 2>/dev/null || true
    fi
    log_success "/dev/uinput permissions and udev rules configured."
else
    log_info "Skipping /dev/uinput permission configuration (--no-sudo flag set)."
fi

# 3. Fast Python Package Manager (uv)
log_step "3/7 Verifying 'uv' Python package manager..."
if ! command -v uv >/dev/null 2>&1; then
    log_info "Installing astral-sh/uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.local/bin:$PATH"
fi
log_success "'uv' is installed: $(uv --version)"

# 4. Sync Python Dependencies
log_step "4/7 Syncing VoiceKey Python virtual environment..."
cd "$PROJECT_DIR"
uv sync
log_success "Dependencies synced in $PROJECT_DIR/.venv"

# 5. Global CLI Symlinks
log_step "5/7 Installing CLI symlinks into ~/.local/bin..."
mkdir -p "$HOME/.local/bin"
ln -sf "$PROJECT_DIR/.venv/bin/voicekey" "$HOME/.local/bin/voicekey"
ln -sf "$PROJECT_DIR/.venv/bin/voicekeyd" "$HOME/.local/bin/voicekeyd"
export PATH="$HOME/.local/bin:$PATH"
log_success "Symlinked 'voicekey' and 'voicekeyd' to $HOME/.local/bin"

if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    log_warn "$HOME/.local/bin is not in your current PATH. Consider adding 'export PATH=\"\$HOME/.local/bin:\$PATH\"' to your ~/.bashrc or ~/.zshrc."
fi

# 6. Auto-Tune Hardware & Config
log_step "6/7 Initializing configuration & auto-tuning hardware..."
"$PROJECT_DIR/.venv/bin/voicekey" config --init >/dev/null 2>&1 || true
"$PROJECT_DIR/.venv/bin/voicekey" doctor --apply
log_success "Hardware-optimized configuration applied."

# 7. Hotkey & Systemd Autostart
log_step "7/7 Configuring GNOME keybinding & systemd autostart..."

# Setup GNOME keybinding if gsettings is present
if command -v gsettings >/dev/null 2>&1; then
    "$PROJECT_DIR/scripts/setup_gnome_hotkey.sh" "$HOTKEY"
    log_success "GNOME shortcut bound to '$HOTKEY'."
else
    log_info "Desktop environment does not appear to use GNOME gsettings."
    log_info "To bind VoiceKey in Hyprland/Sway/KDE, bind your shortcut to run: voicekey toggle"
fi

# Install and restart systemd service
"$PROJECT_DIR/.venv/bin/voicekey" autostart --install
"$PROJECT_DIR/.venv/bin/voicekey" autostart --start
log_success "VoiceKey background daemon started via systemd user service."

# Verification
echo ""
"$PROJECT_DIR/.venv/bin/python" "$PROJECT_DIR/scripts/verify_installation.py"

echo -e "\n${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}  🎉 VoiceKey is completely installed and ready to use!        ${RESET}"
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "  ${BOLD}Hotkey:${RESET}           ${CYAN}$HOTKEY${RESET} (Press to record, press again to paste)"
echo -e "  ${BOLD}Manual toggle:${RESET}    ${CYAN}voicekey toggle${RESET}"
echo -e "  ${BOLD}Daemon status:${RESET}    ${CYAN}voicekey status${RESET} or ${CYAN}systemctl --user status voicekey${RESET}"
echo -e "  ${BOLD}Settings:${RESET}         ${CYAN}~/.config/voicekey/config.toml${RESET}"
echo -e "  ${BOLD}Uninstall:${RESET}        ${CYAN}./install.sh --uninstall${RESET}"
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}\n"
