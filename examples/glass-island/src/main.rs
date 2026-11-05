use glass_core::font::{FontWeight, UIFont};
use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material, Tokens};
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
    SystemOverview,
    PowerMenu,
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

#[derive(Debug, Clone)]
pub struct SysStats {
    pub cpu_pct: u32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub bat_pct: u32,
    pub bat_charging: bool,
    pub uptime_hrs: u32,
    pub uptime_mins: u32,
    prev_idle: u64,
    prev_total: u64,
    last_poll: Instant,
}

impl SysStats {
    pub fn new() -> Self {
        Self {
            cpu_pct: 0,
            ram_used_gb: 0.0,
            ram_total_gb: 1.0,
            bat_pct: 100,
            bat_charging: false,
            uptime_hrs: 0,
            uptime_mins: 0,
            prev_idle: 0,
            prev_total: 0,
            last_poll: Instant::now() - Duration::from_secs(5),
        }
    }
}

impl Default for SysStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SysStats {
    pub fn poll(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(1500) {
            return;
        }
        self.last_poll = Instant::now();

        // Sample CPU from /proc/stat
        if let Ok(s) = std::fs::read_to_string("/proc/stat") {
            if let Some(line) = s.lines().next() {
                let parts: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if parts.len() >= 4 {
                    let idle = parts[3];
                    let total: u64 = parts.iter().sum();
                    let d_idle = idle.saturating_sub(self.prev_idle);
                    let d_total = total.saturating_sub(self.prev_total);
                    if d_total > 0 {
                        self.cpu_pct = (((d_total - d_idle) as f32 / d_total as f32) * 100.0)
                            .clamp(0.0, 100.0) as u32;
                    }
                    self.prev_idle = idle;
                    self.prev_total = total;
                }
            }
        }

        // Sample RAM from /proc/meminfo
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            let mut total_kb = 0u64;
            let mut avail_kb = 0u64;
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    avail_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(0);
                }
            }
            if total_kb > 0 {
                self.ram_total_gb = total_kb as f32 / 1024.0 / 1024.0;
                self.ram_used_gb = total_kb.saturating_sub(avail_kb) as f32 / 1024.0 / 1024.0;
            }
        }

        // Sample Battery
        if let Ok(s) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
            .or_else(|_| std::fs::read_to_string("/sys/class/power_supply/BAT1/capacity"))
        {
            self.bat_pct = s.trim().parse().unwrap_or(100);
        }
        if let Ok(s) = std::fs::read_to_string("/sys/class/power_supply/BAT0/status")
            .or_else(|_| std::fs::read_to_string("/sys/class/power_supply/BAT1/status"))
        {
            self.bat_charging = s.trim().eq_ignore_ascii_case("charging");
        }

        // Sample Uptime
        if let Ok(s) = std::fs::read_to_string("/proc/uptime") {
            if let Some(sec_str) = s.split_whitespace().next() {
                if let Ok(sec) = sec_str.parse::<f32>() {
                    let total_sec = sec as u32;
                    self.uptime_hrs = total_sec / 3600;
                    self.uptime_mins = (total_sec % 3600) / 60;
                }
            }
        }
    }
}

pub enum IslandIpcEvent {
    Toggle,
    ShowMode(IslandMode),
    Notification(NotificationData),
    NextMenu,
    PrevMenu,
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

    // Slide Carousel Transitions
    slide_offset: f32,
    slide_vel: f32,
    slide_dir: f32,
    transition_from_mode: Option<IslandMode>,

    music: MusicState,
    last_music_poll: Instant,

    active_notif: Option<NotificationData>,
    notif_timeout_secs: f32,

    // IPC receiver
    ipc_rx: Receiver<IslandIpcEvent>,

    // Typography & UI overlay
    font: UIFont,
    overlay_buffer: Vec<u8>,

    // Interaction & Swipe Navigation
    press_pos: [f32; 2],
    is_swiping: bool,
    last_interaction: Instant,

    // Power Menu Hold-to-Confirm: (tile_idx, press_start_time)
    power_held: Option<(usize, Instant)>,

    // System Monitoring
    sys_stats: SysStats,

    // Hyprland Window Occlusion Auto-Hiding
    is_window_overlapping: bool,
    last_window_check: Instant,

    // Frame management
    width: u32,
    height: u32,
    pos_x: i32,
    pos_y: i32,
    first_configure: bool,
    animating: bool,
    needs_redraw: bool,

    primary_output: Option<wl_output::WlOutput>,
    is_wallpaper_light: bool,
    wallpaper_mgr: glass_platform::WallpaperManager,
    start_time: Instant,
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

impl AppData {
    fn page_index(mode: IslandMode) -> Option<usize> {
        match mode {
            IslandMode::MusicExpanded => Some(0),
            IslandMode::SystemOverview => Some(1),
            IslandMode::QuickAccess => Some(2),
            IslandMode::PowerMenu => Some(3),
            IslandMode::Notification => Some(0),
            _ => None,
        }
    }

    fn set_mode(&mut self, new_mode: IslandMode) {
        if self.mode == new_mode {
            return;
        }
        let old_mode = self.mode;
        self.prev_mode = old_mode;
        self.mode = new_mode;

        if let (Some(from_idx), Some(to_idx)) =
            (Self::page_index(old_mode), Self::page_index(new_mode))
        {
            if from_idx != to_idx {
                let dir = if (to_idx > from_idx && !(from_idx == 0 && to_idx == 3))
                    || (from_idx == 3 && to_idx == 0)
                {
                    1.0f32
                } else {
                    -1.0f32
                };
                self.transition_from_mode = Some(old_mode);
                self.slide_dir = dir;
                self.slide_offset = dir * 460.0;
                self.slide_vel = 0.0;
            }
        }

        match new_mode {
            IslandMode::Hidden => {
                self.target_w = 160.0;
                self.target_h = 10.0;
                self.target_y = -30.0;
                self.target_alpha = 0.0;
            }
            IslandMode::CompactPill => {
                self.target_w = 220.0;
                self.target_h = Tokens::ISLAND_COMPACT_H;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
            IslandMode::MusicExpanded
            | IslandMode::Notification
            | IslandMode::QuickAccess
            | IslandMode::SystemOverview
            | IslandMode::PowerMenu => {
                // Unified standard geometry: all expanded pages have identical 460x120px footprint
                self.target_w = 460.0;
                self.target_h = Tokens::ISLAND_EXPANDED_H;
                self.target_y = 12.0;
                self.target_alpha = 1.0;
            }
        }
        self.last_interaction = Instant::now();
        self.animating = true;
        self.needs_redraw = true;
    }

    fn cycle_mode_forward(&mut self) {
        let next = match self.mode {
            IslandMode::Hidden | IslandMode::CompactPill => IslandMode::MusicExpanded,
            IslandMode::MusicExpanded => IslandMode::SystemOverview,
            IslandMode::SystemOverview => IslandMode::QuickAccess,
            IslandMode::QuickAccess => IslandMode::PowerMenu,
            IslandMode::PowerMenu => IslandMode::MusicExpanded,
            IslandMode::Notification => IslandMode::MusicExpanded,
        };
        self.set_mode(next);
    }

    fn cycle_mode_backward(&mut self) {
        let prev = match self.mode {
            IslandMode::Hidden | IslandMode::CompactPill => IslandMode::PowerMenu,
            IslandMode::MusicExpanded => IslandMode::PowerMenu,
            IslandMode::SystemOverview => IslandMode::MusicExpanded,
            IslandMode::QuickAccess => IslandMode::SystemOverview,
            IslandMode::PowerMenu => IslandMode::QuickAccess,
            IslandMode::Notification => IslandMode::QuickAccess,
        };
        self.set_mode(prev);
    }

    fn toggle(&mut self) {
        match self.mode {
            IslandMode::Hidden => self.set_mode(IslandMode::CompactPill),
            _ => self.set_mode(IslandMode::Hidden),
        }
    }

    fn check_ipc(&mut self) {
        while let Ok(evt) = self.ipc_rx.try_recv() {
            match evt {
                IslandIpcEvent::Toggle => self.toggle(),
                IslandIpcEvent::ShowMode(m) => self.set_mode(m),
                IslandIpcEvent::NextMenu => self.cycle_mode_forward(),
                IslandIpcEvent::PrevMenu => self.cycle_mode_backward(),
                IslandIpcEvent::Notification(notif) => {
                    self.active_notif = Some(notif);
                    self.set_mode(IslandMode::Notification);
                }
            }
        }
    }

    fn update_physics(&mut self, dt: f32) -> bool {
        self.check_window_overlap();

        // If a window is overlapping and user is NOT hovering, smoothly retract upwards & fade
        let effective_target_y =
            if self.is_window_overlapping && !self.is_hovered && !self.is_pressed {
                -self.target_h - 10.0
            } else {
                self.target_y
            };
        let effective_target_alpha =
            if self.is_window_overlapping && !self.is_hovered && !self.is_pressed {
                0.0
            } else {
                self.target_alpha
            };

        let spring_k = 380.0f32;
        let damping = 33.1f32;

        let force_w = -spring_k * (self.current_w - self.target_w) - damping * self.vel_w;
        self.vel_w += force_w * dt;
        self.current_w += self.vel_w * dt;

        let force_h = -spring_k * (self.current_h - self.target_h) - damping * self.vel_h;
        self.vel_h += force_h * dt;
        self.current_h += self.vel_h * dt;

        let force_y = -spring_k * (self.current_y - effective_target_y) - damping * self.vel_y;
        self.vel_y += force_y * dt;
        self.current_y += self.vel_y * dt;

        let force_a =
            -spring_k * (self.current_alpha - effective_target_alpha) - damping * self.vel_alpha;
        self.vel_alpha += force_a * dt;
        self.current_alpha = (self.current_alpha + self.vel_alpha * dt).clamp(0.0, 1.0);

        let settling = (self.current_w - self.target_w).abs() < 0.5
            && (self.current_h - self.target_h).abs() < 0.5
            && (self.current_y - effective_target_y).abs() < 0.5
            && (self.current_alpha - effective_target_alpha).abs() < 0.01
            && self.vel_w.abs() < 1.0
            && self.vel_h.abs() < 1.0
            && self.vel_y.abs() < 1.0;

        let mut slide_settling = true;
        if self.transition_from_mode.is_some() {
            let force_slide = -380.0 * self.slide_offset - 32.0 * self.slide_vel;
            self.slide_vel += force_slide * dt;
            self.slide_offset += self.slide_vel * dt;

            if self.slide_offset.abs() < 0.5 && self.slide_vel.abs() < 1.0 {
                self.slide_offset = 0.0;
                self.slide_vel = 0.0;
                self.transition_from_mode = None;
            } else {
                slide_settling = false;
            }
        }

        if settling {
            self.current_w = self.target_w;
            self.current_h = self.target_h;
            self.current_y = effective_target_y;
            self.current_alpha = effective_target_alpha;
            self.vel_w = 0.0;
            self.vel_h = 0.0;
            self.vel_y = 0.0;
            self.vel_alpha = 0.0;
        }

        if !settling || !slide_settling {
            self.animating = true;
            self.needs_redraw = true;
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
        if self.music.is_playing && self.mode != IslandMode::Hidden {
            self.music.elapsed_sec += dt;
            if self.music.elapsed_sec >= self.music.duration_sec {
                self.music.elapsed_sec = 0.0;
            }
            self.music.progress =
                (self.music.elapsed_sec / self.music.duration_sec).clamp(0.0, 1.0);
        }

        // Auto-hide / auto-collapse after inactivity (8 seconds)
        if !self.is_hovered
            && !self.is_pressed
            && matches!(
                self.mode,
                IslandMode::MusicExpanded
                    | IslandMode::QuickAccess
                    | IslandMode::SystemOverview
                    | IslandMode::PowerMenu
            )
            && self.last_interaction.elapsed() >= Duration::from_secs(8)
        {
            self.set_mode(IslandMode::CompactPill);
        }

        // Power menu hold-to-confirm progress update
        let mut hold_active = false;
        if let Some((idx, start)) = self.power_held {
            hold_active = true;
            if start.elapsed() >= Duration::from_millis(1200) {
                self.power_held = None;
                match idx {
                    2 => {
                        let _ = Command::new("systemctl").arg("reboot").spawn();
                    }
                    3 => {
                        let _ = Command::new("systemctl").arg("poweroff").spawn();
                    }
                    _ => {}
                }
                self.set_mode(IslandMode::CompactPill);
            }
        }

        !settling
            || (self.music.is_playing && self.mode != IslandMode::Hidden)
            || self.is_hovered
            || hold_active
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
            } else {
                self.music.is_playing = false;
            }
        } else {
            self.music.is_playing = false;
        }
    }

    fn check_window_overlap(&mut self) {
        if self.last_window_check.elapsed() < Duration::from_millis(300) {
            return;
        }
        self.last_window_check = Instant::now();

        // Query active workspace and client geometries via Hyprland UNIX socket
        let overlapping =
            Self::query_hyprland_overlap(self.pos_x, self.pos_y, self.width as i32, 90);
        if overlapping != self.is_window_overlapping {
            self.is_window_overlapping = overlapping;
            self.animating = true;
            self.needs_redraw = true;
        }
    }

    fn query_hyprland_overlap(island_x: i32, island_y: i32, island_w: i32, island_h: i32) -> bool {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;

        let his = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            Ok(h) => h,
            Err(_) => return false,
        };
        let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
            let uid = unsafe { libc::getuid() };
            format!("/run/user/{}", uid)
        });
        let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, his);

        // 1. Get active workspace ID
        let mut active_ws_id = None;
        if let Ok(mut stream) = UnixStream::connect(&sock_path) {
            if stream.write_all(b"j/activeworkspace").is_ok() {
                let mut resp = Vec::new();
                if stream.read_to_end(&mut resp).is_ok() {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&resp) {
                        active_ws_id = val.get("id").and_then(|id| id.as_i64());
                    }
                }
            }
        }

        let ws_id = match active_ws_id {
            Some(id) => id,
            None => return false,
        };

        // 2. Query clients on active workspace
        if let Ok(mut stream) = UnixStream::connect(&sock_path) {
            if stream.write_all(b"j/clients").is_ok() {
                let mut resp = Vec::new();
                if stream.read_to_end(&mut resp).is_ok() {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&resp) {
                        if let Some(arr) = val.as_array() {
                            let island_r = island_x + island_w;
                            let island_b = island_y + island_h;

                            for client in arr {
                                let mapped = client
                                    .get("mapped")
                                    .and_then(|m| m.as_bool())
                                    .unwrap_or(false);
                                let hidden = client
                                    .get("hidden")
                                    .and_then(|h| h.as_bool())
                                    .unwrap_or(false);
                                let client_ws = client
                                    .get("workspace")
                                    .and_then(|w| w.get("id"))
                                    .and_then(|id| id.as_i64());

                                if mapped && !hidden && client_ws == Some(ws_id) {
                                    let at = client.get("at").and_then(|a| a.as_array());
                                    let sz = client.get("size").and_then(|s| s.as_array());
                                    if let (Some(at), Some(sz)) = (at, sz) {
                                        if at.len() >= 2 && sz.len() >= 2 {
                                            let wx = at[0].as_i64().unwrap_or(0) as i32;
                                            let wy = at[1].as_i64().unwrap_or(0) as i32;
                                            let ww = sz[0].as_i64().unwrap_or(0) as i32;
                                            let wh = sz[1].as_i64().unwrap_or(0) as i32;

                                            let wr = wx + ww;
                                            let wb = wy + wh;

                                            // Check AABB intersection with the island top zone
                                            if wx < island_r
                                                && wr > island_x
                                                && wy < island_b
                                                && wb > island_y
                                            {
                                                return true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn check_wallpaper(&mut self) {
        if self.wallpaper_mgr.check_update() {
            if let (Some(cache), Some(renderer)) =
                (self.wallpaper_mgr.cached_buffer(), &mut self.renderer)
            {
                let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                    cache,
                    self.pos_x.max(0) as u32,
                    self.pos_y.max(0) as u32,
                    self.width,
                    self.height,
                );
                renderer.update_background(self.width, self.height, &crop);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_page_dots(
        buf: &mut [u8],
        width: u32,
        height: u32,
        island_cx: f32,
        island_top: f32,
        island_w: f32,
        is_light: bool,
        active_idx: usize,
    ) {
        let dot_start_x = island_cx + island_w * 0.5 - 65.0;
        let dot_y = island_top + 22.0;

        for i in 0..4 {
            let dx = dot_start_x + (i as f32) * 11.0;
            let (half_r, col) = if i == active_idx {
                (
                    3.2f32,
                    if is_light {
                        [40, 50, 70, 255]
                    } else {
                        [255, 255, 255, 255]
                    },
                )
            } else {
                (
                    2.0f32,
                    if is_light {
                        [160, 170, 185, 180]
                    } else {
                        [255, 255, 255, 90]
                    },
                )
            };
            UIFont::draw_rounded_rect_fill(
                buf, width, height, dx, dot_y, half_r, half_r, half_r, col,
            );
        }
    }

    fn render_ui_overlay(&mut self) {
        if self.current_alpha <= 0.05 {
            return;
        }

        let mut overlay_vec = std::mem::take(&mut self.overlay_buffer);
        if overlay_vec.len() != (self.width * self.height * 4) as usize {
            overlay_vec.resize((self.width * self.height * 4) as usize, 0);
        }
        overlay_vec.fill(0);
        let buf = &mut overlay_vec[..];

        let is_light = self.is_wallpaper_light;
        let col_ink = if is_light {
            [8, 12, 20, 255]
        } else {
            Tokens::TEXT_PRIMARY
        };
        let col_dim = if is_light {
            [50, 60, 75, 255]
        } else {
            Tokens::TEXT_SECONDARY
        };
        let col_shadow = [0, 0, 0, 0];

        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let island_w = self.current_w;
        let island_h = self.current_h;
        let island_cy = island_top + island_h * 0.5;

        // Base Island Physical Glass Body (Apple-grade Obsidian Near-Opaque Dark Glass)
        let island_r = if self.mode == IslandMode::CompactPill {
            island_h * 0.5
        } else {
            Tokens::RADIUS_OUTER
        };

        let backdrop_col = if is_light {
            Tokens::CLEAR_BACKDROP_LIGHT
        } else {
            Tokens::OBSIDIAN_BACKDROP
        };

        UIFont::draw_rounded_rect_fill(
            buf,
            self.width,
            self.height,
            island_cx,
            island_cy,
            island_w * 0.5,
            island_h * 0.5,
            island_r,
            backdrop_col,
        );

        // Specular outer rim highlight (ONLY on outer shell)
        UIFont::draw_rounded_rect_stroke(
            buf,
            self.width,
            self.height,
            island_cx,
            island_cy,
            island_w * 0.5,
            island_h * 0.5,
            island_r,
            1.0,
            if is_light {
                Tokens::RIM_OUTER_LIGHT
            } else {
                Tokens::RIM_OUTER_DARK
            },
        );

        // Top specular bevel highlight
        let bevel_span = (island_w * 0.5 - island_r).max(10.0);
        UIFont::draw_segment(
            buf,
            self.width,
            self.height,
            (island_cx - bevel_span, island_top + 1.2),
            (island_cx + bevel_span, island_top + 1.2),
            1.0,
            if is_light {
                [255, 255, 255, 180]
            } else {
                [255, 255, 255, 65]
            },
        );

        match self.mode {
            IslandMode::CompactPill => {
                // Left: Playing or Paused Lucide Icon
                let disc_cx = island_cx - island_w * 0.5 + 24.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    island_cy,
                    12.0,
                    12.0,
                    12.0,
                    if self.music.is_playing {
                        Tokens::ACCENT_FILL
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    },
                );
                if self.music.is_playing {
                    UIFont::draw_music_icon(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        island_cy,
                        Tokens::ACCENT,
                    );
                } else {
                    UIFont::draw_pause_icon(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        island_cy,
                        6.0,
                        Tokens::TEXT_SECONDARY,
                    );
                }

                // Center: Track Title
                let title = if self.music.title.len() > 16 {
                    format!("{}…", &self.music.title[..15])
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
                    Tokens::TYPE_BODY_SM,
                    Tokens::TEXT_PRIMARY,
                    col_shadow,
                    FontWeight::SemiBold,
                );

                // Right: Equalizer waveform bars or Paused Indicator
                let right_x = island_cx + island_w * 0.5 - 28.0;
                if self.music.is_playing {
                    let t_sec = self.start_time.elapsed().as_secs_f32();
                    for i in 0..3 {
                        let bx = right_x + (i as f32) * 6.0;
                        let wave = ((t_sec * (6.0 + i as f32 * 2.0)).sin() * 0.5 + 0.5) * 5.0 + 2.5;
                        UIFont::draw_rounded_rect_fill(
                            buf,
                            self.width,
                            self.height,
                            bx,
                            island_cy,
                            1.2,
                            wave,
                            1.0,
                            Tokens::ACCENT,
                        );
                    }
                } else {
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        "PAUSED",
                        right_x + 6.0,
                        island_cy,
                        Tokens::TYPE_CAPTION,
                        Tokens::TEXT_TERTIARY,
                        col_shadow,
                        FontWeight::Medium,
                    );
                }
            }
            IslandMode::Hidden => {}
            _ => {
                if let Some(active_idx) = Self::page_index(self.mode) {
                    Self::draw_page_dots(
                        buf,
                        self.width,
                        self.height,
                        island_cx,
                        island_top,
                        island_w,
                        is_light,
                        active_idx,
                    );
                }

                if let Some(from_mode) = self.transition_from_mode {
                    let from_offset_x = self.slide_offset - self.slide_dir * island_w;
                    self.render_expanded_page_content(
                        buf,
                        from_mode,
                        island_cx + from_offset_x,
                        island_cy,
                        island_top,
                        island_w,
                        island_h,
                        is_light,
                        col_ink,
                        col_dim,
                        col_shadow,
                    );
                }

                self.render_expanded_page_content(
                    buf,
                    self.mode,
                    island_cx + self.slide_offset,
                    island_cy,
                    island_top,
                    island_w,
                    island_h,
                    is_light,
                    col_ink,
                    col_dim,
                    col_shadow,
                );
            }
        }
        self.overlay_buffer = overlay_vec;
    }

    #[allow(clippy::too_many_arguments)]
    fn render_expanded_page_content(
        &self,
        buf: &mut [u8],
        mode: IslandMode,
        cx: f32,
        island_cy: f32,
        island_top: f32,
        island_w: f32,
        _island_h: f32,
        is_light: bool,
        col_ink: [u8; 4],
        col_dim: [u8; 4],
        col_shadow: [u8; 4],
    ) {
        match mode {
            IslandMode::MusicExpanded => {
                // Header: NOW PLAYING
                let header_y = island_top + 22.0;
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    "NOW PLAYING",
                    cx - island_w * 0.5 + 32.0,
                    header_y,
                    11.0,
                    col_dim,
                    FontWeight::Bold,
                );

                // Vinyl Album Disc (half_w = 23.0, half_h = 23.0)
                let disc_cx = cx - island_w * 0.5 + 50.0;
                let disc_cy = island_top + 68.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    disc_cy,
                    23.0,
                    23.0,
                    23.0,
                    if is_light {
                        [25, 30, 42, 255]
                    } else {
                        [235, 240, 252, 255]
                    },
                );
                // Center accent core (half_w = 9.0, half_h = 9.0)
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    disc_cx,
                    disc_cy,
                    9.0,
                    9.0,
                    9.0,
                    Tokens::ACCENT,
                );

                // Track details
                let title_x = cx - island_w * 0.5 + 88.0;
                let title = if self.music.title.len() > 22 {
                    format!("{}…", &self.music.title[..20])
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
                    Tokens::TYPE_BODY,
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
                    Tokens::TYPE_BODY_SM,
                    col_dim,
                    FontWeight::Medium,
                );

                // Transport Controls: Prev, Play/Pause, Next
                let ctrl_y = island_top + 68.0;
                let play_cx = cx + island_w * 0.5 - 64.0;
                let prev_cx = play_cx - 44.0;
                let next_cx = play_cx + 44.0;

                // Previous
                UIFont::draw_prev_icon(buf, self.width, self.height, prev_cx, ctrl_y, 8.5, col_ink);

                // Play / Pause Button with circle fill (half_w = 18.0, half_h = 18.0)
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    play_cx,
                    ctrl_y,
                    18.0,
                    18.0,
                    18.0,
                    if is_light {
                        [20, 24, 34, 255]
                    } else {
                        [245, 248, 255, 255]
                    },
                );
                let play_icon_col = if is_light {
                    [255, 255, 255, 255]
                } else {
                    [10, 14, 20, 255]
                };
                if self.music.is_playing {
                    UIFont::draw_pause_icon(
                        buf,
                        self.width,
                        self.height,
                        play_cx,
                        ctrl_y,
                        7.5,
                        play_icon_col,
                    );
                } else {
                    UIFont::draw_play_icon(
                        buf,
                        self.width,
                        self.height,
                        play_cx,
                        ctrl_y,
                        8.0,
                        play_icon_col,
                    );
                }

                // Next
                UIFont::draw_next_icon(buf, self.width, self.height, next_cx, ctrl_y, 8.5, col_ink);

                // Progress Bar (at y = 104.0)
                let bar_y = island_top + 104.0;
                let bar_w = island_w - 64.0;
                let bar_x = cx;
                // High-contrast track back (half_w = bar_w * 0.5, half_h = 2.0)
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    bar_x,
                    bar_y,
                    bar_w * 0.5,
                    2.0,
                    2.0,
                    if is_light {
                        [160, 170, 190, 200]
                    } else {
                        [140, 155, 185, 160]
                    },
                );
                // Active fill
                let fill_w = bar_w * self.music.progress;
                if fill_w > 2.0 {
                    let fill_cx = cx - bar_w * 0.5 + fill_w * 0.5;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        fill_cx,
                        bar_y,
                        fill_w * 0.5,
                        2.0,
                        2.0,
                        Tokens::ACCENT,
                    );
                    // Glowing scrubber knob
                    let knob_x = cx - bar_w * 0.5 + fill_w;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        knob_x,
                        bar_y,
                        4.0,
                        4.0,
                        4.0,
                        [255, 255, 255, 255],
                    );
                }
            }
            IslandMode::Notification => {
                if let Some(notif) = &self.active_notif {
                    // Left App Icon badge (half_w = 20.0, half_h = 20.0) with deep royal accent
                    let badge_cx = cx - island_w * 0.5 + 42.0;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        badge_cx,
                        island_cy,
                        20.0,
                        20.0,
                        14.0,
                        [30, 96, 220, 255], // Deep royal blue badge (>5.5:1 against white bell)
                    );
                    UIFont::draw_bell_icon(
                        buf,
                        self.width,
                        self.height,
                        badge_cx,
                        island_cy,
                        9.5,
                        [255, 255, 255, 255],
                    );

                    // App Name Header
                    let content_x = cx - island_w * 0.5 + 76.0;
                    self.font.draw_text(
                        buf,
                        self.width,
                        self.height,
                        &notif.app.to_uppercase(),
                        content_x,
                        island_top + 28.0,
                        11.0,
                        [200, 224, 255, 255], // Crisp #c8e0ff (>6:1 contrast)
                        FontWeight::Bold,
                    );

                    // Summary
                    let summary = if notif.summary.len() > 30 {
                        format!("{}...", &notif.summary[..28])
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
                    let body = if notif.body.len() > 38 {
                        format!("{}...", &notif.body[..36])
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

                    // Right-side metadata: "now" badge pill + "✕" dismiss button
                    let right_x = cx + island_w * 0.5 - 34.0;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        right_x - 38.0,
                        island_top + 26.0,
                        18.0,
                        8.5,
                        6.0,
                        [255, 255, 255, 28],
                    );
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        "now",
                        right_x - 38.0,
                        island_top + 26.0,
                        10.5,
                        [200, 224, 255, 255],
                        col_shadow,
                        FontWeight::Medium,
                    );

                    // Close/dismiss button circle (r = 13.0)
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        right_x,
                        island_cy,
                        13.0,
                        13.0,
                        13.0,
                        [255, 255, 255, 24],
                    );
                    UIFont::draw_close_icon(
                        buf,
                        self.width,
                        self.height,
                        right_x,
                        island_cy,
                        5.0,
                        col_dim,
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
                    cx - island_w * 0.5 + 32.0,
                    island_top + 24.0,
                    11.0,
                    col_dim,
                    FontWeight::Bold,
                );

                let apps = ["Terminal", "Browser", "Editor", "Files", "Music"];
                let key_hints = ["1", "2", "3", "4", "5"];

                let item_w = (island_w - 50.0) / (apps.len() as f32);
                let start_x = cx - island_w * 0.5 + 25.0 + item_w * 0.5;

                let cur_lx = self.cursor_pos[0] * (self.width as f32);
                let cur_ly = self.cursor_pos[1] * (self.height as f32);

                for (idx, name) in apps.iter().enumerate() {
                    let cx = start_x + (idx as f32) * item_w;
                    let cy = island_top + 65.0;

                    let is_tile_hover = self.is_hovered
                        && (cur_lx - cx).abs() <= (item_w - 10.0) * 0.5
                        && (cur_ly - cy).abs() <= 28.0;

                    let tile_bg = if is_tile_hover {
                        if is_light {
                            Tokens::INNER_TILE_BG_LIGHT
                        } else {
                            Tokens::INNER_TILE_BG_HOVER_DARK
                        }
                    } else if is_light {
                        [255, 255, 255, 60]
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    };

                    // Subtle inner fill (NO wireframe outline!)
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        cx,
                        cy,
                        (item_w - 10.0) * 0.5,
                        28.0,
                        Tokens::RADIUS_INNER_TILE,
                        tile_bg,
                    );

                    // Top highlight bevel only
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                        1.0,
                        Tokens::INNER_TOP_HIGHLIGHT_DARK,
                    );

                    // Key hint badge at top-right of tile (1..5)
                    let hint_cx = cx + (item_w - 10.0) * 0.5 - 10.0;
                    let hint_cy = cy - 16.0;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        hint_cx,
                        hint_cy,
                        6.5,
                        6.5,
                        3.5,
                        if is_light {
                            [200, 208, 222, 200]
                        } else {
                            [45, 54, 72, 220]
                        },
                    );
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        key_hints[idx],
                        hint_cx,
                        hint_cy,
                        Tokens::TYPE_CAPTION - 2.0,
                        Tokens::TEXT_SECONDARY,
                        col_shadow,
                        FontWeight::Bold,
                    );

                    let icon_cy = cy - 8.0;
                    match idx {
                        0 => UIFont::draw_terminal_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            icon_cy,
                            col_ink,
                        ),
                        1 => UIFont::draw_browser_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            icon_cy,
                            col_ink,
                        ),
                        2 => UIFont::draw_editor_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            icon_cy,
                            col_ink,
                        ),
                        3 => UIFont::draw_folder_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            icon_cy,
                            col_ink,
                        ),
                        _ => UIFont::draw_music_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            icon_cy,
                            col_ink,
                        ),
                    }

                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        name,
                        cx,
                        cy + 13.0,
                        Tokens::TYPE_CAPTION,
                        Tokens::TEXT_PRIMARY,
                        col_shadow,
                        FontWeight::Medium,
                    );
                }
            }
            IslandMode::SystemOverview => {
                // Header
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    "SYSTEM OVERVIEW",
                    cx - island_w * 0.5 + 32.0,
                    island_top + 24.0,
                    Tokens::TYPE_CAPTION,
                    Tokens::TEXT_TERTIARY,
                    FontWeight::Bold,
                );

                // 4 Metric cards: CPU, RAM, Battery, Uptime
                let stat_items = [
                    (
                        "CPU",
                        format!("{}%", self.sys_stats.cpu_pct),
                        if self.sys_stats.cpu_pct > 80 {
                            Tokens::STATUS_RED
                        } else {
                            Tokens::ACCENT
                        },
                    ),
                    (
                        "RAM",
                        format!("{:.1}G", self.sys_stats.ram_used_gb),
                        Tokens::ACCENT,
                    ),
                    (
                        "BATTERY",
                        format!("{}%", self.sys_stats.bat_pct),
                        if self.sys_stats.bat_pct <= 20 {
                            Tokens::STATUS_AMBER
                        } else if self.sys_stats.bat_charging {
                            Tokens::STATUS_GREEN
                        } else {
                            Tokens::ACCENT
                        },
                    ),
                    (
                        "UPTIME",
                        format!(
                            "{}h {}m",
                            self.sys_stats.uptime_hrs, self.sys_stats.uptime_mins
                        ),
                        Tokens::ACCENT,
                    ),
                ];

                let item_w = (island_w - 52.0) / 4.0;
                let start_x = cx - island_w * 0.5 + 26.0 + item_w * 0.5;
                let cy = island_top + 65.0;

                for (idx, (lbl, val, accent)) in stat_items.iter().enumerate() {
                    let cx = start_x + (idx as f32) * item_w;

                    // Container pill: subtle 7% white fill (NO wireframe outline!)
                    let tile_bg = if is_light {
                        Tokens::INNER_TILE_BG_LIGHT
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        cx,
                        cy,
                        (item_w - 10.0) * 0.5,
                        28.0,
                        Tokens::RADIUS_INNER_TILE,
                        tile_bg,
                    );

                    // Top highlight bevel only
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                        1.0,
                        Tokens::INNER_TOP_HIGHLIGHT_DARK,
                    );

                    // Lucide Vector Icon (top)
                    match idx {
                        0 => UIFont::draw_cpu_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            *accent,
                        ),
                        1 => UIFont::draw_ram_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            *accent,
                        ),
                        2 => UIFont::draw_battery_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            self.sys_stats.bat_pct,
                            self.sys_stats.bat_charging,
                            *accent,
                        ),
                        _ => UIFont::draw_clock_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            *accent,
                        ),
                    }

                    // Value (pure white tabular text for optical clarity)
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        val,
                        cx,
                        cy + 6.0,
                        Tokens::TYPE_BODY,
                        Tokens::TEXT_PRIMARY,
                        col_shadow,
                        FontWeight::SemiBold,
                    );

                    // Label (clean caption below)
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        lbl,
                        cx,
                        cy + 20.0,
                        Tokens::TYPE_CAPTION - 0.5,
                        Tokens::TEXT_TERTIARY,
                        col_shadow,
                        FontWeight::Medium,
                    );
                }
            }
            IslandMode::PowerMenu => {
                // Header
                self.font.draw_text(
                    buf,
                    self.width,
                    self.height,
                    "SESSION & POWER",
                    cx - island_w * 0.5 + 32.0,
                    island_top + 24.0,
                    Tokens::TYPE_CAPTION,
                    Tokens::TEXT_TERTIARY,
                    FontWeight::Bold,
                );

                // 4 Action buttons: Lock, Suspend, Restart, Shut Down
                let actions = [
                    ("Lock", Tokens::ACCENT),
                    ("Suspend", Tokens::ACCENT),
                    ("Restart", Tokens::STATUS_AMBER),
                    ("Power Off", Tokens::STATUS_RED),
                ];

                let item_w = (island_w - 52.0) / 4.0;
                let start_x = cx - island_w * 0.5 + 26.0 + item_w * 0.5;
                let cy = island_top + 65.0;

                let cur_lx = self.cursor_pos[0] * (self.width as f32);
                let cur_ly = self.cursor_pos[1] * (self.height as f32);

                for (idx, (name, accent_col)) in actions.iter().enumerate() {
                    let cx = start_x + (idx as f32) * item_w;

                    let is_tile_hover = self.is_hovered
                        && (cur_lx - cx).abs() <= (item_w - 10.0) * 0.5
                        && (cur_ly - cy).abs() <= 28.0;

                    let is_tile_pressed = self.power_held.is_some_and(|(h_idx, _)| h_idx == idx);
                    let hold_progress = if let Some((h_idx, start)) = self.power_held {
                        if h_idx == idx {
                            (start.elapsed().as_secs_f32() / 1.2).clamp(0.0, 1.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    let tile_bg = if is_tile_pressed {
                        if is_light {
                            [255, 200, 200, 245]
                        } else {
                            [120, 35, 45, 230]
                        }
                    } else if is_tile_hover {
                        if is_light {
                            Tokens::INNER_TILE_BG_LIGHT
                        } else {
                            Tokens::INNER_TILE_BG_HOVER_DARK
                        }
                    } else if is_light {
                        [255, 255, 255, 60]
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    };

                    // Subtle inner fill (NO wireframe outline!)
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        cx,
                        cy,
                        (item_w - 10.0) * 0.5,
                        28.0,
                        Tokens::RADIUS_INNER_TILE,
                        tile_bg,
                    );

                    // Top highlight bevel only
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                        1.0,
                        Tokens::INNER_TOP_HIGHLIGHT_DARK,
                    );

                    // Red hold-to-confirm progress bar fill
                    if hold_progress > 0.01 {
                        let max_fill_w = (item_w - 16.0) * 0.5;
                        let fill_w = max_fill_w * hold_progress;
                        UIFont::draw_rounded_rect_fill(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy + 24.0,
                            fill_w,
                            1.8,
                            1.5,
                            Tokens::STATUS_RED,
                        );
                    }

                    // Precision Lucide Vector Icon (top)
                    match idx {
                        0 => UIFont::draw_lock_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            *accent_col,
                        ),
                        1 => UIFont::draw_moon_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            *accent_col,
                        ),
                        2 => UIFont::draw_restart_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            *accent_col,
                        ),
                        _ => UIFont::draw_power_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            *accent_col,
                        ),
                    }

                    // Text (pure white / hold hint)
                    let display_text = if is_tile_pressed && (idx == 2 || idx == 3) {
                        "Hold..."
                    } else {
                        name
                    };
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        display_text,
                        cx,
                        cy + 12.0,
                        Tokens::TYPE_CAPTION,
                        Tokens::TEXT_PRIMARY,
                        col_shadow,
                        FontWeight::SemiBold,
                    );
                }
            }
            _ => {}
        }
    }

    fn draw(&mut self) -> bool {
        self.check_ipc();
        self.check_music();
        self.check_wallpaper();
        self.check_window_overlap();
        self.sys_stats.poll();

        self.render_ui_overlay();

        if let Some(renderer) = &mut self.renderer {
            if self.current_alpha <= 0.01 {
                let _ = renderer.render(&[]);
                return false;
            }

            renderer.update_overlay(self.width, self.height, &self.overlay_buffer);

            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);
            let x_norm = 0.50 - w_norm * 0.5;
            let y_norm = self.current_y / (self.height as f32);

            let is_light = if self.is_wallpaper_light { 1.0 } else { 0.0 };

            let island_shape = if self.mode == IslandMode::CompactPill {
                Shape::Capsule
            } else {
                Shape::RoundedRectangle {
                    radius: Tokens::RADIUS_OUTER,
                }
            };

            let island = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(island_shape)
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(0.06)
                .bevel(14.0)
                .refraction(0.75)
                .dispersion(0.025)
                .blur(7.0)
                .specular(1.20)
                .fresnel(0.75)
                .tint(
                    if self.music.is_playing { 1.0 } else { 0.0 },
                    if self.is_pressed { 1.0 } else { 0.0 },
                    self.current_alpha,
                    is_light,
                )
                .element_type(27.0)
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

        // Page dot clicks across all expanded modes
        let dot_start_x = island_cx + island_w * 0.5 - 65.0;
        let dot_y = island_top + 22.0;
        if ly >= dot_y - 10.0
            && ly <= dot_y + 10.0
            && lx >= dot_start_x - 8.0
            && lx <= dot_start_x + 45.0
        {
            let dot_idx = (((lx - dot_start_x + 5.0) / 11.0).floor() as usize).min(3);
            match dot_idx {
                0 => self.set_mode(IslandMode::MusicExpanded),
                1 => self.set_mode(IslandMode::SystemOverview),
                2 => self.set_mode(IslandMode::QuickAccess),
                _ => self.set_mode(IslandMode::PowerMenu),
            }
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
            IslandMode::SystemOverview => {
                // Clicking outside tiles collapses or cycles to QuickAccess
                if lx < island_cx - island_w * 0.5 + 80.0 {
                    self.set_mode(IslandMode::CompactPill);
                }
            }
            IslandMode::PowerMenu => {
                let item_w = (island_w - 52.0) / 4.0;
                let start_x = island_cx - island_w * 0.5 + 26.0 + item_w * 0.5;
                let cy = island_top + 64.0;

                for idx in 0..4 {
                    let cx = start_x + (idx as f32) * item_w;
                    if (lx - cx).abs() <= (item_w - 10.0) * 0.5 && (ly - cy).abs() <= 25.0 {
                        match idx {
                            0 => {
                                // Lock screen (safe immediate)
                                let _ = Command::new("hyprlock").spawn();
                                self.set_mode(IslandMode::CompactPill);
                            }
                            1 => {
                                // Suspend (safe immediate)
                                let _ = Command::new("systemctl").arg("suspend").spawn();
                                self.set_mode(IslandMode::CompactPill);
                            }
                            // 2 (Restart) and 3 (Power Off) require Hold-to-Confirm!
                            // Instant clicks do nothing to prevent accidents.
                            _ => {}
                        }
                        break;
                    }
                }
            }
            IslandMode::Hidden => {}
        }
    }
}

#[allow(dead_code)]
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

                if let Some(cache) = self.wallpaper_mgr.cached_buffer() {
                    let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                        cache,
                        self.pos_x.max(0) as u32,
                        self.pos_y.max(0) as u32,
                        self.width,
                        self.height,
                    );
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
                    self.last_interaction = Instant::now();
                    self.cursor_pos = [
                        (event.position.0 as f32) / (self.width as f32),
                        (event.position.1 as f32) / (self.height as f32),
                    ];
                }
                PointerEventKind::Leave { .. } => {
                    self.is_hovered = false;
                    self.is_pressed = false;
                    self.is_swiping = false;
                    self.cursor_pos = [-1.0, -1.0];
                }
                PointerEventKind::Motion { .. } => {
                    self.last_interaction = Instant::now();
                    self.cursor_pos = [
                        (event.position.0 as f32) / (self.width as f32),
                        (event.position.1 as f32) / (self.height as f32),
                    ];
                    if self.is_pressed && !self.is_swiping {
                        let dx = event.position.0 as f32 - self.press_pos[0];
                        if dx.abs() > 25.0 {
                            self.is_swiping = true;
                            if dx > 0.0 {
                                self.cycle_mode_backward();
                            } else {
                                self.cycle_mode_forward();
                            }
                        }
                    }
                }
                PointerEventKind::Press { .. } => {
                    self.is_pressed = true;
                    self.is_swiping = false;
                    self.last_interaction = Instant::now();
                    let lx = event.position.0 as f32;
                    let ly = event.position.1 as f32;
                    self.press_pos = [lx, ly];

                    // Check if pressing Restart (idx=2) or Power Off (idx=3) in PowerMenu
                    if self.mode == IslandMode::PowerMenu {
                        let island_cx = (self.width as f32) * 0.5;
                        let island_top = self.current_y;
                        let island_w = self.current_w;
                        let item_w = (island_w - 52.0) / 4.0;
                        let start_x = island_cx - island_w * 0.5 + 26.0 + item_w * 0.5;
                        let cy = island_top + 64.0;

                        for idx in 2..=3 {
                            let cx = start_x + (idx as f32) * item_w;
                            if (lx - cx).abs() <= (item_w - 10.0) * 0.5 && (ly - cy).abs() <= 25.0 {
                                self.power_held = Some((idx, Instant::now()));
                                break;
                            }
                        }
                    }
                }
                PointerEventKind::Release { .. } => {
                    let lx = event.position.0 as f32;
                    let ly = event.position.1 as f32;
                    if !self.is_swiping {
                        self.handle_click(lx, ly);
                    }
                    self.is_pressed = false;
                    self.is_swiping = false;
                    self.power_held = None;
                }
                PointerEventKind::Axis { horizontal, .. } => {
                    self.last_interaction = Instant::now();
                    if horizontal.absolute.abs() > 15.0 {
                        if horizontal.absolute > 0.0 {
                            self.cycle_mode_forward();
                        } else {
                            self.cycle_mode_backward();
                        }
                    }
                }
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
            // Open with read-write so the descriptor remains open across client invocations
            // and read never returns premature EOF on FIFO close!
            let file = OpenOptions::new().read(true).write(true).open(FIFO_PATH);
            if let Ok(f) = file {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(f);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if trimmed == "toggle" {
                        let _ = tx.send(IslandIpcEvent::Toggle);
                    } else if trimmed == "hide" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::Hidden));
                    } else if trimmed == "show" || trimmed == "compact" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::CompactPill));
                    } else if trimmed == "music" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::MusicExpanded));
                    } else if trimmed == "sys" || trimmed == "system" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::SystemOverview));
                    } else if trimmed == "power" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::PowerMenu));
                    } else if trimmed == "apps" || trimmed == "quick" {
                        let _ = tx.send(IslandIpcEvent::ShowMode(IslandMode::QuickAccess));
                    } else if trimmed == "next" {
                        let _ = tx.send(IslandIpcEvent::NextMenu);
                    } else if trimmed == "prev" {
                        let _ = tx.send(IslandIpcEvent::PrevMenu);
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

        slide_offset: 0.0,
        slide_vel: 0.0,
        slide_dir: 1.0,
        transition_from_mode: None,

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

        press_pos: [-1.0, -1.0],
        is_swiping: false,
        last_interaction: Instant::now(),
        power_held: None,
        sys_stats: SysStats::new(),
        is_window_overlapping: false,
        last_window_check: Instant::now() - Duration::from_secs(5),

        width,
        height,
        pos_x,
        pos_y,
        first_configure: true,
        animating: true,
        needs_redraw: true,
        primary_output: None,
        is_wallpaper_light: false,
        wallpaper_mgr: glass_platform::WallpaperManager::new(),
        start_time: Instant::now(),
    };

    let surface = app_data.compositor_state.create_surface(&qh);

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

        app_data.check_ipc();

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = now;

        let active = app_data.update_physics(dt);
        if active || app_data.needs_redraw {
            app_data.draw();
            app_data.needs_redraw = false;
            let _ = conn.flush();
        }

        let timeout_ms = if active || app_data.is_hovered {
            16
        } else {
            250
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
