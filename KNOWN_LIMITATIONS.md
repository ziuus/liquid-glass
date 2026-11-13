# Known Limitations

- **Layer-Shell Compositor Blur**: While `liquid-shell` uses real-time inotify wallpaper synchronization and analytical Snell refraction, full arbitrary-window blurring behind Wayland layer-shell requires compositor extensions (`org_kde_kwin_blur` or wlroots screencopy). On compositors without screencopy, wallpaper-level sampling is used.
- **Platform Scope**: Wayland layer-shell features (`liquid-shell`, `glass-panel`) are Linux-specific. For cross-platform environments (macOS/Windows), use the standard `winit` window runner (`simple-window`).
- **Multi-Monitor Layer Placement**: Current defaults bind to the primary Wayland output; explicit multi-monitor output routing is configured via Hyprland workspace rules or output flags.
