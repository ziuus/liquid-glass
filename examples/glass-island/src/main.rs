use glass_core::font::{FontWeight, UIFont};
use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material};
use glass_platform::WallpaperManager;
mod capture;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::delegate_shm;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_client::{
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

const FIFO_PATH: &str = "/tmp/liquid_island.fifo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandMode {
    Hidden,
    CompactPill,
    MusicExpanded,
    Notification,
    QuickAccess,
}

#[derive(Debug, Clone)]
pub struct NotificationData {
    pub app: String,
    pub summary: String,
    pub body: String,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct MusicState {
    pub title: String,
    pub artist: String,
    pub is_playing: bool,
    pub progress: f32,
    pub duration_sec: f32,
    pub elapsed_sec: f32,
}

pub enum IslandIpcEvent {
    Toggle,
    ShowMode(IslandMode),
    Notification(NotificationData),
}

pub struct AppData {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    screencopy: capture::ScreencopyState,
    seat_state: SeatState,

    layer_surface: Option<LayerSurface>,
    renderer: Option<GlassRenderer>,
    window_wrapper: Option<Arc<WaylandWindow>>,

    pointer: Option<wl_pointer::WlPointer>,
    cursor_pos: [f32; 2],
    is_pressed: bool,
    is_hovered: bool,

    // Dynamic Island geometry & physics
    current_w: f32,
    target_w: f32,
    vel_w: f32,

    current_h: f32,
    target_h: f32,
    vel_h: f32,

    current_alpha: f32,
    target_alpha: f32,
    vel_alpha: f32,

    current_y: f32,
    target_y: f32,
    vel_y: f32,

    mode: IslandMode,
    prev_mode: IslandMode,

    music: MusicState,
    last_music_poll: Instant,

    active_notif: Option<NotificationData>,
    notif_timeout_secs: f32,

    // IPC receiver
    ipc_rx: Receiver<IslandIpcEvent>,

    // Typography & UI overlay
    font: UIFont,
    overlay_buffer: Vec<u8>,

    // Frame management
    width: u32,
    height: u32,
    pos_x: i32,
    pos_y: i32,
    first_configure: bool,
    animating: bool,

    wallpaper_mgr: WallpaperManager,
    primary_output: Option<wl_output::WlOutput>,
    is_wallpaper_light: bool,
    last_desktop_grab: Instant,
}

struct WaylandWindow {
    surface: wl_surface::WlSurface,
    display: *mut std::ffi::c_void,
}

unsafe impl Send for WaylandWindow {}
unsafe impl Sync for WaylandWindow {}

impl HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = WaylandWindowHandle::new(
            std::ptr::NonNull::new(self.surface.id().as_ptr() as *mut _).unwrap(),
        );
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::Wayland(raw))) }
    }
}

impl HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let raw = WaylandDisplayHandle::new(std::ptr::NonNull::new(self.display).unwrap());
        unsafe { Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(raw))) }
    }
}

fn capture_desktop_under_island(x: i32, y: i32, width: u32, height: u32) -> Option<Vec<u8>> {
    let geom = format!("{},{} {}x{}", x, y, width, height);
    let output = Command::new("grim")
        .args(["-g", &geom, "-t", "ppm", "-"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let data = output.stdout;
    let header_marker = b"255\n";
    let pos = data.windows(4).position(|w| w == header_marker)?;
    let raw_rgb = &data[pos + 4..];
    let expected_rgb_len = (width * height * 3) as usize;
    if raw_rgb.len() < expected_rgb_len {
        return None;
    }

    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for chunk in raw_rgb[..expected_rgb_len].as_chunks::<3>().0 {
        rgba.push(chunk[0]);
        rgba.push(chunk[1]);
        rgba.push(chunk[2]);
        rgba.push(255);
    }
    Some(rgba)
}

impl AppData {
    fn set_mode(&mut self, new_mode: IslandMode) {
        if self.mode == new_mode {
            return;
        }
        self.prev_mode = self.mode;
        self.mode = new_mode;

        match new_mode {
            IslandMode::Hidden => {
                self.target_w = 180.0;
                self.target_h = 10.0;
                self.target_y = -50.0;
                self.target_alpha = 0.0;
            }
            IslandMode::CompactPill => {
                self.target_w = 210.0;
                self.target_h = 42.0;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
            IslandMode::MusicExpanded => {
                self.target_w = 440.0;
                self.target_h = 130.0;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
            IslandMode::Notification => {
                self.target_w = 460.0;
                self.target_h = 92.0;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
            IslandMode::QuickAccess => {
                self.target_w = 480.0;
                self.target_h = 110.0;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
        }
        self.animating = true;
    }

    fn toggle(&mut self) {
        match self.mode {
            IslandMode::Hidden => self.set_mode(IslandMode::CompactPill),
            IslandMode::CompactPill => self.set_mode(IslandMode::QuickAccess),
            _ => self.set_mode(IslandMode::CompactPill),
        }
    }

    fn check_ipc(&mut self) {
        while let Ok(evt) = self.ipc_rx.try_recv() {
            match evt {
                IslandIpcEvent::Toggle => self.toggle(),
                IslandIpcEvent::ShowMode(m) => self.set_mode(m),
                IslandIpcEvent::Notification(notif) => {
                    self.active_notif = Some(notif);
                    self.set_mode(IslandMode::Notification);
                }
            }
        }
    }

    fn update_physics(&mut self, dt: f32) -> bool {
        let spring_k = 320.0f32;
        let damping = 26.0f32;

        let force_w = -spring_k * (self.current_w - self.target_w) - damping * self.vel_w;
        self.vel_w += force_w * dt;
        self.current_w += self.vel_w * dt;

        let force_h = -spring_k * (self.current_h - self.target_h) - damping * self.vel_h;
        self.vel_h += force_h * dt;
        self.current_h += self.vel_h * dt;

        let force_y = -spring_k * (self.current_y - self.target_y) - damping * self.vel_y;
        self.vel_y += force_y * dt;
        self.current_y += self.vel_y * dt;

        let force_a =
            -spring_k * (self.current_alpha - self.target_alpha) - damping * self.vel_alpha;
        self.vel_alpha += force_a * dt;
        self.current_alpha = (self.current_alpha + self.vel_alpha * dt).clamp(0.0, 1.0);

        let settling = (self.current_w - self.target_w).abs() < 0.5
            && (self.current_h - self.target_h).abs() < 0.5
            && (self.current_y - self.target_y).abs() < 0.5
            && self.vel_w.abs() < 1.0
            && self.vel_h.abs() < 1.0
            && self.vel_y.abs() < 1.0;

        if settling {
            self.current_w = self.target_w;
            self.current_h = self.target_h;
            self.current_y = self.target_y;
            self.vel_w = 0.0;
            self.vel_h = 0.0;
            self.vel_y = 0.0;
        }

        // Notification expiration timer
        if self.mode == IslandMode::Notification {
            if let Some(notif) = &self.active_notif {
                if notif.timestamp.elapsed().as_secs_f32() > self.notif_timeout_secs {
                    self.active_notif = None;
                    self.set_mode(IslandMode::CompactPill);
                }
            }
        }

        // Advance live/simulated music playback
        if self.music.is_playing {
            self.music.elapsed_sec += dt;
            if self.music.elapsed_sec >= self.music.duration_sec {
                self.music.elapsed_sec = 0.0;
            }
            self.music.progress =
                (self.music.elapsed_sec / self.music.duration_sec).clamp(0.0, 1.0);
        }

        !settling || self.music.is_playing || self.is_hovered
    }

    fn check_music(&mut self) {
        if self.last_music_poll.elapsed() < Duration::from_millis(1500) {
            return;
        }
        self.last_music_poll = Instant::now();

        // Query playerctl if available
        if let Ok(out) = Command::new("playerctl").args(["status"]).output() {
            if out.status.success() {
                let status_str = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                self.music.is_playing = status_str.contains("playing");

                if let Ok(meta_out) = Command::new("playerctl")
                    .args([
                        "metadata",
                        "--format",
                        "{{title}}||{{artist}}||{{mpris:length}}",
                    ])
                    .output()
                {
                    if meta_out.status.success() {
                        let text = String::from_utf8_lossy(&meta_out.stdout);
                        let parts: Vec<&str> = text.trim().split("||").collect();
                        if !parts.is_empty() && !parts[0].is_empty() {
                            self.music.title = parts[0].to_string();
                        }
                        if parts.len() > 1 && !parts[1].is_empty() {
                            self.music.artist = parts[1].to_string();
                        }
                        if parts.len() > 2 {
                            if let Ok(len_us) = parts[2].parse::<f32>() {
                                if len_us > 1000.0 {
                                    self.music.duration_sec = len_us / 1_000_000.0;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_desktop_backdrop(&mut self) {
        // Sample real desktop surface under the island every 800ms (or on demand)
        // so if there is a dark code editor or browser underneath, it refracts THAT window,
        // eliminating the double-transparent wallpaper bug permanently!
        if self.last_desktop_grab.elapsed() > Duration::from_millis(800) {
            self.last_desktop_grab = Instant::now();
            if let Some(desktop_rgba) =
                capture_desktop_under_island(self.pos_x, self.pos_y, self.width, self.height)
            {
                self.is_wallpaper_light = is_buffer_light(&desktop_rgba);
                if let Some(r) = &mut self.renderer {
                    r.update_background(self.width, self.height, &desktop_rgba);
                }
                return;
            }
        }

        // Fallback to wallpaper update if grim fails
        if self.wallpaper_mgr.check_update() {
            if let (Some(cache), Some(renderer)) =
                (self.wallpaper_mgr.cached_buffer(), &mut self.renderer)
            {
                let crop = WallpaperManager::get_wallpaper_crop(
                    cache,
                    self.pos_x as u32,
                    self.pos_y as u32,
                    self.width,
                    self.height,
                );
                self.is_wallpaper_light = is_buffer_light(&crop);
                renderer.update_background(self.width, self.height, &crop);
            }
        }
    }

    fn render_ui_overlay(&mut self) {
        let buf = &mut self.overlay_buffer;
        buf.fill(0);

        if self.current_alpha <= 0.05 {
            return;
        }

        let is_light = self.is_wallpaper_light;
        let col_ink = if is_light {
            [8, 12, 20, 255]
        } else {
            [255, 255, 255, 255]
        };
        let col_dim = if is_light {
            [60, 70, 85, 220]
        } else {
            [180, 190, 210, 220]
        };
        let col_accent = [90, 155, 255, 255];
        let col_green = [70, 215, 120, 255];
        let col_shadow = [0, 0, 0, 0];

        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let island_w = self.current_w;
        let island_h = self.current_h;
        let island_cy = island_top + island_h * 0.5;

        match self.mode {
            IslandMode::CompactPill => {
                // Left: Rotating Vinyl Disc icon with pulsing glow when playing
                let disc_cx = island_cx - island_w * 0.5 + 24.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    island_cy,
                    22.0,
                    22.0,
                    11.0,
                    if self.music.is_playing {
                        col_green
                    } else {
                        col_dim
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    if self.music.is_playing { "♫" } else { "●" },
                    disc_cx,
                    island_cy,
                    13.0,
                    [10, 14, 20, 255],
                    col_shadow,
                    FontWeight::Bold,
                );

                // Center: Track Title
                let title = if self.music.title.len() > 14 {
                    format!("{}...", &self.music.title[..12])
                } else {
                    self.music.title.clone()
                };
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    &title,
                    island_cx + 4.0,
                    island_cy,
                    13.5,
                    col_ink,
                    col_shadow,
                    FontWeight::Bold,
                );

                // Right: Mini waveform bars
                let right_x = island_cx + island_w * 0.5 - 28.0;
                for i in 0..3 {
                    let bx = right_x + (i as f32) * 6.0;
                    let h = if self.music.is_playing {
                        10.0 + (i as f32 * 4.0)
                    } else {
                        4.0
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        island_cy,
                        3.0,
                        h,
                        1.5,
                        col_green,
                    );
                }
            }
            IslandMode::MusicExpanded => {
                // Header: NOW PLAYING
                let header_y = island_top + 22.0;
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    "NOW PLAYING",
                    island_cx - island_w * 0.5 + 32.0,
                    header_y,
                    11.0,
                    col_dim,
                    FontWeight::Bold,
                );

                // Vinyl Album Disc
                let disc_cx = island_cx - island_w * 0.5 + 50.0;
                let disc_cy = island_top + 68.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    disc_cy,
                    46.0,
                    46.0,
                    23.0,
                    if is_light {
                        [25, 30, 42, 255]
                    } else {
                        [235, 240, 252, 255]
                    },
                );
                // Center accent core
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    disc_cy,
                    18.0,
                    18.0,
                    9.0,
                    col_green,
                );

                // Track details
                let title_x = island_cx - island_w * 0.5 + 88.0;
                let title = if self.music.title.len() > 20 {
                    format!("{}...", &self.music.title[..18])
                } else {
                    self.music.title.clone()
                };
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    &title,
                    title_x,
                    island_top + 58.0,
                    16.0,
                    col_ink,
                    FontWeight::Bold,
                );
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    &self.music.artist,
                    title_x,
                    island_top + 78.0,
                    13.0,
                    col_dim,
                    FontWeight::Medium,
                );

                // Transport Controls: Prev, Play/Pause, Next
                let ctrl_y = island_top + 68.0;
                let play_cx = island_cx + island_w * 0.5 - 64.0;
                let prev_cx = play_cx - 44.0;
                let next_cx = play_cx + 44.0;

                // Previous
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    "⏮",
                    prev_cx,
                    ctrl_y,
                    16.0,
                    col_ink,
                    col_shadow,
                    FontWeight::Bold,
                );

                // Play / Pause Button with circle fill
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    play_cx,
                    ctrl_y,
                    36.0,
                    36.0,
                    18.0,
                    if is_light {
                        [20, 24, 34, 255]
                    } else {
                        [245, 248, 255, 255]
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    if self.music.is_playing { "⏸" } else { "▶" },
                    play_cx,
                    ctrl_y,
                    15.0,
                    if is_light {
                        [255, 255, 255, 255]
                    } else {
                        [10, 14, 20, 255]
                    },
                    col_shadow,
                    FontWeight::Bold,
                );

                // Next
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    "⏭",
                    next_cx,
                    ctrl_y,
                    16.0,
                    col_ink,
                    col_shadow,
                    FontWeight::Bold,
                );

                // Progress Bar
                let bar_y = island_top + 108.0;
                let bar_w = island_w - 64.0;
                let bar_x = island_cx;
                // Track back
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    bar_x,
                    bar_y,
                    bar_w,
                    4.0,
                    2.0,
                    if is_light {
                        [180, 185, 200, 150]
                    } else {
                        [80, 90, 110, 150]
                    },
                );
                // Active fill
                let fill_w = bar_w * self.music.progress;
                if fill_w > 2.0 {
                    let fill_cx = island_cx - bar_w * 0.5 + fill_w * 0.5;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        fill_cx,
                        bar_y,
                        fill_w,
                        4.0,
                        2.0,
                        col_accent,
                    );
                }
            }
            IslandMode::Notification => {
                if let Some(notif) = &self.active_notif {
                    // Left App Icon badge
                    let badge_cx = island_cx - island_w * 0.5 + 42.0;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        badge_cx,
                        island_cy,
                        40.0,
                        40.0,
                        14.0,
                        col_accent,
                    );
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        "🔔",
                        badge_cx,
                        island_cy,
                        18.0,
                        [255, 255, 255, 255],
                        col_shadow,
                        FontWeight::Bold,
                    );

                    // App Name & Time
                    let content_x = island_cx - island_w * 0.5 + 76.0;
                    self.font.draw_text(
                        buf,
                        self.width,
                        self.height,
                        &notif.app.to_uppercase(),
                        content_x,
                        island_top + 28.0,
                        10.5,
                        col_accent,
                        FontWeight::Bold,
                    );

                    // Summary
                    let summary = if notif.summary.len() > 34 {
                        format!("{}...", &notif.summary[..32])
                    } else {
                        notif.summary.clone()
                    };
                    self.font.draw_text(
                        buf,
                        self.width,
                        self.height,
                        &summary,
                        content_x,
                        island_top + 48.0,
                        15.0,
                        col_ink,
                        FontWeight::Bold,
                    );

                    // Body
                    let body = if notif.body.len() > 42 {
                        format!("{}...", &notif.body[..40])
                    } else {
                        notif.body.clone()
                    };
                    self.font.draw_text(
                        buf,
                        self.width,
                        self.height,
                        &body,
                        content_x,
                        island_top + 68.0,
                        13.0,
                        col_dim,
                        FontWeight::Medium,
                    );
                }
            }
            IslandMode::QuickAccess => {
                // Header
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    "QUICK ACCESS",
                    island_cx - island_w * 0.5 + 32.0,
                    island_top + 24.0,
                    11.0,
                    col_dim,
                    FontWeight::Bold,
                );

                // App launcher shortcuts
                let apps = [
                    ("Terminal", "💻"),
                    ("Browser", "🌐"),
                    ("Editor", "📝"),
                    ("Files", "📁"),
                    ("Music", "🎵"),
                ];

                let item_w = (island_w - 50.0) / (apps.len() as f32);
                let start_x = island_cx - island_w * 0.5 + 25.0 + item_w * 0.5;

                for (idx, (name, icon)) in apps.iter().enumerate() {
                    let cx = start_x + (idx as f32) * item_w;
                    let cy = island_top + 62.0;

                    // Container pill
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        cx,
                        cy,
                        item_w - 10.0,
                        50.0,
                        12.0,
                        if is_light {
                            [240, 243, 250, 160]
                        } else {
                            [45, 52, 70, 160]
                        },
                    );

                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        icon,
                        cx,
                        cy - 8.0,
                        16.0,
                        col_ink,
                        col_shadow,
                        FontWeight::Bold,
                    );
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        name,
                        cx,
                        cy + 12.0,
                        10.5,
                        col_dim,
                        col_shadow,
                        FontWeight::Medium,
                    );
                }
            }
            IslandMode::Hidden => {}
        }
    }

    fn draw(&mut self) -> bool {
        self.check_ipc();
        self.check_music();
        self.check_desktop_backdrop();

        self.render_ui_overlay();

        if let Some(renderer) = &mut self.renderer {
            if self.current_alpha <= 0.01 {
                let spec: [f32; 4] = [0.5, 0.0, 0.01, 0.01];
                let island = GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::Capsule)
                    .bounds(spec[0], spec[1], spec[2], spec[3]);
                let _ = renderer.render(&[island]);
                return false;
            }

            renderer.update_overlay(self.width, self.height, &self.overlay_buffer);

            // Bounds normalized relative to canvas [x, y, w, h]
            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);
            let x_norm = 0.50 - w_norm * 0.5;
            let y_norm = self.current_y / (self.height as f32);

            let is_light = if self.is_wallpaper_light { 1.0 } else { 0.0 };

            // Pure Liquid Glass with element_type = 25.0 (Clean canvas with zero hardcoded shader overlaps)
            let island = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(0.06)
                .bevel(12.0)
                .refraction(0.55)
                .dispersion(0.02)
                .blur(8.0)
                .specular(0.85)
                .fresnel(0.60)
                .tint(
                    if self.music.is_playing { 1.0 } else { 0.0 },
                    if self.is_pressed { 1.0 } else { 0.0 },
                    self.current_alpha,
                    is_light,
                )
                .element_type(25.0)
                .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);

            let _ = renderer.render(&[island]);
            self.animating
        } else {
            false
        }
    }

    fn handle_click(&mut self, lx: f32, ly: f32) {
        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let island_w = self.current_w;
        let island_h = self.current_h;

        // Check if click was inside current island capsule
        if lx < island_cx - island_w * 0.5
            || lx > island_cx + island_w * 0.5
            || ly < island_top
            || ly > island_top + island_h
        {
            return;
        }

        match self.mode {
            IslandMode::CompactPill => {
                self.set_mode(IslandMode::MusicExpanded);
            }
            IslandMode::MusicExpanded => {
                let ctrl_y = island_top + 68.0;
                let play_cx = island_cx + island_w * 0.5 - 64.0;
                let prev_cx = play_cx - 44.0;
                let next_cx = play_cx + 44.0;

                let d_play = ((lx - play_cx).powi(2) + (ly - ctrl_y).powi(2)).sqrt();
                let d_prev = ((lx - prev_cx).powi(2) + (ly - ctrl_y).powi(2)).sqrt();
                let d_next = ((lx - next_cx).powi(2) + (ly - ctrl_y).powi(2)).sqrt();

                if d_play <= 22.0 {
                    let _ = Command::new("playerctl").args(["play-pause"]).spawn();
                    self.music.is_playing = !self.music.is_playing;
                } else if d_prev <= 20.0 {
                    let _ = Command::new("playerctl").args(["previous"]).spawn();
                } else if d_next <= 20.0 {
                    let _ = Command::new("playerctl").args(["next"]).spawn();
                } else if ly > island_top + 80.0 {
                    let bar_w = island_w - 64.0;
                    let bar_left = island_cx - bar_w * 0.5;
                    let ratio = ((lx - bar_left) / bar_w).clamp(0.0, 1.0);
                    self.music.progress = ratio;
                    self.music.elapsed_sec = ratio * self.music.duration_sec;
                    let pos_str = format!("{:.1}", self.music.elapsed_sec);
                    let _ = Command::new("playerctl")
                        .args(["position", &pos_str])
                        .spawn();
                } else if lx < island_cx - island_w * 0.5 + 80.0 {
                    self.set_mode(IslandMode::CompactPill);
                }
            }
            IslandMode::Notification => {
                self.active_notif = None;
                self.set_mode(IslandMode::CompactPill);
            }
            IslandMode::QuickAccess => {
                let apps = [
                    ("foot", "Terminal"),
                    ("brave", "Browser"),
                    ("code", "Editor"),
                    ("thunar", "Files"),
                    ("spotify", "Music"),
                ];
                let item_w = (island_w - 50.0) / (apps.len() as f32);
                let start_x = island_cx - island_w * 0.5 + 25.0 + item_w * 0.5;

                for (idx, (cmd, _name)) in apps.iter().enumerate() {
                    let cx = start_x + (idx as f32) * item_w;
                    let cy = island_top + 62.0;
                    if (lx - cx).abs() <= item_w * 0.45 && (ly - cy).abs() <= 25.0 {
                        let _ = Command::new(cmd).spawn();
                        self.set_mode(IslandMode::CompactPill);
                        break;
                    }
                }
            }
            IslandMode::Hidden => {}
        }
    }
}

fn is_buffer_light(buf: &[u8]) -> bool {
    let mut total_lum = 0.0f64;
    let mut count = 0usize;
    let step = 16 * 4;
    for i in (0..buf.len()).step_by(step) {
        if i + 2 < buf.len() {
            let r = buf[i] as f64 / 255.0;
            let g = buf[i + 1] as f64 / 255.0;
            let b = buf[i + 2] as f64 / 255.0;
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            total_lum += lum;
            count += 1;
        }
    }
    if count > 0 {
        (total_lum / count as f64) > 0.179
    } else {
        false
    }
}

// ── SCTK Handlers ──

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_scale: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.primary_output.is_none() {
            self.primary_output = Some(output);
        }
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }

        if self.renderer.is_none() {
            if let Some(window) = &self.window_wrapper {
                let mut renderer =
                    pollster::block_on(GlassRenderer::new(window.clone(), self.width, self.height))
                        .expect("Failed to initialize GlassRenderer for Dynamic Island");

                if let Some(desktop_rgba) =
                    capture_desktop_under_island(self.pos_x, self.pos_y, self.width, self.height)
                {
                    self.is_wallpaper_light = is_buffer_light(&desktop_rgba);
                    renderer.update_background(self.width, self.height, &desktop_rgba);
                } else if let Some(cache) = self.wallpaper_mgr.cached_buffer() {
                    let crop = WallpaperManager::get_wallpaper_crop(
                        cache,
                        self.pos_x as u32,
                        self.pos_y as u32,
                        self.width,
                        self.height,
                    );
                    self.is_wallpaper_light = is_buffer_light(&crop);
                    renderer.update_background(self.width, self.height, &crop);
                }

                self.overlay_buffer = vec![0u8; (self.width * self.height * 4) as usize];
                self.renderer = Some(renderer);
            }
        }

        self.first_configure = false;
        self.draw();
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }
    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }
    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl PointerHandler for AppData {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.is_hovered = true;
                    self.cursor_pos = [
                        (event.position.0 as f32) / (self.width as f32),
                        (event.position.1 as f32) / (self.height as f32),
                    ];
                }
                PointerEventKind::Leave { .. } => {
                    self.is_hovered = false;
                    self.is_pressed = false;
                    self.cursor_pos = [-1.0, -1.0];
                }
                PointerEventKind::Motion { .. } => {
                    self.cursor_pos = [
                        (event.position.0 as f32) / (self.width as f32),
                        (event.position.1 as f32) / (self.height as f32),
                    ];
                }
                PointerEventKind::Press { .. } => {
                    self.is_pressed = true;
                    let lx = event.position.0 as f32;
                    let ly = event.position.1 as f32;
                    self.handle_click(lx, ly);
                }
                PointerEventKind::Release { .. } => {
                    self.is_pressed = false;
                }
                PointerEventKind::Axis { .. } => {}
            }
        }
    }
}

impl ShmHandler for AppData {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(AppData);
delegate_output!(AppData);
delegate_layer!(AppData);
delegate_registry!(AppData);
delegate_shm!(AppData);
delegate_seat!(AppData);
delegate_pointer!(AppData);

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn start_fifo_listener(tx: Sender<IslandIpcEvent>) {
    std::thread::spawn(move || {
        let _ = std::fs::remove_file(FIFO_PATH);
        unsafe {
            let c_path = std::ffi::CString::new(FIFO_PATH).unwrap();
            libc::mkfifo(c_path.as_ptr(), 0o666);
        }

        loop {
            if let Ok(mut file) = OpenOptions::new().read(true).open(FIFO_PATH) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed == "toggle" {
                            let _ = tx.send(IslandIpcEvent::Toggle);
                        } else if trimmed == "hide" {
                            let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::Hidden));
                        } else if trimmed == "compact" {
                            let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::CompactPill));
                        } else if trimmed == "music" {
                            let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::MusicExpanded));
                        } else if trimmed == "apps" || trimmed == "quick" {
                            let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::QuickAccess));
                        } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            if val.get("type").and_then(|t| t.as_str()) == Some("notification") {
                                let app = val
                                    .get("app")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("System")
                                    .to_string();
                                let summary = val
                                    .get("summary")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let body = val
                                    .get("body")
                                    .and_then(|b| b.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let _ = tx.send(IslandIpcEvent::Notification(NotificationData {
                                    app,
                                    summary,
                                    body,
                                    timestamp: Instant::now(),
                                }));
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

fn start_dbus_monitor() {
    std::thread::spawn(|| {
        let script = r#"
import dbus
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib
import json
import os
import time

FIFO_PATH = "/tmp/liquid_island.fifo"

for _ in range(50):
    if os.path.exists(FIFO_PATH):
        break
    time.sleep(0.1)

DBusGMainLoop(set_as_default=True)
try:
    bus = dbus.SessionBus()
    bus_obj = bus.get_object('org.freedesktop.DBus', '/org/freedesktop/DBus')
    bus_iface = dbus.Interface(bus_obj, 'org.freedesktop.DBus.Monitoring')
    bus_iface.BecomeMonitor(["type='method_call',interface='org.freedesktop.Notifications',member='Notify'"], 0)

    def msg_handler(bus, msg):
        if msg.get_member() == 'Notify':
            args = msg.get_args_list()
            app_name = str(args[0]) if len(args) > 0 else 'System'
            summary = str(args[3]) if len(args) > 3 else ''
            body = str(args[4]) if len(args) > 4 else ''
            payload = json.dumps({"type": "notification", "app": app_name, "summary": summary, "body": body})
            try:
                if os.path.exists(FIFO_PATH):
                    with open(FIFO_PATH, "a") as f:
                        f.write(payload + "\n")
            except Exception:
                pass

    bus.add_message_filter(msg_handler)
    GLib.MainLoop().run()
except Exception:
    pass
"#;
        let child = Command::new("python3").arg("-c").arg(script).spawn();
        if let Ok(mut c) = child {
            let _ = c.wait();
        }
    });
}

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();
    let display = conn.display();
    let display_ptr = display.id().as_ptr() as *mut std::ffi::c_void;

    let (globals, mut event_queue) =
        wayland_client::globals::registry_queue_init::<AppData>(&conn).unwrap();
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let shm = Shm::bind(&globals, &qh).expect("wl_shm missing");
    let screencopy = capture::ScreencopyState::new(&registry_state, &qh, &shm);
    let seat_state = SeatState::new(&globals, &qh);

    let (tx, rx) = channel();
    start_fifo_listener(tx);
    start_dbus_monitor();

    let wallpaper_mgr = WallpaperManager::new();

    // Canvas size: 540px wide x 160px high at Top Center
    let screen_w: i32 = 1920;
    let width: u32 = 540;
    let height: u32 = 160;
    let pos_x: i32 = (screen_w - width as i32) / 2;
    let pos_y: i32 = 0;

    let mut app_data = AppData {
        registry_state,
        output_state: OutputState::new(&globals, &qh),
        compositor_state: CompositorState::bind(&globals, &qh).expect("wl_compositor missing"),
        layer_shell: LayerShell::bind(&globals, &qh).expect("wlr layer shell missing"),
        shm,
        screencopy,
        seat_state,
        layer_surface: None,
        renderer: None,
        window_wrapper: None,
        pointer: None,
        cursor_pos: [-1.0, -1.0],
        is_pressed: false,
        is_hovered: false,

        current_w: 210.0,
        target_w: 210.0,
        vel_w: 0.0,

        current_h: 42.0,
        target_h: 42.0,
        vel_h: 0.0,

        current_alpha: 1.0,
        target_alpha: 1.0,
        vel_alpha: 0.0,

        current_y: 12.0,
        target_y: 12.0,
        vel_y: 0.0,

        mode: IslandMode::CompactPill,
        prev_mode: IslandMode::CompactPill,

        music: MusicState {
            title: "Starboy".to_string(),
            artist: "The Weeknd".to_string(),
            is_playing: true,
            progress: 0.42,
            duration_sec: 230.0,
            elapsed_sec: 96.6,
        },
        last_music_poll: Instant::now() - Duration::from_secs(5),

        active_notif: None,
        notif_timeout_secs: 5.0,

        ipc_rx: rx,
        font: UIFont::new(),
        overlay_buffer: Vec::new(),

        width,
        height,
        pos_x,
        pos_y,
        first_configure: true,
        animating: true,
        wallpaper_mgr,
        primary_output: None,
        is_wallpaper_light: false,
        last_desktop_grab: Instant::now() - Duration::from_secs(10),
    };

    let surface = app_data.compositor_state.create_surface(&qh);

    // Dynamic Island layer shell surface: Layer::Overlay (always floats above apps)
    let layer_surface = app_data.layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Overlay,
        Some("glass_island"),
        None,
    );

    layer_surface.set_anchor(Anchor::TOP);
    layer_surface.set_size(app_data.width, app_data.height);
    layer_surface.set_margin(app_data.pos_y, 0, 0, 0);
    layer_surface.set_exclusive_zone(0);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.commit();

    app_data.layer_surface = Some(layer_surface);

    let window_wrapper = Arc::new(WaylandWindow {
        surface: surface.clone(),
        display: display_ptr,
    });
    app_data.window_wrapper = Some(window_wrapper.clone());

    while app_data.first_configure {
        event_queue.blocking_dispatch(&mut app_data).unwrap();
    }

    let mut last_frame = Instant::now();

    while !app_data.first_configure {
        let _ = event_queue.dispatch_pending(&mut app_data);
        let _ = conn.flush();

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = now;

        let active = app_data.update_physics(dt);
        if active {
            app_data.draw();
            let _ = conn.flush();
        }

        let timeout_ms = if active || app_data.is_hovered {
            16
        } else {
            200
        };

        let guard = match conn.prepare_read() {
            Some(g) => g,
            None => {
                let _ = event_queue.dispatch_pending(&mut app_data);
                continue;
            }
        };

        let fd = conn.as_fd().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_res = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if poll_res > 0 && (pfd.revents & libc::POLLIN) != 0 {
            let _ = guard.read();
        } else {
            drop(guard);
        }
    }
}
