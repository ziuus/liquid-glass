#!/usr/bin/env bash
set -e

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
CONFIG_DIR="$HOME/.config/liquid-glass"
AUTOSTART_DIR="$HOME/.config/autostart"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

DRY_RUN=false
CHECK_ONLY=false
INSTALL_SYSTEMD=false
INSTALL_AUTOSTART=false
DO_UNINSTALL=false

show_help() {
    cat << EOF
Liquid Glass & Liquid Shell Turnkey Installer

Usage: $(basename "$0") [OPTIONS]

Options:
  --check        Check all required prerequisites and exit 0 without compiling or installing
  --dry-run      Simulate installation, display planned actions/paths without modifying system
  --systemd      Install and enable systemd user service (liquid-shell.service)
  --autostart    Install XDG autostart desktop entry (liquid-shell.desktop)
  --uninstall    Remove installed binaries, systemd service, and autostart entry
  -h, --help     Show this help message and exit

Environment Variables:
  PREFIX         Installation prefix (default: \$HOME/.local)
EOF
}

# Parse command line options
for arg in "$@"; do
    case "$arg" in
        --check)
            CHECK_ONLY=true
            ;;
        --dry-run)
            DRY_RUN=true
            ;;
        --systemd)
            INSTALL_SYSTEMD=true
            ;;
        --autostart)
            INSTALL_AUTOSTART=true
            ;;
        --uninstall|uninstall)
            DO_UNINSTALL=true
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo "ERROR: Unknown option '$arg'" >&2
            echo "Run '$(basename "$0") --help' for usage." >&2
            exit 1
            ;;
    esac
done

if [ "$DO_UNINSTALL" = true ]; then
    echo "==> Uninstalling liquid-shell..."
    killall -9 liquid-shell 2>/dev/null || true
    systemctl --user stop liquid-shell.service 2>/dev/null || true
    systemctl --user disable liquid-shell.service 2>/dev/null || true
    rm -f "$SYSTEMD_USER_DIR/liquid-shell.service"
    rm -f "$BIN_DIR/liquid-shell"
    rm -f "$BIN_DIR/island-ctl"
    rm -f "$AUTOSTART_DIR/liquid-shell.desktop"
    echo "==> Removed binaries, systemd service, and autostart entry."
    echo "==> Config remains at $CONFIG_DIR (remove manually if desired)."
    exit 0
fi

# Verify dependencies
check_prerequisites() {
    local missing=0
    echo "==> Checking prerequisites..."
    if command -v cargo &>/dev/null; then
        echo "  [OK] cargo: $(cargo --version 2>/dev/null || echo 'installed')"
    else
        echo "  [FAIL] cargo: Rust compiler ('cargo') is required but not found in PATH." >&2
        echo "         Install Rust via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
        missing=$((missing + 1))
    fi

    if [ $missing -gt 0 ]; then
        return 1
    fi
    return 0
}

if [ "$CHECK_ONLY" = true ]; then
    check_prerequisites
    echo "==> All prerequisites satisfied."
    exit 0
fi

if [ "$DRY_RUN" = true ]; then
    echo "==> [DRY-RUN] Simulating Liquid Glass installation..."
    check_prerequisites
    echo "==> [DRY-RUN] Planned installation targets and actions:"
    echo "  Prefix:              $PREFIX"
    echo "  Binary directory:    $BIN_DIR"
    echo "  Binaries:            $BIN_DIR/liquid-shell"
    echo "                       $BIN_DIR/island-ctl"
    echo "  Config directory:    $CONFIG_DIR"
    echo "  Configuration file:  $CONFIG_DIR/config.json"
    if [ "$INSTALL_SYSTEMD" = true ]; then
        echo "  Systemd unit:        $SYSTEMD_USER_DIR/liquid-shell.service (planned: enable and start)"
    fi
    if [ "$INSTALL_AUTOSTART" = true ]; then
        echo "  Desktop entry:       $AUTOSTART_DIR/liquid-shell.desktop (planned: autostart)"
    fi
    echo "==> [DRY-RUN] Dry run complete. Zero files were created or modified."
    exit 0
fi

check_prerequisites

echo "==> Building liquid-shell (release profile)..."
cargo build --release -p liquid-shell

echo "==> Installing binaries to $BIN_DIR..."
mkdir -p "$BIN_DIR"
install -Dm755 "target/release/liquid-shell" "$BIN_DIR/liquid-shell"
install -Dm755 "bin/island-ctl" "$BIN_DIR/island-ctl"

echo "==> Ensuring configuration directory at $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.json" ]; then
    cat << 'EOF' > "$CONFIG_DIR/config.json"
{
  "theme_mode": 0,
  "style_mode": 2.0,
  "blur_radius": 0.0,
  "animation_speed": 1.0,
  "clock": {
    "enabled": true,
    "style_mode": 2.0,
    "blur_radius": 0.0,
    "animation_speed": 1.0
  },
  "island": {
    "edge_style": "floating",
    "fillet_radius_compact": 10.0,
    "fillet_radius_expanded": 24.0,
    "blur_radius": 0.0,
    "style_mode": 2.0,
    "hover_to_expand": true,
    "animation_speed": 1.0,
    "quick_access": [
      { "name": "Terminal", "command": "foot", "icon": "terminal" },
      { "name": "Browser", "command": "brave", "icon": "browser" },
      { "name": "Editor", "command": "code", "icon": "editor" },
      { "name": "Files", "command": "thunar", "icon": "files" },
      { "name": "Music", "command": "spotify", "icon": "music" }
    ]
  },
  "dock": {
    "enabled": true,
    "hover_to_expand": true,
    "style_mode": 2.0,
    "blur_radius": 0.0,
    "animation_speed": 1.0,
    "apps": [
      { "name": "Terminal", "command": "foot", "icon": "terminal", "class": "foot" },
      { "name": "Browser", "command": "brave", "icon": "browser", "class": "brave" },
      { "name": "Editor", "command": "code", "icon": "editor", "class": "code" },
      { "name": "Files", "command": "thunar", "icon": "folder", "class": "thunar" },
      { "name": "Music", "command": "spotify", "icon": "music", "class": "spotify" }
    ]
  },
  "todo": {
    "enabled": true,
    "style_mode": 2.0,
    "blur_radius": 0.0,
    "animation_speed": 1.0
  }
}
EOF
    echo "==> Created default config at $CONFIG_DIR/config.json"
else
    echo "==> Preserved existing config at $CONFIG_DIR/config.json"
fi

# Optional systemd service installation
if [ "$INSTALL_SYSTEMD" = true ]; then
    mkdir -p "$SYSTEMD_USER_DIR"
    cat << EOF > "$SYSTEMD_USER_DIR/liquid-shell.service"
[Unit]
Description=Liquid Glass UI Shell for Wayland
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=$BIN_DIR/liquid-shell
Restart=on-failure
RestartSec=1

[Install]
WantedBy=graphical-session.target
EOF
    systemctl --user daemon-reload
    systemctl --user enable --now liquid-shell.service
    echo "==> Systemd service installed and enabled (liquid-shell.service)"
fi

# Optional XDG autostart entry
if [ "$INSTALL_AUTOSTART" = true ]; then
    mkdir -p "$AUTOSTART_DIR"
    cat << EOF > "$AUTOSTART_DIR/liquid-shell.desktop"
[Desktop Entry]
Type=Application
Name=Liquid Shell
Comment=Physical optical lensing and glassmorphism UI engine for Wayland
Exec=$BIN_DIR/liquid-shell
Terminal=false
Categories=Utility;System;
EOF
    echo "==> Autostart entry installed to $AUTOSTART_DIR/liquid-shell.desktop"
fi

echo ""
echo "Liquid Glass installation complete!"
echo "Binary:  $BIN_DIR/liquid-shell"
echo "CLI:     $BIN_DIR/island-ctl (e.g. 'island-ctl toggle')"
echo "Config:  $CONFIG_DIR/config.json"
echo "Control: /tmp/liquid_island.fifo"
echo ""
echo "To launch now:       liquid-shell &"
echo "To control:          island-ctl toggle"
echo "To install systemd:  ./install.sh --systemd"
echo "To autostart Hyprland: add to ~/.config/hypr/hyprland.conf:"
echo "  exec-once = liquid-shell"
