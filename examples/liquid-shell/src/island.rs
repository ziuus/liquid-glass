use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::{GlassRenderer, GlassSurface, Material, SharedGpu, Tokens};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::compositor::{CompositorState, SurfaceData};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
    LayerSurfaceData,
};
use smithay_client_toolkit::shell::WaylandSurface;
use std::fs::OpenOptions;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

const FIFO_PATH: &str = "/tmp/liquid_island.fifo";

pub fn safe_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandDockMode {
    Floating,
    AttachedStraight,
    AttachedCurved,
}

impl IslandDockMode {
    #[allow(dead_code)]
    pub fn is_attached(&self) -> bool {
        matches!(
            self,
            IslandDockMode::AttachedStraight | IslandDockMode::AttachedCurved
        )
    }

    pub fn is_curved(&self) -> bool {
        matches!(self, IslandDockMode::AttachedCurved)
    }

    pub fn deformation(&self) -> f32 {
        match self {
            IslandDockMode::Floating => 0.0,
            IslandDockMode::AttachedStraight => 1.0,
            IslandDockMode::AttachedCurved => 2.0,
        }
    }

    pub fn dock_y(&self) -> f32 {
        match self {
            IslandDockMode::Floating => 8.0,
            IslandDockMode::AttachedStraight | IslandDockMode::AttachedCurved => 0.0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            IslandDockMode::Floating => "floating",
            IslandDockMode::AttachedStraight => "straight",
            IslandDockMode::AttachedCurved => "curved",
        }
    }

    pub fn from_str(s: &str) -> Self {
        let trimmed = s.trim().to_ascii_lowercase();
        if trimmed == "straight" || trimmed == "flush" || trimmed == "attached_straight" {
            IslandDockMode::AttachedStraight
        } else if trimmed == "curved"
            || trimmed == "notch"
            || trimmed == "fillet"
            || trimmed == "attached"
            || trimmed == "attached_curved"
        {
            IslandDockMode::AttachedCurved
        } else {
            IslandDockMode::Floating
        }
    }

    pub fn next(&self) -> Self {
        match self {
            IslandDockMode::AttachedCurved => IslandDockMode::AttachedStraight,
            IslandDockMode::AttachedStraight => IslandDockMode::Floating,
            IslandDockMode::Floating => IslandDockMode::AttachedCurved,
        }
    }
}

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
    pub art_url: String,
    pub art_thumbnail: Option<Vec<u8>>,
    pub disc_rotation: f32,
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
    SetStyle(f32),
    CycleStyle,
    SetTheme(u32),
    CycleTheme,
    SetDock(IslandDockMode),
    ToggleDock,
    SetFillet(f32),
    ReloadConfig,
}

pub struct WaylandWindow {
    pub surface: wl_surface::WlSurface,
    pub display: *mut std::ffi::c_void,
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

pub struct IslandWidget {
    #[allow(dead_code)]
    pub layer_surface: Option<LayerSurface>,
    pub surface: wl_surface::WlSurface,
    pub renderer: Option<GlassRenderer>,
    pub window_wrapper: Arc<WaylandWindow>,

    pub cursor_pos: [f32; 2],
    pub is_pressed: bool,
    pub is_hovered: bool,

    pub current_w: f32,
    pub target_w: f32,
    pub vel_w: f32,

    pub current_h: f32,
    pub target_h: f32,
    pub vel_h: f32,

    pub current_alpha: f32,
    pub target_alpha: f32,
    pub vel_alpha: f32,

    pub current_y: f32,
    pub target_y: f32,
    pub vel_y: f32,

    pub dock_mode: IslandDockMode,
    pub fillet_radius: f32,

    pub mode: IslandMode,
    pub prev_mode: IslandMode,

    pub slide_offset: f32,
    pub slide_vel: f32,
    pub slide_dir: f32,
    pub transition_from_mode: Option<IslandMode>,

    pub music: MusicState,
    pub last_music_poll: Instant,

    pub active_notif: Option<NotificationData>,
    pub notif_timeout_secs: f32,

    pub ipc_rx: Receiver<IslandIpcEvent>,

    pub font: UIFont,
    pub overlay_buffer: Vec<u8>,

    pub press_pos: [f32; 2],
    pub press_time: Instant,
    pub last_click_time: Instant,
    pub last_click_pos: [f32; 2],
    pub is_swiping: bool,
    pub last_interaction: Instant,
    pub last_mode_cycle: Instant,

    pub power_held: Option<(usize, Instant)>,
    pub sys_stats: SysStats,

    pub is_window_overlapping: bool,
    pub last_window_check: Instant,

    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub animating: bool,
    pub needs_redraw: bool,

    pub is_wallpaper_light: bool,
    pub wallpaper_crop: Option<Vec<u8>>,
    pub start_time: Instant,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub hot_edge_dwell_start: Option<Instant>,
    pub needs_input_region_update: bool,
}

impl IslandWidget {
    pub fn sample_island_is_light(&self) -> bool {
        if let Some(crop) = &self.wallpaper_crop {
            let tw = self.target_w.max(Tokens::ISLAND_COMPACT_W);
            let th = self.target_h.max(Tokens::ISLAND_COMPACT_H);
            let ty = self.target_y.max(8.0);
            let cx = ((self.width as f32 * 0.5) - tw * 0.5).max(0.0) as u32;
            let cy = ty as u32;
            let cw = (tw as u32).min(self.width.saturating_sub(cx));
            let ch = (th as u32).min(self.height.saturating_sub(cy));
            if cw > 0 && ch > 0 {
                let mut sub = Vec::with_capacity((cw * ch * 4) as usize);
                for dy in 0..ch {
                    let sy = (cy + dy).min(self.height.saturating_sub(1));
                    let src_idx = ((sy * self.width + cx) * 4) as usize;
                    let end_idx = src_idx + (cw * 4) as usize;
                    if end_idx <= crop.len() {
                        sub.extend_from_slice(&crop[src_idx..end_idx]);
                    }
                }
                return is_buffer_light(&sub);
            }
        }
        self.is_wallpaper_light
    }

    pub fn effective_is_light(&self) -> bool {
        match self.theme_mode {
            1 => false, // Forced Dark
            2 => true,  // Forced Light
            _ => self.sample_island_is_light(),
        }
    }
    pub fn new<D>(
        qh: &QueueHandle<D>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        display_ptr: *mut std::ffi::c_void,
        wakeup_fd: Option<i32>,
    ) -> Self
    where
        D: Dispatch<wl_surface::WlSurface, SurfaceData>
            + Dispatch<ZwlrLayerSurfaceV1, LayerSurfaceData>
            + 'static,
    {
        let width = 640u32;
        let height = 180u32;
        let pos_y = 0i32;

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Overlay,
            Some("glass_island"),
            None,
        );

        layer_surface.set_anchor(Anchor::TOP);
        layer_surface.set_size(width, height);
        layer_surface.set_margin(pos_y, 0, 0, 0);
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.commit();

        let window_wrapper = Arc::new(WaylandWindow {
            surface: surface.clone(),
            display: display_ptr,
        });

        let (ipc_tx, ipc_rx) = channel();
        start_fifo_listener(ipc_tx, wakeup_fd);
        start_dbus_monitor();

        let font = UIFont::new();

        let (dock_mode, fillet_radius) = Self::load_config_values();
        let init_y = dock_mode.dock_y();

        Self {
            layer_surface: Some(layer_surface),
            surface,
            renderer: None,
            window_wrapper,

            cursor_pos: [-1.0, -1.0],
            is_pressed: false,
            is_hovered: false,

            current_w: Tokens::ISLAND_COMPACT_W,
            target_w: Tokens::ISLAND_COMPACT_W,
            vel_w: 0.0,

            current_h: Tokens::ISLAND_COMPACT_H,
            target_h: Tokens::ISLAND_COMPACT_H,
            vel_h: 0.0,

            current_alpha: 1.0,
            target_alpha: 1.0,
            vel_alpha: 0.0,

            current_y: init_y,
            target_y: init_y,
            vel_y: 0.0,

            dock_mode,
            fillet_radius,

            mode: IslandMode::CompactPill,
            prev_mode: IslandMode::CompactPill,

            slide_offset: 0.0,
            slide_vel: 0.0,
            slide_dir: 1.0,
            transition_from_mode: None,

            music: MusicState {
                title: "No Media Playing".to_string(),
                artist: String::new(),
                is_playing: false,
                progress: 0.0,
                duration_sec: 0.0,
                elapsed_sec: 0.0,
                art_url: String::new(),
                art_thumbnail: None,
                disc_rotation: 0.0,
            },
            last_music_poll: Instant::now() - Duration::from_secs(10),

            active_notif: None,
            notif_timeout_secs: 5.0,
            ipc_rx,

            font,
            overlay_buffer: Vec::new(),

            press_pos: [0.0, 0.0],
            press_time: Instant::now(),
            last_click_time: Instant::now() - Duration::from_secs(10),
            last_click_pos: [-100.0, -100.0],
            is_swiping: false,
            last_interaction: Instant::now(),
            last_mode_cycle: Instant::now() - Duration::from_secs(10),

            power_held: None,
            sys_stats: SysStats::new(),

            is_window_overlapping: false,
            last_window_check: Instant::now() - Duration::from_secs(10),

            width,
            height,
            pos_x: 640,
            pos_y,
            animating: false,
            needs_redraw: true,

            is_wallpaper_light: false,
            wallpaper_crop: None,
            start_time: Instant::now(),
            style_mode: 2.0,
            theme_mode: 0,
            hot_edge_dwell_start: None,
            needs_input_region_update: true,
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn is_inside_island(&self, lx: f32, ly: f32) -> bool {
        if self.mode == IslandMode::Hidden {
            return false;
        }
        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let flare = if self.dock_mode.is_curved() && ly <= self.fillet_radius {
            self.fillet_radius
        } else {
            0.0
        };
        let half_w = self.current_w * 0.5 + flare;
        let h = self.current_h;

        // Cover from very top screen edge (ly >= 0.0) down through the pill
        lx >= (island_cx - half_w)
            && lx <= (island_cx + half_w)
            && ly >= 0.0
            && ly <= (island_top + h + 6.0)
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        self.needs_input_region_update = false;
        let region = compositor.wl_compositor().create_region(qh, ());
        let flare = if self.dock_mode.is_curved() {
            self.fillet_radius as i32
        } else {
            0
        };
        if self.mode != IslandMode::Hidden && self.current_alpha > 0.01 {
            match self.mode {
                IslandMode::CompactPill => {
                    let pill_w = self.target_w as i32 + flare * 2;
                    let pill_h = (self.target_y + self.target_h + 6.0) as i32;
                    let pill_x = (self.width as i32 - pill_w) / 2;
                    // Intercept from top edge y=0 down to bottom of pill
                    region.add(pill_x, 0, pill_w, pill_h);
                }
                _ => {
                    let card_w = self.target_w as i32 + flare * 2;
                    let card_h = (self.target_y + self.target_h + 10.0) as i32;
                    let card_x = (self.width as i32 - card_w) / 2;
                    region.add(card_x, 0, card_w, card_h);
                }
            }
        } else {
            // Invisible 2px hair strip across top center for immediate edge contact
            let strip_w = 400;
            let strip_x = (self.width as i32 - strip_w) / 2;
            region.add(strip_x, 0, strip_w, 2);
        }
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.commit();
    }

    pub fn init_gpu(&mut self, gpu: &SharedGpu) {
        let mut renderer = gpu
            .create_renderer(self.window_wrapper.clone(), self.width, self.height)
            .expect("Failed to create GlassRenderer for island");

        if let Some(crop) = &self.wallpaper_crop {
            renderer.update_background(self.width, self.height, crop);
        }
        self.renderer = Some(renderer);
    }

    pub fn configure(&mut self, configure: LayerSurfaceConfigure) {
        if configure.new_size.0 != 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 != 0 {
            self.height = configure.new_size.1;
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.width, self.height);
            if let Some(crop) = &self.wallpaper_crop {
                renderer.update_background(self.width, self.height, crop);
            }
        }
    }

    pub fn update_wallpaper(&mut self, crop: Vec<u8>) {
        self.is_wallpaper_light = is_buffer_light(&crop);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
        self.needs_redraw = true;
    }

    pub fn on_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event.kind {
            PointerEventKind::Enter { .. } => {
                let lx = event.position.0 as f32;
                let ly = event.position.1 as f32;
                self.cursor_pos = [lx / (self.width as f32), ly / (self.height as f32)];
                self.hot_edge_dwell_start = None;
                if self.mode == IslandMode::Hidden {
                    self.set_mode(IslandMode::CompactPill);
                    self.is_hovered = true;
                    self.last_interaction = Instant::now();
                    return true;
                }
                if self.is_inside_island(lx, ly) {
                    self.is_hovered = true;
                    self.last_interaction = Instant::now();
                } else {
                    self.is_hovered = false;
                }
                true
            }
            PointerEventKind::Leave { .. } => {
                self.is_hovered = false;
                self.is_pressed = false;
                self.is_swiping = false;
                self.hot_edge_dwell_start = None;
                self.cursor_pos = [-1.0, -1.0];
                self.last_interaction = Instant::now() - Duration::from_millis(1000);
                true
            }
            PointerEventKind::Motion { .. } => {
                let lx = event.position.0 as f32;
                let ly = event.position.1 as f32;
                self.cursor_pos = [lx / (self.width as f32), ly / (self.height as f32)];

                if self.mode == IslandMode::Hidden {
                    self.set_mode(IslandMode::CompactPill);
                    self.is_hovered = true;
                    self.last_interaction = Instant::now();
                    return true;
                }

                let inside = self.is_inside_island(lx, ly);
                if inside {
                    self.is_hovered = true;
                    self.last_interaction = Instant::now();
                } else {
                    self.is_hovered = false;
                }

                if self.is_pressed && inside {
                    let dx = lx - self.press_pos[0];
                    let dy = ly - self.press_pos[1];
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist > 8.0 {
                        self.power_held = None;
                    }

                    if !self.is_swiping
                        && dx.abs() > 55.0
                        && self.press_time.elapsed() >= Duration::from_millis(80)
                    {
                        self.is_swiping = true;
                        if dx > 0.0 {
                            self.cycle_mode_backward();
                        } else {
                            self.cycle_mode_forward();
                        }
                    }
                }
                true
            }
            PointerEventKind::Press { .. } => {
                let lx = event.position.0 as f32;
                let ly = event.position.1 as f32;
                let inside = self.is_inside_island(lx, ly);

                if !inside {
                    self.is_pressed = false;
                    self.is_swiping = false;
                    self.power_held = None;
                    if self.mode != IslandMode::CompactPill && self.mode != IslandMode::Hidden {
                        self.set_mode(IslandMode::CompactPill);
                    }
                    return true;
                }

                self.is_pressed = true;
                self.is_swiping = false;
                self.press_time = Instant::now();
                self.last_interaction = Instant::now();
                self.press_pos = [lx, ly];

                if self.mode == IslandMode::PowerMenu
                    && self.last_mode_cycle.elapsed() >= Duration::from_millis(400)
                {
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
                true
            }
            PointerEventKind::Release { button, .. } => {
                let lx = event.position.0 as f32;
                let ly = event.position.1 as f32;
                let dist =
                    ((lx - self.press_pos[0]).powi(2) + (ly - self.press_pos[1]).powi(2)).sqrt();

                if self.is_inside_island(lx, ly)
                    && !self.is_swiping
                    && dist < 8.0
                    && self.press_time.elapsed() < Duration::from_millis(600)
                    && self.last_mode_cycle.elapsed() >= Duration::from_millis(400)
                {
                    if button == 273 {
                        // Right-click toggles between Attached (docked to top edge) and Floating styles
                        self.toggle_dock_mode();
                    } else if button == 274 {
                        // Middle click cycles desktop glass style mode
                        self.style_mode = (self.style_mode + 1.0) % 3.0;
                        self.needs_redraw = true;
                    } else {
                        self.handle_click(self.press_pos[0], self.press_pos[1]);
                    }
                }
                self.is_pressed = false;
                self.is_swiping = false;
                self.power_held = None;
                true
            }
            PointerEventKind::Axis {
                horizontal,
                vertical,
                ..
            } => {
                let lx = self.cursor_pos[0] * self.width as f32;
                let ly = self.cursor_pos[1] * self.height as f32;
                if !self.is_inside_island(lx, ly) || self.mode == IslandMode::Hidden {
                    return false;
                }

                self.last_interaction = Instant::now();
                self.is_swiping = true;
                self.power_held = None;

                let delta = if horizontal.absolute.abs() > 30.0 {
                    horizontal.absolute
                } else if vertical.absolute.abs() > 30.0 {
                    vertical.absolute
                } else {
                    0.0
                };

                if delta.abs() > 30.0 && self.last_mode_cycle.elapsed() > Duration::from_millis(450)
                {
                    if delta > 0.0 {
                        self.cycle_mode_forward();
                    } else {
                        self.cycle_mode_backward();
                    }
                }
                true
            }
        }
    }

    fn page_index(mode: IslandMode) -> Option<usize> {
        match mode {
            IslandMode::MusicExpanded => Some(0),
            IslandMode::SystemOverview => Some(1),
            IslandMode::QuickAccess => Some(2),
            IslandMode::PowerMenu => Some(3),
            IslandMode::Notification => None,
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

        let dock_y = self.dock_mode.dock_y();

        match new_mode {
            IslandMode::Hidden => {
                self.target_w = 160.0;
                self.target_h = 10.0;
                self.target_y = -30.0;
                self.target_alpha = 0.0;
            }
            IslandMode::CompactPill => {
                self.target_w = Tokens::ISLAND_COMPACT_W;
                self.target_h = Tokens::ISLAND_COMPACT_H;
                self.target_y = dock_y;
                self.target_alpha = 1.0;
            }
            IslandMode::MusicExpanded
            | IslandMode::Notification
            | IslandMode::QuickAccess
            | IslandMode::SystemOverview
            | IslandMode::PowerMenu => {
                // Unified standard geometry: all expanded pages have identical 460x120px footprint
                self.target_w = Tokens::ISLAND_EXPANDED_W;
                self.target_h = Tokens::ISLAND_EXPANDED_H;
                self.target_y = dock_y;
                self.target_alpha = 1.0;
            }
        }
        self.last_interaction = Instant::now();
        self.last_mode_cycle = Instant::now();
        self.power_held = None;
        self.animating = true;
        self.needs_redraw = true;
        self.needs_input_region_update = true;
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

    #[allow(dead_code)]
    pub fn config_path() -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.config/liquid-glass/config.json", home)
    }

    pub fn load_config_values() -> (IslandDockMode, f32) {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf_path = format!("{}/.config/liquid-glass/config.json", home);
        if let Ok(content) = std::fs::read_to_string(&conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let dock_mode = val
                    .get("edge_style")
                    .and_then(|s| s.as_str())
                    .map(IslandDockMode::from_str)
                    .unwrap_or(IslandDockMode::AttachedCurved);
                let fillet_radius = val
                    .get("fillet_radius")
                    .and_then(|f| f.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(24.0);
                return (dock_mode, fillet_radius);
            }
        }
        let dock_path = format!("{}/.config/liquid-glass/island_dock.txt", home);
        if let Ok(content) = std::fs::read_to_string(&dock_path) {
            (IslandDockMode::from_str(&content), 24.0)
        } else {
            (IslandDockMode::AttachedCurved, 24.0)
        }
    }

    pub fn save_config(&self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = format!("{}/.config/liquid-glass", home);
        let _ = std::fs::create_dir_all(&dir);
        let json = serde_json::json!({
            "edge_style": self.dock_mode.as_str(),
            "fillet_radius": self.fillet_radius,
            "theme_mode": self.theme_mode,
            "style_mode": self.style_mode,
        });
        if let Ok(formatted) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(format!("{dir}/config.json"), formatted);
        }
        let _ = std::fs::write(format!("{dir}/island_dock.txt"), self.dock_mode.as_str());
    }

    pub fn set_dock_mode(&mut self, mode: IslandDockMode) {
        self.dock_mode = mode;
        if self.mode != IslandMode::Hidden {
            self.target_y = mode.dock_y();
            self.animating = true;
            self.needs_redraw = true;
            self.needs_input_region_update = true;
        }
        self.save_config();
    }

    pub fn toggle_dock_mode(&mut self) {
        let next = self.dock_mode.next();
        self.set_dock_mode(next);
    }

    pub fn set_fillet_radius(&mut self, r: f32) {
        self.fillet_radius = r.clamp(8.0, 48.0);
        self.needs_redraw = true;
        self.needs_input_region_update = true;
        self.save_config();
    }

    pub fn reload_config(&mut self) {
        let (mode, r) = Self::load_config_values();
        self.fillet_radius = r;
        self.set_dock_mode(mode);
    }

    fn toggle(&mut self) {
        match self.mode {
            IslandMode::Hidden => self.set_mode(IslandMode::CompactPill),
            _ => self.set_mode(IslandMode::Hidden),
        }
    }

    pub fn check_ipc(&mut self) -> (Option<f32>, Option<u32>) {
        let mut style_changed = None;
        let mut theme_changed = None;
        while let Ok(evt) = self.ipc_rx.try_recv() {
            match evt {
                IslandIpcEvent::Toggle => self.toggle(),
                IslandIpcEvent::ShowMode(m) => self.set_mode(m),
                IslandIpcEvent::NextMenu => self.cycle_mode_forward(),
                IslandIpcEvent::PrevMenu => self.cycle_mode_backward(),
                IslandIpcEvent::SetStyle(s) => {
                    self.style_mode = s;
                    self.needs_redraw = true;
                    style_changed = Some(s);
                    self.save_config();
                }
                IslandIpcEvent::CycleStyle => {
                    self.style_mode = (self.style_mode + 1.0) % 3.0;
                    self.needs_redraw = true;
                    style_changed = Some(self.style_mode);
                    self.save_config();
                }
                IslandIpcEvent::SetTheme(t) => {
                    self.theme_mode = t;
                    self.needs_redraw = true;
                    theme_changed = Some(t);
                    self.save_config();
                }
                IslandIpcEvent::CycleTheme => {
                    self.theme_mode = (self.theme_mode + 1) % 3;
                    self.needs_redraw = true;
                    theme_changed = Some(self.theme_mode);
                    self.save_config();
                }
                IslandIpcEvent::Notification(notif) => {
                    self.active_notif = Some(notif);
                    self.set_mode(IslandMode::Notification);
                }
                IslandIpcEvent::SetDock(d) => self.set_dock_mode(d),
                IslandIpcEvent::ToggleDock => self.toggle_dock_mode(),
                IslandIpcEvent::SetFillet(r) => self.set_fillet_radius(r),
                IslandIpcEvent::ReloadConfig => self.reload_config(),
            }
        }
        (style_changed, theme_changed)
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        self.check_window_overlap();

        let spring_k = 380.0f32;
        let damping = 33.1f32;

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
            && (self.current_alpha - self.target_alpha).abs() < 0.01
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
            self.current_y = self.target_y;
            self.current_alpha = self.target_alpha;
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

        // Advance live/simulated music playback and spinning disc
        if self.music.is_playing && self.mode != IslandMode::Hidden {
            self.music.elapsed_sec += dt;
            self.music.disc_rotation += dt * 3.5;
            if self.music.elapsed_sec >= self.music.duration_sec {
                self.music.elapsed_sec = 0.0;
            }
            self.music.progress =
                (self.music.elapsed_sec / self.music.duration_sec).clamp(0.0, 1.0);
        }

        // When expanded, retract back after 4s of no user interaction
        if !self.is_hovered
            && !self.is_pressed
            && matches!(
                self.mode,
                IslandMode::MusicExpanded
                    | IslandMode::QuickAccess
                    | IslandMode::SystemOverview
                    | IslandMode::PowerMenu
            )
            && self.last_interaction.elapsed() >= Duration::from_millis(4000)
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
            || self.mode == IslandMode::Notification
            || self.is_hovered
            || hold_active
    }

    pub fn check_music(&mut self) {
        let poll_interval = if self.music.is_playing {
            Duration::from_millis(2000)
        } else {
            Duration::from_millis(6000)
        };
        if self.last_music_poll.elapsed() < poll_interval {
            return;
        }
        self.last_music_poll = Instant::now();

        // Query playerctl if available
        if let Ok(out) = Command::new("playerctl").args(["status"]).output() {
            if out.status.success() {
                let status_str = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                let was_playing = self.music.is_playing;
                self.music.is_playing = status_str.contains("playing");
                if self.music.is_playing && !was_playing && self.mode == IslandMode::Hidden {
                    self.set_mode(IslandMode::CompactPill);
                }

                if let Ok(meta_out) = Command::new("playerctl")
                    .args([
                        "metadata",
                        "--format",
                        "{{title}}||{{artist}}||{{mpris:length}}||{{mpris:artUrl}}",
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
                        if parts.len() > 3 {
                            let art_url = parts[3].trim().to_string();
                            if art_url != self.music.art_url {
                                self.music.art_url = art_url.clone();
                                self.music.art_thumbnail = None;
                                if art_url.starts_with("file://") {
                                    let raw_path = art_url.trim_start_matches("file://");
                                    let clean_path = raw_path.replace("%20", " ");
                                    if let Ok(img) = image::open(&clean_path) {
                                        let thumb = img
                                            .resize_exact(
                                                46,
                                                46,
                                                image::imageops::FilterType::Triangle,
                                            )
                                            .to_rgba8();
                                        self.music.art_thumbnail = Some(thumb.into_raw());
                                    }
                                }
                            }
                        }
                    }
                }

                if self.music.is_playing {
                    if let Ok(pos_out) = Command::new("playerctl").args(["position"]).output() {
                        if pos_out.status.success() {
                            if let Ok(pos_sec) = String::from_utf8_lossy(&pos_out.stdout)
                                .trim()
                                .parse::<f32>()
                            {
                                self.music.elapsed_sec = pos_sec;
                                if self.music.duration_sec > 0.0 {
                                    self.music.progress =
                                        (pos_sec / self.music.duration_sec).clamp(0.0, 1.0);
                                }
                            }
                        }
                    }
                }
            } else {
                self.music.is_playing = false;
                if self.music.title.is_empty() || self.music.title == "Starboy" {
                    self.music.title = "No Media Playing".to_string();
                    self.music.artist = "System Idle".to_string();
                }
            }
        } else {
            self.music.is_playing = false;
            if self.music.title.is_empty() || self.music.title == "Starboy" {
                self.music.title = "No Media Playing".to_string();
                self.music.artist = "System Idle".to_string();
            }
        }
    }

    pub fn check_window_overlap(&mut self) {
        let check_interval = if self.mode == IslandMode::Hidden {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(80)
        };
        if self.last_window_check.elapsed() < check_interval {
            return;
        }
        self.last_window_check = Instant::now();

        // Stable monitor top-center zone: x in 845..1075, y in 0..50
        let mon_w = 1920i32;
        let check_w = 230i32;
        let check_x = (mon_w - check_w) / 2;
        let check_y = 0i32;
        let check_h = 50i32;

        let overlapping = Self::query_hyprland_overlap(check_x, check_y, check_w, check_h);
        if overlapping != self.is_window_overlapping {
            self.is_window_overlapping = overlapping;
            self.animating = true;
            self.needs_redraw = true;
            self.needs_input_region_update = true;
        }

        if !overlapping {
            // Always show on desktop, whether previously hidden or not
            self.hot_edge_dwell_start = None;
            if self.mode == IslandMode::Hidden {
                self.set_mode(IslandMode::CompactPill);
            }
        } else {
            // Apps/windows are overlapping underneath the island zone
            if let Some((cx, cy)) = Self::query_hyprland_cursor() {
                // Reaching top edge: x in [780, 1140], y <= 6
                let is_at_top_edge = (780..=1140).contains(&cx) && cy <= 6;
                let island_w = self.current_w as i32;
                let island_left = (mon_w - island_w) / 2;
                let island_right = island_left + island_w;
                let island_bottom = (self.current_y + self.current_h + 12.0) as i32;
                let is_inside_island =
                    (island_left..=island_right).contains(&cx) && (0..=island_bottom).contains(&cy);

                if is_at_top_edge {
                    if self.mode == IslandMode::Hidden {
                        self.set_mode(IslandMode::CompactPill);
                    }
                    self.last_interaction = Instant::now();
                    self.hot_edge_dwell_start = None;
                } else if is_inside_island && self.mode != IslandMode::Hidden {
                    self.last_interaction = Instant::now();
                    self.hot_edge_dwell_start = None;
                } else {
                    self.hot_edge_dwell_start = None;
                    // Outside the island while an app is underneath: auto-hide after brief idle
                    if self.mode != IslandMode::Hidden
                        && self.last_interaction.elapsed() >= Duration::from_millis(1200)
                    {
                        self.set_mode(IslandMode::Hidden);
                    }
                }
            } else if self.mode != IslandMode::Hidden {
                self.set_mode(IslandMode::Hidden);
            }
        }
    }

    fn query_hyprland_cursor() -> Option<(i32, i32)> {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;

        let his = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
        let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
            let uid = unsafe { libc::getuid() };
            format!("/run/user/{}", uid)
        });
        let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, his);
        let mut stream = UnixStream::connect(&sock_path).ok()?;
        stream.write_all(b"cursorpos").ok()?;
        let mut resp = String::new();
        stream.read_to_string(&mut resp).ok()?;
        let parts: Vec<&str> = resp.trim().split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let x = parts[0].parse::<i32>().ok()?;
            let y = parts[1].parse::<i32>().ok()?;
            Some((x, y))
        } else {
            None
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

    #[allow(dead_code)]
    fn check_wallpaper(&mut self) {}

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
                        [12, 16, 24, 255]
                    } else {
                        [255, 255, 255, 255]
                    },
                )
            } else {
                (
                    2.0f32,
                    if is_light {
                        [140, 150, 165, 180]
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
        let req_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != req_len {
            self.overlay_buffer.resize(req_len, 0);
        }
        self.overlay_buffer.fill(0);

        if self.current_alpha <= 0.01 {
            return;
        }

        let mut overlay_vec = std::mem::take(&mut self.overlay_buffer);
        let buf = &mut overlay_vec[..];

        let is_light = self.effective_is_light();
        let col_ink = if is_light {
            [8, 12, 20, 255]
        } else {
            Tokens::TEXT_PRIMARY
        };
        let col_dim = if is_light {
            [18, 24, 34, 255]
        } else {
            [235, 240, 252, 255]
        };
        let col_shadow = [0, 0, 0, 0];

        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let island_w = self.current_w;
        let island_h = self.current_h;
        let island_cy = island_top + island_h * 0.5;

        match self.mode {
            IslandMode::CompactPill => {
                let disc_cx = island_cx - island_w * 0.5 + 22.0;
                let right_x = island_cx + island_w * 0.5 - 22.0;

                if self.music.is_playing {
                    // ── Active Music Playing Mode ──
                    // Left: Realistic Spinning Vinyl Record Disc (11px radius)
                    UIFont::draw_spinning_vinyl(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        island_cy,
                        11.0,
                        self.music.disc_rotation,
                        is_light,
                    );

                    // Center: Track Title
                    let title = safe_truncate(&self.music.title, 16);
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        &title,
                        island_cx,
                        island_cy,
                        Tokens::TYPE_BODY_SM,
                        col_ink,
                        col_shadow,
                        FontWeight::SemiBold,
                    );

                    // Right: Dynamic animated 3-bar equalizer
                    let t_sec = self.start_time.elapsed().as_secs_f32();
                    for i in 0..3 {
                        let bx = right_x - 6.0 + (i as f32) * 6.0;
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
                            col_ink,
                        );
                    }
                } else {
                    // ── Idle / Paused Mode (Dynamic Status Island) ──
                    // Left: Clean Clock Lucide Icon
                    UIFont::draw_clock_icon(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        island_cy,
                        col_ink,
                    );

                    // Center: High-legibility Digital Clock
                    let now = chrono::Local::now();
                    let time_str = now.format("%I:%M %p").to_string();
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        &time_str,
                        island_cx,
                        island_cy,
                        Tokens::TYPE_BODY_SM,
                        col_ink,
                        col_shadow,
                        FontWeight::Bold,
                    );

                    // Right: Crisp Live Battery Indicator
                    UIFont::draw_battery_icon(
                        buf,
                        self.width,
                        self.height,
                        right_x,
                        island_cy,
                        self.sys_stats.bat_pct,
                        self.sys_stats.bat_charging,
                        col_ink,
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

                // Album Art Thumbnail or Spinning Vinyl Disc (radius = 23.0)
                let disc_cx = cx - island_w * 0.5 + 50.0;
                let disc_cy = island_top + 68.0;
                if let Some(thumb) = &self.music.art_thumbnail {
                    UIFont::draw_rounded_thumbnail(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        disc_cy,
                        thumb,
                        46,
                        46,
                        8.0,
                    );
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        disc_cy,
                        23.0,
                        23.0,
                        8.0,
                        1.0,
                        if is_light {
                            [0, 0, 0, 45]
                        } else {
                            [255, 255, 255, 70]
                        },
                    );
                } else {
                    UIFont::draw_spinning_vinyl(
                        buf,
                        self.width,
                        self.height,
                        disc_cx,
                        disc_cy,
                        23.0,
                        self.music.disc_rotation,
                        is_light,
                    );
                }

                // Dynamic Multi-Bar Audio Frequency Spectrum Visualizer (placed in center channel)
                let vis_cx = cx + 42.0;
                let vis_cy = island_top + 68.0;
                let t_sec = self.start_time.elapsed().as_secs_f32();
                UIFont::draw_audio_visualizer(
                    buf,
                    self.width,
                    self.height,
                    vis_cx,
                    vis_cy,
                    t_sec,
                    self.music.is_playing,
                    col_ink,
                );

                // Track details
                let title_x = cx - island_w * 0.5 + 88.0;
                let title = safe_truncate(&self.music.title, 20);
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

                let is_media_active = self.music.is_playing;
                let ctrl_col = if is_media_active {
                    col_ink
                } else {
                    // Dimmed to 40% when idle / no active playback
                    [col_ink[0], col_ink[1], col_ink[2], 102]
                };

                // Previous
                UIFont::draw_prev_icon(
                    buf,
                    self.width,
                    self.height,
                    prev_cx,
                    ctrl_y,
                    8.5,
                    ctrl_col,
                );

                // Play / Pause Button with circle fill (half_w = 18.0, half_h = 18.0)
                let btn_circle_fill = if is_media_active {
                    if is_light {
                        [20, 24, 34, 255]
                    } else {
                        [245, 248, 255, 255]
                    }
                } else {
                    // Muted 24% fill when idle
                    if is_light {
                        [20, 24, 34, 60]
                    } else {
                        [245, 248, 255, 60]
                    }
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    play_cx,
                    ctrl_y,
                    18.0,
                    18.0,
                    18.0,
                    btn_circle_fill,
                );

                let play_icon_col = if is_media_active {
                    if is_light {
                        [255, 255, 255, 255]
                    } else {
                        [10, 14, 20, 255]
                    }
                } else {
                    // Muted play icon when idle
                    if is_light {
                        [20, 24, 34, 180]
                    } else {
                        [245, 248, 255, 180]
                    }
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
                UIFont::draw_next_icon(
                    buf,
                    self.width,
                    self.height,
                    next_cx,
                    ctrl_y,
                    8.5,
                    ctrl_col,
                );

                // Progress Bar (at y = 104.0)
                let bar_y = island_top + 104.0;
                let bar_w = island_w - 64.0;
                let bar_x = cx;
                // High-contrast track back (half_w = bar_w * 0.5, half_h = 2.0)
                let track_bg = if is_media_active {
                    if is_light {
                        [160, 170, 190, 200]
                    } else {
                        [140, 155, 185, 160]
                    }
                } else {
                    // Flat muted idle track back
                    if is_light {
                        [160, 170, 190, 75]
                    } else {
                        [140, 155, 185, 55]
                    }
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    bar_x,
                    bar_y,
                    bar_w * 0.5,
                    2.0,
                    2.0,
                    track_bg,
                );

                // Active fill and scrubber only shown when media is active
                if is_media_active {
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
                            col_ink,
                        );
                        // Scrubber knob
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
                            if is_light {
                                [20, 24, 32, 255]
                            } else {
                                [255, 255, 255, 255]
                            },
                        );
                    }
                }
            }
            IslandMode::Notification => {
                if let Some(notif) = &self.active_notif {
                    // Left App Icon badge (half_w = 20.0, half_h = 20.0) with clean monochrome badge
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
                        if is_light {
                            [24, 28, 38, 255]
                        } else {
                            [245, 248, 255, 255]
                        },
                    );
                    UIFont::draw_bell_icon(
                        buf,
                        self.width,
                        self.height,
                        badge_cx,
                        island_cy,
                        9.5,
                        if is_light {
                            [255, 255, 255, 255]
                        } else {
                            [16, 20, 28, 255]
                        },
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
                        col_dim,
                        FontWeight::Bold,
                    );

                    // Summary
                    let summary = safe_truncate(&notif.summary, 28);
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
                    let body = safe_truncate(&notif.body, 36);
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
                        if is_light {
                            [20, 26, 38, 240]
                        } else {
                            [255, 255, 255, 28]
                        },
                    );
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        "now",
                        right_x - 38.0,
                        island_top + 26.0,
                        10.5,
                        if is_light {
                            [255, 255, 255, 255]
                        } else {
                            col_dim
                        },
                        if is_light { [0, 0, 0, 0] } else { col_shadow },
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
                        if is_light {
                            [0, 0, 0, 16]
                        } else {
                            [255, 255, 255, 24]
                        },
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
                            Tokens::INNER_TILE_BG_HOVER_LIGHT
                        } else {
                            Tokens::INNER_TILE_BG_HOVER_DARK
                        }
                    } else if is_light {
                        Tokens::INNER_TILE_BG_LIGHT
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

                    // Top highlight bevel only (inset 10px from tile radius)
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 10.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 10.0, cy - 27.0),
                        1.0,
                        if is_light {
                            Tokens::INNER_TOP_HIGHLIGHT_LIGHT
                        } else {
                            Tokens::INNER_TOP_HIGHLIGHT_DARK
                        },
                    );

                    let icon_cy = cy - 7.0;
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
                        col_ink,
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
                    col_dim,
                    FontWeight::Bold,
                );

                // Interactive Style / Theme Switcher Badge & Dock Switcher Badge
                let theme_cx = cx + island_w * 0.5 - 130.0;
                let theme_cy = island_top + 22.0;
                let style_lbl = match self.style_mode as u32 {
                    0 => "Obsidian",
                    1 => "Frosted",
                    _ => "Liquid",
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    theme_cx,
                    theme_cy,
                    42.0,
                    9.5,
                    5.0,
                    if is_light {
                        [225, 230, 240, 180]
                    } else {
                        [40, 48, 64, 180]
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    &format!("{} ⟳", style_lbl),
                    theme_cx,
                    theme_cy,
                    10.5,
                    col_ink,
                    col_shadow,
                    FontWeight::Medium,
                );

                // Dock Mode Switcher Badge (Attached vs Floating)
                let dock_cx = theme_cx - 86.0;
                let dock_lbl = match self.dock_mode {
                    IslandDockMode::AttachedCurved => "Edge: Notch",
                    IslandDockMode::AttachedStraight => "Edge: Flush",
                    IslandDockMode::Floating => "Edge: Float",
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    dock_cx,
                    theme_cy,
                    36.0,
                    9.5,
                    5.0,
                    if is_light {
                        [225, 230, 240, 180]
                    } else {
                        [40, 48, 64, 180]
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    dock_lbl,
                    dock_cx,
                    theme_cy,
                    10.5,
                    col_ink,
                    col_shadow,
                    FontWeight::Medium,
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

                for (idx, (lbl, val, _accent)) in stat_items.iter().enumerate() {
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

                    // Top highlight bevel only (inset 10px from tile radius)
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 10.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 10.0, cy - 27.0),
                        1.0,
                        if is_light {
                            Tokens::INNER_TOP_HIGHLIGHT_LIGHT
                        } else {
                            Tokens::INNER_TOP_HIGHLIGHT_DARK
                        },
                    );

                    // Lucide Vector Icon (clean monochrome B&W)
                    match idx {
                        0 => UIFont::draw_cpu_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            col_ink,
                        ),
                        1 => UIFont::draw_ram_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            col_ink,
                        ),
                        2 => {
                            let bat_col =
                                if self.sys_stats.bat_pct < 20 && !self.sys_stats.bat_charging {
                                    Tokens::STATUS_AMBER
                                } else {
                                    col_ink
                                };
                            UIFont::draw_battery_icon(
                                buf,
                                self.width,
                                self.height,
                                cx,
                                cy - 12.0,
                                self.sys_stats.bat_pct,
                                self.sys_stats.bat_charging,
                                bat_col,
                            );
                        }
                        _ => UIFont::draw_clock_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 12.0,
                            col_ink,
                        ),
                    }

                    // Value (tabular text for optical clarity)
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        val,
                        cx,
                        cy + 6.0,
                        Tokens::TYPE_BODY,
                        col_ink,
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
                        col_dim,
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
                    col_dim,
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

                for (idx, (name, _accent_col)) in actions.iter().enumerate() {
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
                            Tokens::INNER_TILE_BG_HOVER_LIGHT
                        } else {
                            Tokens::INNER_TILE_BG_HOVER_DARK
                        }
                    } else if is_light {
                        Tokens::INNER_TILE_BG_LIGHT
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

                    // Top highlight bevel only (inset 10px from tile radius)
                    UIFont::draw_segment(
                        buf,
                        self.width,
                        self.height,
                        (cx - (item_w - 10.0) * 0.5 + 10.0, cy - 27.0),
                        (cx + (item_w - 10.0) * 0.5 - 10.0, cy - 27.0),
                        1.0,
                        if is_light {
                            Tokens::INNER_TOP_HIGHLIGHT_LIGHT
                        } else {
                            Tokens::INNER_TOP_HIGHLIGHT_DARK
                        },
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

                    // Precision Lucide Vector Icon (clean monochrome B&W)
                    match idx {
                        0 => UIFont::draw_lock_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            col_ink,
                        ),
                        1 => UIFont::draw_moon_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            col_ink,
                        ),
                        2 => UIFont::draw_restart_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            col_ink,
                        ),
                        _ => UIFont::draw_power_icon(
                            buf,
                            self.width,
                            self.height,
                            cx,
                            cy - 10.0,
                            col_ink,
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
                        col_ink,
                        col_shadow,
                        FontWeight::SemiBold,
                    );
                }
            }
            _ => {}
        }
    }

    pub fn draw(&mut self) -> bool {
        self.check_ipc();
        self.check_music();
        self.check_window_overlap();
        if self.mode == IslandMode::SystemOverview {
            self.sys_stats.poll();
        }

        self.render_ui_overlay();

        let is_light = if self.effective_is_light() { 1.0 } else { 0.0 };

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
                .curvature(self.fillet_radius)
                .bevel(14.0)
                .refraction(14.0)
                .dispersion(0.12)
                .blur(2.0)
                .specular(1.10)
                .fresnel(0.60)
                .style_mode(self.style_mode)
                .tint(
                    if self.music.is_playing { 1.0 } else { 0.0 },
                    if self.is_pressed { 1.0 } else { 0.0 },
                    self.current_alpha,
                    is_light,
                )
                .element_type(27.0)
                .deformation(self.dock_mode.deformation())
                .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);

            let _ = renderer.render(&[island]);
            self.animating
        } else {
            false
        }
    }

    fn handle_click(&mut self, lx: f32, ly: f32) {
        // Discard any clicks within 400ms of a mode switch / swipe to prevent accidental triggers
        if self.last_mode_cycle.elapsed() < Duration::from_millis(400) {
            return;
        }

        let island_cx = (self.width as f32) * 0.5;
        let island_top = self.current_y;
        let island_w = self.current_w;
        let island_h = self.current_h;

        // Check if click was inside current island capsule, flared wings, or top edge
        let flare = if self.dock_mode.is_curved() && ly <= self.fillet_radius {
            self.fillet_radius
        } else {
            0.0
        };
        if lx < island_cx - (island_w * 0.5 + flare)
            || lx > island_cx + (island_w * 0.5 + flare)
            || ly < 0.0
            || ly > island_top + island_h + 6.0
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

        // Track navigation button exemption (allows rapid skipping without triggering double-click collapse)
        let ctrl_y = island_top + 68.0;
        let play_cx = island_cx + island_w * 0.5 - 64.0;
        let prev_cx = play_cx - 44.0;
        let next_cx = play_cx + 44.0;
        let is_track_nav = self.mode == IslandMode::MusicExpanded
            && (((lx - prev_cx).powi(2) + (ly - ctrl_y).powi(2)).sqrt() <= 20.0
                || ((lx - next_cx).powi(2) + (ly - ctrl_y).powi(2)).sqrt() <= 20.0);

        // Double-click detection: expands compact pill and collapses/hides expanded pages
        if !is_track_nav {
            let click_dt = self.last_click_time.elapsed();
            let click_dx = lx - self.last_click_pos[0];
            let click_dy = ly - self.last_click_pos[1];
            let click_dist = (click_dx * click_dx + click_dy * click_dy).sqrt();

            if click_dt < Duration::from_millis(400) && click_dist < 35.0 {
                self.last_click_time = Instant::now() - Duration::from_secs(10);
                if self.mode == IslandMode::CompactPill {
                    self.set_mode(IslandMode::MusicExpanded);
                } else {
                    self.set_mode(IslandMode::CompactPill);
                }
                return;
            }
        }

        self.last_click_time = Instant::now();
        self.last_click_pos = [lx, ly];

        match self.mode {
            IslandMode::CompactPill => {
                // On CompactPill, single click on the disc/icon toggles playback
                let disc_cx = island_cx - island_w * 0.5 + 20.0;
                let island_cy = island_top + island_h * 0.5;
                let d_disc = ((lx - disc_cx).powi(2) + (ly - island_cy).powi(2)).sqrt();
                if d_disc <= 18.0 {
                    let _ = Command::new("playerctl").args(["play-pause"]).spawn();
                    self.music.is_playing = !self.music.is_playing;
                } else {
                    self.set_mode(IslandMode::MusicExpanded);
                }
            }
            IslandMode::MusicExpanded => {
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
                let theme_cx = island_cx + island_w * 0.5 - 130.0;
                let theme_cy = island_top + 22.0;
                let dock_cx = theme_cx - 86.0;
                if (lx - dock_cx).abs() <= 38.0 && (ly - theme_cy).abs() <= 12.0 {
                    self.toggle_dock_mode();
                } else if (lx - theme_cx).abs() <= 45.0 && (ly - theme_cy).abs() <= 12.0 {
                    self.style_mode = (self.style_mode + 1.0) % 3.0;
                    self.needs_redraw = true;
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
        (total_lum / count as f64) > 0.32
    } else {
        false
    }
}
fn start_fifo_listener(tx: Sender<IslandIpcEvent>, wakeup_fd: Option<i32>) {
    std::thread::spawn(move || {
        let _ = std::fs::remove_file(FIFO_PATH);
        unsafe {
            let c_path = std::ffi::CString::new(FIFO_PATH).unwrap();
            libc::mkfifo(c_path.as_ptr(), 0o666);
        }

        let send_evt = |evt: IslandIpcEvent| {
            let _ = tx.send(evt);
            if let Some(fd) = wakeup_fd {
                unsafe {
                    let b = [1u8];
                    libc::write(fd, b.as_ptr() as *const _, 1);
                }
            }
        };

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
                        send_evt(IslandIpcEvent::Toggle);
                    } else if trimmed == "hide" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::Hidden));
                    } else if trimmed == "show" || trimmed == "compact" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::CompactPill));
                    } else if trimmed == "music" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::MusicExpanded));
                    } else if trimmed == "sys" || trimmed == "system" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::SystemOverview));
                    } else if trimmed == "power" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::PowerMenu));
                    } else if trimmed == "apps" || trimmed == "quick" {
                        send_evt(IslandIpcEvent::ShowMode(IslandMode::QuickAccess));
                    } else if trimmed == "next" {
                        send_evt(IslandIpcEvent::NextMenu);
                    } else if trimmed == "prev" {
                        send_evt(IslandIpcEvent::PrevMenu);
                    } else if trimmed == "style obsidian"
                        || trimmed == "obsidian"
                        || trimmed == "style 0"
                    {
                        send_evt(IslandIpcEvent::SetStyle(0.0));
                    } else if trimmed == "style frost"
                        || trimmed == "style glassmorphism"
                        || trimmed == "frost"
                        || trimmed == "style 1"
                    {
                        send_evt(IslandIpcEvent::SetStyle(1.0));
                    } else if trimmed == "style liquid"
                        || trimmed == "liquid"
                        || trimmed == "style 2"
                    {
                        send_evt(IslandIpcEvent::SetStyle(2.0));
                    } else if trimmed == "style" || trimmed == "style cycle" {
                        send_evt(IslandIpcEvent::CycleStyle);
                    } else if trimmed == "theme auto" || trimmed == "auto" || trimmed == "theme 0" {
                        send_evt(IslandIpcEvent::SetTheme(0));
                    } else if trimmed == "theme dark"
                        || trimmed == "dark"
                        || trimmed == "pure black"
                        || trimmed == "theme 1"
                    {
                        send_evt(IslandIpcEvent::SetTheme(1));
                    } else if trimmed == "theme light"
                        || trimmed == "light"
                        || trimmed == "pure white"
                        || trimmed == "theme 2"
                    {
                        send_evt(IslandIpcEvent::SetTheme(2));
                    } else if trimmed == "theme"
                        || trimmed == "theme toggle"
                        || trimmed == "theme cycle"
                    {
                        send_evt(IslandIpcEvent::CycleTheme);
                    } else if trimmed == "edge curved"
                        || trimmed == "edge:curved"
                        || trimmed == "curved"
                        || trimmed == "notch"
                        || trimmed == "fillet"
                        || trimmed == "dock attached"
                        || trimmed == "dock:attached"
                        || trimmed == "attached"
                    {
                        send_evt(IslandIpcEvent::SetDock(IslandDockMode::AttachedCurved));
                    } else if trimmed == "edge straight"
                        || trimmed == "edge:straight"
                        || trimmed == "straight"
                        || trimmed == "flush"
                        || trimmed == "attached_straight"
                    {
                        send_evt(IslandIpcEvent::SetDock(IslandDockMode::AttachedStraight));
                    } else if trimmed == "edge floating"
                        || trimmed == "edge:floating"
                        || trimmed == "floating"
                        || trimmed == "float"
                        || trimmed == "dock floating"
                        || trimmed == "dock:floating"
                    {
                        send_evt(IslandIpcEvent::SetDock(IslandDockMode::Floating));
                    } else if trimmed == "edge toggle"
                        || trimmed == "edge:toggle"
                        || trimmed == "dock toggle"
                        || trimmed == "dock:toggle"
                        || trimmed == "toggle edge"
                        || trimmed == "toggle dock"
                        || trimmed == "dock"
                        || trimmed == "edge"
                    {
                        send_evt(IslandIpcEvent::ToggleDock);
                    } else if trimmed.starts_with("fillet ")
                        || trimmed.starts_with("fillet:")
                        || trimmed.starts_with("radius ")
                        || trimmed.starts_with("radius:")
                    {
                        let num_part = trimmed
                            .trim_start_matches("fillet ")
                            .trim_start_matches("fillet:")
                            .trim_start_matches("radius ")
                            .trim_start_matches("radius:")
                            .trim();
                        if let Ok(r) = num_part.parse::<f32>() {
                            send_evt(IslandIpcEvent::SetFillet(r));
                        }
                    } else if trimmed == "reload"
                        || trimmed == "reload config"
                        || trimmed == "reload_config"
                    {
                        send_evt(IslandIpcEvent::ReloadConfig);
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
                            send_evt(IslandIpcEvent::Notification(NotificationData {
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
        let _ = Command::new("pkill")
            .args(["-f", "python3.*liquid_island"])
            .status();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "mako"])
            .status();
        let _ = Command::new("pkill").args(["-9", "mako"]).status();

        let script = r#"
import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib
import json
import os
import time
import queue
import threading

FIFO_PATH = "/tmp/liquid_island.fifo"

for _ in range(50):
    if os.path.exists(FIFO_PATH):
        break
    time.sleep(0.1)

q = queue.Queue()
def writer():
    while True:
        line = q.get()
        try:
            with open(FIFO_PATH, "a") as f:
                f.write(line + "\n")
                f.flush()
        except Exception:
            pass

threading.Thread(target=writer, daemon=True).start()

DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()

class NotificationServer(dbus.service.Object):
    def __init__(self, bus):
        bus_name = dbus.service.BusName('org.freedesktop.Notifications', bus, do_not_queue=True)
        super().__init__(bus_name, '/org/freedesktop/Notifications')
        self._next_id = 1

    @dbus.service.method('org.freedesktop.Notifications', in_signature='susssasa{sv}i', out_signature='u')
    def Notify(self, app_name, replaces_id, app_icon, summary, body, actions, hints, timeout):
        nid = self._next_id
        self._next_id += 1
        raw_app = str(app_name).strip()
        if not raw_app:
            raw_app = "System"
        payload = json.dumps({"type": "notification", "app": raw_app, "summary": str(summary), "body": str(body)})
        q.put(payload)
        return nid

    @dbus.service.method('org.freedesktop.Notifications', in_signature='', out_signature='ssss')
    def GetServerInformation(self):
        return ("LiquidIsland", "LiquidGlass", "1.0", "1.2")

    @dbus.service.method('org.freedesktop.Notifications', in_signature='', out_signature='as')
    def GetCapabilities(self):
        return ["body", "actions"]

    @dbus.service.method('org.freedesktop.Notifications', in_signature='u', out_signature='')
    def CloseNotification(self, id):
        pass

try:
    server = NotificationServer(bus)
except Exception:
    # Fallback to monitor mode if another service has the name
    try:
        bus_obj = bus.get_object('org.freedesktop.DBus', '/org/freedesktop/DBus')
        bus_iface = dbus.Interface(bus_obj, 'org.freedesktop.DBus.Monitoring')
        bus_iface.BecomeMonitor(["interface='org.freedesktop.Notifications'"], 0)

        def msg_handler(bus, msg):
            if msg.get_member() == 'Notify':
                args = msg.get_args_list()
                raw_app = str(args[0]).strip() if len(args) > 0 and str(args[0]).strip() else 'System'
                summary = str(args[3]) if len(args) > 3 else ''
                body = str(args[4]) if len(args) > 4 else ''
                payload = json.dumps({"type": "notification", "app": raw_app, "summary": summary, "body": body})
                q.put(payload)

        bus.add_message_filter(msg_handler)
    except Exception:
        pass

GLib.MainLoop().run()
"#;
        let child = Command::new("python3").arg("-c").arg(script).spawn();
        if let Ok(mut c) = child {
            let _ = c.wait();
        }
    });
}
