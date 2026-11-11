use glass_core::font::{FontWeight, QuicksandFont, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu, Spring, SpringParams, Tokens};
use glass_platform::wallpaper::WallpaperManager;
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
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

pub const DOCK_WIN_W: u32 = 640;
pub const DOCK_WIN_H: u32 = 100;
pub const DOCK_EXPANDED_W: f32 = 520.0;
pub const DOCK_EXPANDED_H: f32 = 68.0;
pub const DOCK_COMPACT_W: f32 = 96.0;
pub const DOCK_COMPACT_H: f32 = 4.0;
pub const DOCK_EXPANDED_Y: f32 = 18.0;
pub const DOCK_COMPACT_Y: f32 = 93.0;

pub struct WaylandWindow {
    pub surface: wl_surface::WlSurface,
    pub display: *mut std::ffi::c_void,
}

unsafe impl Send for WaylandWindow {}
unsafe impl Sync for WaylandWindow {}

impl HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let ptr = std::ptr::NonNull::new(self.surface.id().as_ptr() as *mut _).unwrap();
        let handle = WaylandWindowHandle::new(ptr);
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle))) }
    }
}

impl HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let ptr = std::ptr::NonNull::new(self.display).unwrap();
        let handle = WaylandDisplayHandle::new(ptr);
        unsafe { Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(handle))) }
    }
}

fn default_class() -> String {
    String::new()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DockItem {
    pub name: String,
    pub icon: String,
    pub command: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(skip)]
    pub is_running: bool,
    #[serde(skip)]
    pub is_focused: bool,
}

impl DockItem {
    pub fn defaults() -> Vec<Self> {
        vec![
            DockItem {
                name: "Terminal".to_string(),
                icon: "terminal".to_string(),
                command: "foot".to_string(),
                class: "foot".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Browser".to_string(),
                icon: "browser".to_string(),
                command: "brave".to_string(),
                class: "brave".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Editor".to_string(),
                icon: "editor".to_string(),
                command: "code".to_string(),
                class: "code".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Files".to_string(),
                icon: "folder".to_string(),
                command: "thunar".to_string(),
                class: "thunar".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Music".to_string(),
                icon: "music".to_string(),
                command: "spotify".to_string(),
                class: "spotify".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Tasks".to_string(),
                icon: "check".to_string(),
                command: "/home/zius/.local/bin/island-ctl todo".to_string(),
                class: "liquid-shell".to_string(),
                is_running: true,
                is_focused: false,
            },
            DockItem {
                name: "Settings".to_string(),
                icon: "settings".to_string(),
                command: "/home/zius/.local/bin/settings-tui".to_string(),
                class: "settings".to_string(),
                is_running: false,
                is_focused: false,
            },
            DockItem {
                name: "Island".to_string(),
                icon: "sync".to_string(),
                command: "/home/zius/.local/bin/island-ctl toggle".to_string(),
                class: "liquid-shell".to_string(),
                is_running: true,
                is_focused: false,
            },
        ]
    }
}

pub struct DockWidget {
    #[allow(dead_code)]
    pub layer_surface: Option<LayerSurface>,
    pub surface: wl_surface::WlSurface,
    pub window_wrapper: Arc<WaylandWindow>,
    pub renderer: Option<GlassRenderer>,

    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,

    pub cursor_pos: [f32; 2],
    pub is_hovered: bool,

    pub font: QuicksandFont,
    pub overlay_buffer: Vec<u8>,
    pub wallpaper_crop: Option<Vec<u8>>,
    pub is_wallpaper_light: bool,

    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    pub anim_speed: f32,

    pub is_compact: bool,
    pub hover_to_expand: bool,
    pub current_w: f32,
    pub current_h: f32,
    pub current_y: f32,
    pub compact_w: f32,
    pub compact_h: f32,
    pub size_spring: Spring,
    pub last_hover_time: Instant,
    pub needs_input_region_update: bool,

    pub items: Vec<DockItem>,
    pub last_client_poll: Instant,
    pub pressed_item: Option<usize>,
}

impl DockWidget {
    pub fn effective_is_light(&self) -> bool {
        match self.theme_mode {
            1 => false,                   // Forced Dark
            2 => true,                    // Forced Light
            _ => self.is_wallpaper_light, // Auto based on wallpaper
        }
    }

    pub fn new<D>(
        qh: &QueueHandle<D>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        display_ptr: *mut std::ffi::c_void,
    ) -> Self
    where
        D: Dispatch<wl_surface::WlSurface, SurfaceData>
            + Dispatch<ZwlrLayerSurfaceV1, LayerSurfaceData>
            + 'static,
    {
        let width = DOCK_WIN_W;
        let height = DOCK_WIN_H;

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Top,
            Some("liquid-glass-dock"),
            None,
        );

        // Center horizontally at screen bottom
        layer_surface.set_anchor(Anchor::BOTTOM);
        layer_surface.set_size(width, height);
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.commit();

        let window_wrapper = Arc::new(WaylandWindow {
            surface: surface.clone(),
            display: display_ptr,
        });

        let spring_params = SpringParams {
            stiffness: 240.0,
            damping: 24.0,
            mass: 1.0,
            initial_velocity: 0.0,
            epsilon: 0.001,
        };

        let mut widget = Self {
            layer_surface: Some(layer_surface),
            surface,
            window_wrapper,
            renderer: None,

            width,
            height,
            pos_x: 640,
            pos_y: 980,

            cursor_pos: [-10.0, -10.0],
            is_hovered: false,

            font: QuicksandFont::new(),
            overlay_buffer: Vec::new(),
            wallpaper_crop: None,
            is_wallpaper_light: false,

            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 0.0,
            anim_speed: 1.0,

            is_compact: true,
            hover_to_expand: true,
            current_w: DOCK_COMPACT_W,
            current_h: DOCK_COMPACT_H,
            current_y: DOCK_COMPACT_Y,
            compact_w: DOCK_COMPACT_W,
            compact_h: DOCK_COMPACT_H,
            size_spring: Spring::with_params(0.0, spring_params),
            last_hover_time: Instant::now() - Duration::from_secs(60),
            needs_input_region_update: true,

            items: DockItem::defaults(),
            last_client_poll: Instant::now() - Duration::from_secs(60),
            pressed_item: None,
        };

        widget.load_config();
        widget
    }

    pub fn load_config(&mut self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf_path = format!("{}/.config/liquid-glass/config.json", home);
        if let Ok(content) = std::fs::read_to_string(&conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(d) = val.get("dock") {
                    if let Some(apps) = d
                        .get("apps")
                        .and_then(|a| serde_json::from_value::<Vec<DockItem>>(a.clone()).ok())
                    {
                        if !apps.is_empty() {
                            self.items = apps;
                        }
                    }
                    if let Some(h) = d.get("hover_to_expand").and_then(|v| v.as_bool()) {
                        self.hover_to_expand = h;
                    }
                    if let Some(s) = d.get("style_mode").and_then(|v| v.as_f64()) {
                        self.style_mode = s as f32;
                    }
                    if let Some(b) = d.get("blur_radius").and_then(|v| v.as_f64()) {
                        self.blur_radius = b as f32;
                    }
                    if let Some(spd) = d.get("animation_speed").and_then(|v| v.as_f64()) {
                        self.anim_speed = spd as f32;
                    }
                    if let Some(iw) = d.get("indicator_width").and_then(|v| v.as_f64()) {
                        self.compact_w = iw as f32;
                        if self.is_compact {
                            self.current_w = self.compact_w;
                        }
                    }
                    if let Some(ih) = d.get("indicator_height").and_then(|v| v.as_f64()) {
                        self.compact_h = ih as f32;
                        if self.is_compact {
                            self.current_h = self.compact_h;
                        }
                    }
                }
            }
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
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

    pub fn init_gpu(&mut self, gpu: &SharedGpu) {
        let mut renderer = gpu
            .create_renderer(self.window_wrapper.clone(), self.width, self.height)
            .expect("Failed to create GlassRenderer for dock");

        if let Some(crop) = &self.wallpaper_crop {
            renderer.update_background(self.width, self.height, crop);
        }
        self.renderer = Some(renderer);
    }

    pub fn update_wallpaper(&mut self, crop: Vec<u8>) {
        self.is_wallpaper_light =
            WallpaperManager::is_buffer_light_hysteresis(&crop, self.is_wallpaper_light);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
    }

    pub fn expand(&mut self) {
        if self.is_compact {
            self.is_compact = false;
            self.size_spring.set_target(1.0);
            self.last_hover_time = Instant::now();
            self.needs_input_region_update = true;
            self.poll_hyprland_clients();
        }
    }

    pub fn collapse(&mut self) {
        if !self.is_compact {
            self.is_compact = true;
            self.size_spring.set_target(0.0);
            self.needs_input_region_update = true;
            self.pressed_item = None;
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.is_compact {
            self.expand();
        } else {
            self.collapse();
        }
    }

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        let cx = (self.width as f32) * 0.5;
        if self.is_compact {
            // Generous hover trigger zone across bottom 36px
            let half_w = (self.compact_w * 0.5 + 40.0).max(90.0);
            px >= (cx - half_w)
                && px <= (cx + half_w)
                && py >= (DOCK_COMPACT_Y - 24.0)
                && py <= (self.height as f32)
        } else {
            let half_w = self.current_w * 0.5 + 16.0;
            let top = (self.current_y - 12.0).max(0.0);
            let bottom = self.height as f32;
            px >= (cx - half_w) && px <= (cx + half_w) && py >= top && py <= bottom
        }
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        self.needs_input_region_update = false;
        let region = compositor.wl_compositor().create_region(qh, ());
        let cx = self.width as i32 / 2;
        if self.is_compact && self.size_spring.value < 0.1 {
            let pill_w = (self.compact_w + 80.0) as i32;
            let pill_h = 28i32;
            let pill_x = cx - pill_w / 2;
            let pill_y = (DOCK_COMPACT_Y - 20.0) as i32;
            region.add(pill_x, pill_y, pill_w, pill_h);
        } else {
            let card_w = (self.current_w + 24.0) as i32;
            let card_h = (self.current_h + 30.0) as i32;
            let card_x = cx - card_w / 2;
            let card_y = (self.current_y - 10.0).max(0.0) as i32;
            region.add(card_x, card_y, card_w, card_h);
        }
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.commit();
    }

    pub fn poll_hyprland_clients(&mut self) {
        if self.last_client_poll.elapsed() < Duration::from_millis(1500) {
            return;
        }
        self.last_client_poll = Instant::now();

        // Query active windows from Hyprland via hyprctl
        if let Ok(output) = Command::new("hyprctl").arg("clients").arg("-j").output() {
            if output.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(list) = val.as_array() {
                        let mut active_classes = Vec::new();
                        for item in list {
                            if let Some(c) = item.get("class").and_then(|v| v.as_str()) {
                                active_classes.push(c.to_lowercase());
                            }
                        }

                        // Query active window for focus highlight
                        let focused_class = Command::new("hyprctl")
                            .arg("activewindow")
                            .arg("-j")
                            .output()
                            .ok()
                            .and_then(|out| {
                                serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()
                            })
                            .and_then(|val| {
                                val.get("class")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_lowercase())
                            });

                        for item in &mut self.items {
                            let pat = item.class.to_lowercase();
                            item.is_running =
                                !pat.is_empty() && active_classes.iter().any(|c| c.contains(&pat));
                            item.is_focused = !pat.is_empty()
                                && focused_class
                                    .as_ref()
                                    .map(|f| f.contains(&pat))
                                    .unwrap_or(false);
                        }
                    }
                }
            }
        }
    }

    pub fn on_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event.kind {
            PointerEventKind::Enter { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                let inside = self.is_inside(px, py);
                self.is_hovered = inside;
                if inside {
                    self.last_hover_time = Instant::now();
                    if self.is_compact && self.hover_to_expand {
                        self.expand();
                    }
                }
                true
            }
            PointerEventKind::Leave { .. } => {
                self.is_hovered = false;
                self.cursor_pos = [-10.0, -10.0];
                self.last_hover_time = Instant::now();
                self.pressed_item = None;
                true
            }
            PointerEventKind::Motion { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                let nx = px / (self.width as f32);
                let ny = py / (self.height as f32);
                self.cursor_pos = [nx, ny];
                self.last_hover_time = Instant::now();

                let inside = self.is_inside(px, py);
                self.is_hovered = inside;
                if inside && self.is_compact && self.hover_to_expand {
                    self.expand();
                }
                true
            }
            PointerEventKind::Press { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                if !self.is_inside(px, py) {
                    return false;
                }
                if self.is_compact {
                    self.expand();
                    return true;
                }
                self.last_hover_time = Instant::now();

                // Detect item click
                let cx = (self.width as f32) * 0.5;
                let dock_left = cx - self.current_w * 0.5;
                let item_count = self.items.len();
                if item_count > 0 {
                    let total_avail = self.current_w - 32.0;
                    let slot_w = total_avail / (item_count as f32);
                    let start_x = dock_left + 16.0 + slot_w * 0.5;

                    for idx in 0..item_count {
                        let item_cx = start_x + (idx as f32) * slot_w;
                        let item_cy = self.current_y + self.current_h * 0.5;
                        if (px - item_cx).abs() <= slot_w * 0.5
                            && (py - item_cy).abs() <= self.current_h * 0.5
                        {
                            self.pressed_item = Some(idx);
                            break;
                        }
                    }
                }
                true
            }
            PointerEventKind::Release { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;

                if let Some(idx) = self.pressed_item.take() {
                    if idx < self.items.len() {
                        let cx = (self.width as f32) * 0.5;
                        let dock_left = cx - self.current_w * 0.5;
                        let item_count = self.items.len();
                        let total_avail = self.current_w - 32.0;
                        let slot_w = total_avail / (item_count as f32);
                        let item_cx = dock_left + 16.0 + slot_w * 0.5 + (idx as f32) * slot_w;
                        let item_cy = self.current_y + self.current_h * 0.5;

                        if (px - item_cx).abs() <= slot_w * 0.5
                            && (py - item_cy).abs() <= self.current_h * 0.5
                        {
                            let cmd = self.items[idx].command.clone();
                            let _ = Command::new("sh").args(["-c", &cmd]).spawn();
                            self.collapse();
                        }
                    }
                }
                true
            }
            _ => true,
        }
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        let eff_dt = (dt * self.anim_speed).clamp(0.001, 0.05);
        self.size_spring.update(eff_dt);

        let t = self.size_spring.value.clamp(0.0, 1.0);
        self.current_w = self.compact_w + (DOCK_EXPANDED_W - self.compact_w) * t;
        self.current_h = self.compact_h + (DOCK_EXPANDED_H - self.compact_h) * t;
        self.current_y = DOCK_COMPACT_Y + (DOCK_EXPANDED_Y - DOCK_COMPACT_Y) * t;

        // Auto-collapse after inactivity (2.5 seconds unhovered)
        if !self.is_compact
            && !self.is_hovered
            && self.last_hover_time.elapsed().as_secs_f32() > 2.5
        {
            self.collapse();
        }

        !self.size_spring.is_at_rest()
    }

    fn render_ui_overlay(&mut self) {
        let buf_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != buf_len {
            self.overlay_buffer.resize(buf_len, 0);
        }
        self.overlay_buffer.fill(0);

        let t = self.size_spring.value.clamp(0.0, 1.0);

        // When compact (t < 0.15), the GPU glass surface itself is the single sleek glowing bar.
        // No inner rectangle is rendered to avoid double-box / scrollbar artifacts!
        if t < 0.15 {
            return;
        }

        let is_light = self.effective_is_light();
        let col_ink = if is_light {
            [16, 20, 28, 255]
        } else {
            [255, 255, 255, 255]
        };
        let col_dim = if is_light {
            [90, 100, 115, 220]
        } else {
            [180, 195, 215, 220]
        };
        let col_hover_tile = if is_light {
            [0, 0, 0, 24]
        } else {
            [255, 255, 255, 34]
        };

        let cx = (self.width as f32) * 0.5;
        let buf = &mut self.overlay_buffer;

        let dock_left = cx - self.current_w * 0.5;
        let dock_top = self.current_y;
        let dock_w = self.current_w;
        let dock_h = self.current_h;
        let dock_cy = dock_top + dock_h * 0.5;

        let item_count = self.items.len();
        if item_count == 0 {
            return;
        }

        let total_avail = dock_w - 32.0;
        let slot_w = total_avail / (item_count as f32);
        let start_x = dock_left + 16.0 + slot_w * 0.5;

        let cur_lx = self.cursor_pos[0] * (self.width as f32);
        let cur_ly = self.cursor_pos[1] * (self.height as f32);

        let mut hovered_item_idx: Option<usize> = None;
        let mut hovered_item_scale = 1.0f32;
        let mut hovered_item_pos = [0.0f32, 0.0f32];

        for (idx, item) in self.items.iter().enumerate() {
            let base_cx = start_x + (idx as f32) * slot_w;
            let base_cy = dock_cy - 2.0;

            // Parabolic magnification under cursor
            let mut scale = 1.0f32;
            let mut lift = 0.0f32;

            if self.is_hovered && (cur_ly >= dock_top - 10.0 && cur_ly <= dock_top + dock_h + 10.0)
            {
                let dist = (cur_lx - base_cx).abs();
                let sigma = 38.0f32;
                let bell = (-(dist * dist) / (2.0 * sigma * sigma)).exp();
                scale = 1.0 + 0.32 * bell;
                lift = -8.0 * (scale - 1.0) / 0.32;
                if dist < slot_w * 0.45 && scale > 1.15 {
                    hovered_item_idx = Some(idx);
                    hovered_item_scale = scale;
                    hovered_item_pos = [base_cx, base_cy + lift];
                }
            }

            let item_cx = base_cx;
            let item_cy = base_cy + lift;

            // Tile backing on hover
            let tile_r = 18.0 * scale;
            if scale > 1.05 {
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    tile_r,
                    tile_r,
                    12.0,
                    col_hover_tile,
                );
            }

            // Draw Icon
            let icon_col = if self.pressed_item == Some(idx) {
                col_dim
            } else {
                col_ink
            };

            match item.icon.as_str() {
                "terminal" => UIFont::draw_terminal_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "browser" => UIFont::draw_browser_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "editor" => UIFont::draw_editor_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "folder" => UIFont::draw_folder_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "music" => UIFont::draw_music_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "check" => UIFont::draw_check_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "settings" => UIFont::draw_settings_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
                "sync" => {
                    UIFont::draw_sync_icon(buf, self.width, self.height, item_cx, item_cy, icon_col)
                }
                _ => UIFont::draw_terminal_icon(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    item_cy,
                    icon_col,
                ),
            }

            // Running status dot / pill
            let dot_y = dock_top + dock_h - 7.0;
            if item.is_focused {
                // Focused app: elongated luminous pill
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    dot_y,
                    4.5,
                    1.6,
                    1.2,
                    col_ink,
                );
            } else if item.is_running {
                // Running app: sleek precision dot
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    item_cx,
                    dot_y,
                    1.8,
                    1.8,
                    1.8,
                    col_dim,
                );
            }
        }

        // Draw Floating Tooltip Badge for the primary hovered item
        if let Some(idx) = hovered_item_idx {
            if idx < self.items.len() && hovered_item_scale > 1.15 {
                let name = &self.items[idx].name;
                let tip_cx = hovered_item_pos[0];
                let tip_cy = dock_top - 14.0;
                let text_w = (name.len() as f32) * 6.5 + 16.0;

                // Tooltip glass backing
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    tip_cx,
                    tip_cy,
                    text_w * 0.5,
                    9.0,
                    6.0,
                    if is_light {
                        [245, 248, 255, 230]
                    } else {
                        [20, 24, 32, 230]
                    },
                );

                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    name,
                    tip_cx,
                    tip_cy - 4.5,
                    10.5,
                    col_ink,
                    [0, 0, 0, 0],
                    FontWeight::Bold,
                );
            }
        }
    }

    pub fn draw(&mut self) -> bool {
        self.poll_hyprland_clients();
        self.render_ui_overlay();

        let is_light_f32 = if self.effective_is_light() { 1.0 } else { 0.0 };
        let t = self.size_spring.value.clamp(0.0, 1.0);

        if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &self.overlay_buffer);

            let cx = (self.width as f32) * 0.5;
            let card_left = cx - self.current_w * 0.5;
            let x_norm = card_left / (self.width as f32);
            let y_norm = self.current_y / (self.height as f32);
            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);

            let shape = if t < 0.15 {
                Shape::Capsule
            } else {
                Shape::RoundedRectangle {
                    radius: Tokens::RADIUS_OUTER * t + 2.0 * (1.0 - t),
                }
            };

            let dock_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(shape)
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(if t < 0.15 { 2.0 } else { 24.0 })
                .bevel(if t < 0.15 { 1.5 } else { 16.0 })
                .refraction(if t < 0.15 { 6.0 } else { 18.0 })
                .dispersion(0.12)
                .blur(self.blur_radius)
                .specular(1.10)
                .fresnel(0.60)
                .style_mode(self.style_mode)
                .tint(0.0, 0.0, 1.0, is_light_f32)
                .element_type(28.0)
                .deformation(0.0) // Floating pill with 360-degree glass meniscus
                .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);

            let _ = renderer.render(&[dock_card]);
            !self.size_spring.is_at_rest()
        } else {
            false
        }
    }
}
