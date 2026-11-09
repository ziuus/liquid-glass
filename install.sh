#!/usr/bin/env bash
set -e

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
CONFIG_DIR="$HOME/.config/liquid-glass"
AUTOSTART_DIR="$HOME/.config/autostart"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

if [ "$1" = "--uninstall" ] || [ "$1" = "uninstall" ]; then
    echo "==> Uninstalling liquid-shell..."
    killall liquid-shell 2>/dev/null || true
    rm -f "$BIN_DIR/liquid-shell"
    rm -f "$AUTOSTART_DIR/liquid-shell.desktop"
    echo "==> Removed binary and autostart entry."
    echo "==> Config remains at $CONFIG_DIR (remove manually if desired)."
    exit 0
fi

echo "==> Building liquid-shell (release profile)..."
cargo build --release -p liquid-shell

echo "==> Installing binary to $BIN_DIR/liquid-shell..."
mkdir -p "$BIN_DIR"
install -Dm755 "target/release/liquid-shell" "$BIN_DIR/liquid-shell"

echo "==> Ensuring configuration directory at $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.json" ]; then
    cat << 'EOF' > "$CONFIG_DIR/config.json"
{
  "edge_style": "curved",
  "fillet_radius_compact": 10.0,
  "fillet_radius_expanded": 24.0,
  "style_mode": 2.0,
  "theme_mode": 0,
  "blur_radius": 8.0,
  "quick_access": [
    { "name": "Terminal", "command": "foot", "icon": "terminal" },
    { "name": "Browser", "command": "brave", "icon": "browser" },
    { "name": "Editor", "command": "code", "icon": "editor" },
    { "name": "Files", "command": "thunar", "icon": "files" },
    { "name": "Music", "command": "spotify", "icon": "music" }
  ]
}
EOF
    echo "==> Created default config at $CONFIG_DIR/config.json"
else
    echo "==> Preserved existing config at $CONFIG_DIR/config.json"
fi

# Optional autostart entry
if [ "$1" = "--autostart" ]; then
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
echo "Config:  $CONFIG_DIR/config.json"
echo "Control: /tmp/liquid_island.fifo"
echo ""
echo "To launch: liquid-shell &"
echo "To autostart in Hyprland, add to ~/.config/hypr/hyprland.conf:"
echo "  exec-once = liquid-shell"
