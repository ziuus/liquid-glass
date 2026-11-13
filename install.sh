#!/usr/bin/env bash
set -e

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
CONFIG_DIR="$HOME/.config/liquid-glass"
AUTOSTART_DIR="$HOME/.config/autostart"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

if [ "$1" = "--uninstall" ] || [ "$1" = "uninstall" ]; then
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
if ! command -v cargo &>/dev/null; then
    echo "ERROR: Rust compiler ('cargo') is required but not found in PATH."
    echo "Install Rust via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

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
if [ "$1" = "--systemd" ] || [ "$2" = "--systemd" ]; then
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
if [ "$1" = "--autostart" ] || [ "$2" = "--autostart" ]; then
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
