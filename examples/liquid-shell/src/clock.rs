use crate::island::SysStats;
use chrono::Local;
use glass_core::font::{FontWeight, QuicksandFont, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu, Spring, SpringParams};
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

pub const CARD_PAD: f32 = 24.0;
pub const CARD_W: f32 = 340.0;
pub const CARD_H: f32 = 286.0;
pub const COMPACT_W: f32 = 168.0;
pub const COMPACT_H: f32 = 32.0;

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

pub struct ClockWidget {
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
    #[allow(dead_code)]
    pub date_is_light: bool,
    #[allow(dead_code)]
    pub time_is_light: bool,
    #[allow(dead_code)]
    pub bat_is_light: bool,

    pub sys_stats: SysStats,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    pub anim_speed: f32,
    pub enabled: bool,

    // Liquid Edge Emergence
    pub is_docked: bool,
    pub is_compact: bool,
    pub hover_to_expand: bool,
    pub current_w: f32,
    pub target_w: f32,
    pub current_h: f32,
    pub target_h: f32,
    pub size_spring: Spring,
    #[allow(dead_code)]
    pub fillet_radius: f32,
    pub last_hover_time: Instant,
    pub needs_input_region_update: bool,

    // Multi-page sliding carousel
    #[allow(dead_code)]
    pub current_page: usize,
    #[allow(dead_code)]
    pub page_spring: Spring,
    #[allow(dead_code)]
    pub hovered_tab: Option<usize>,
    #[allow(dead_code)]
    pub hovered_row: Option<usize>,
    pub hovered_tile: Option<usize>,
    pub hovered_btn: Option<usize>,
    pub power_held: Option<Instant>,
    pub power_hold_progress: f32,

    pub is_window_overlapping: bool,
    pub last_window_check: Instant,
    pub is_hidden_by_window: bool,
    pub needs_redraw: bool,

    // Attachment mode (right-click cycle)
    pub dock_mode: crate::island::IslandDockMode,
}

impl ClockWidget {
    pub fn effective_is_light(&self) -> bool {
        match self.theme_mode {
            1 => false, // Forced Dark
            2 => true,  // Forced Light
            _ => {
                if let Some(crop) = &self.wallpaper_crop {
                    if self.is_compact || self.size_spring.value < 0.35 {
                        let pill_h = 40.min(self.height) as usize;
                        let row_bytes = (self.width * 4) as usize;
                        let sub_len = (pill_h * row_bytes).min(crop.len());
                        WallpaperManager::is_buffer_light_hysteresis(
                            &crop[..sub_len],
                            self.is_wallpaper_light,
                        )
                    } else {
                        WallpaperManager::is_buffer_light_hysteresis(crop, self.is_wallpaper_light)
                    }
                } else {
                    self.is_wallpaper_light
                }
            }
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
        let width = (CARD_W + CARD_PAD * 2.0) as u32; // 388
        let height = (CARD_H + CARD_PAD * 2.0) as u32; // 334
        let pos_x = 24i32;
        let pos_y = 0i32; // Flush to top bezel for attached notch fillets

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Overlay,
            Some("liquid-glass-control-hub"),
            None,
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer_surface.set_size(width, height);
        layer_surface.set_margin(pos_y, 0, 0, pos_x);
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

        let page_params = SpringParams {
            stiffness: 260.0,
            damping: 26.0,
            mass: 0.9,
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
            pos_x,
            pos_y,

            cursor_pos: [-10.0, -10.0],
            is_hovered: false,

            font: QuicksandFont::new(),
            overlay_buffer: Vec::new(),
            wallpaper_crop: None,
            is_wallpaper_light: false,
            date_is_light: false,
            time_is_light: false,
            bat_is_light: false,

            sys_stats: SysStats::new(),
            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 6.0,
            anim_speed: 1.0,
            enabled: true,

            is_docked: true,
            is_compact: true,
            hover_to_expand: true,
            current_w: COMPACT_W,
            target_w: COMPACT_W,
            current_h: COMPACT_H,
            target_h: COMPACT_H,
            size_spring: Spring::with_params(0.0, spring_params),
            fillet_radius: 18.0,
            last_hover_time: Instant::now(),
            needs_input_region_update: true,

            current_page: 0,
            page_spring: Spring::with_params(0.0, page_params),
            hovered_tab: None,
            hovered_row: None,
            hovered_tile: None,
            hovered_btn: None,
            power_held: None,
            power_hold_progress: 0.0,

            is_window_overlapping: false,
            last_window_check: Instant::now() - Duration::from_secs(60),
            is_hidden_by_window: false,
            needs_redraw: true,

            dock_mode: crate::island::IslandDockMode::AttachedCurved,
        };

        widget.load_config();
        widget.check_window_overlap();
        widget
    }

    pub fn check_window_overlap(&mut self) {
        if self.last_window_check.elapsed() < Duration::from_millis(300) {
            return;
        }
        self.last_window_check = Instant::now();

        // Calculate monitor coordinates of the compact pill
        // Surface left: pos_x = 24. Inside surface, pill is centered at width/2 = 194.
        // Pill center on monitor: 24 + 194 = 218.
        let check_w = self.current_w as i32;
        let check_x = self.pos_x + (self.width as i32 / 2) - check_w / 2;
        let check_y = 0i32;
        let check_h = 50i32;

        let overlapping =
            crate::island::IslandWidget::query_hyprland_overlap(check_x, check_y, check_w, check_h);
        if overlapping != self.is_window_overlapping {
            self.is_window_overlapping = overlapping;
            self.needs_input_region_update = true;
            self.needs_redraw = true;
        }

        let was_hidden = self.is_hidden_by_window;
        if !overlapping {
            // Empty desktop: always show compact pill
            self.is_hidden_by_window = false;
        } else {
            // Window overlapping: hide unless cursor is near top-left hot edge or we're hovered/expanded
            if let Some((cx, cy)) = crate::island::IslandWidget::query_hyprland_cursor() {
                let is_at_top_edge = cx <= 340 && cy <= 12;
                if is_at_top_edge || self.is_hovered || !self.is_compact {
                    self.is_hidden_by_window = false;
                    self.last_hover_time = Instant::now();
                } else if self.last_hover_time.elapsed() >= Duration::from_millis(1200) {
                    self.is_hidden_by_window = true;
                }
            } else if !self.is_hovered && self.is_compact {
                self.is_hidden_by_window = true;
            }
        }

        if was_hidden != self.is_hidden_by_window {
            self.needs_input_region_update = true;
            self.needs_redraw = true;
        }
    }

    pub fn load_config(&mut self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf_path = format!("{}/.config/liquid-glass/config.json", home);
        if let Ok(content) = std::fs::read_to_string(&conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let obj = val.get("control_hub").or_else(|| val.get("clock"));
                if let Some(c) = obj {
                    if let Some(edge) = c
                        .get("edge_style")
                        .or_else(|| c.get("mode"))
                        .and_then(|v| v.as_str())
                    {
                        self.dock_mode = crate::island::IslandDockMode::from_str(edge);
                    }
                    if let Some(e) = c.get("enabled").and_then(|v| v.as_bool()) {
                        self.enabled = e;
                    }
                    if let Some(s) = c.get("style_mode").and_then(|v| v.as_f64()) {
                        self.style_mode = s as f32;
                    }
                    if let Some(b) = c.get("blur_radius").and_then(|v| v.as_f64()) {
                        self.blur_radius = b as f32;
                    }
                    if let Some(spd) = c.get("animation_speed").and_then(|v| v.as_f64()) {
                        self.anim_speed = spd as f32;
                    }
                    if let Some(hov) = c.get("hover_to_expand").and_then(|v| v.as_bool()) {
                        self.hover_to_expand = hov;
                    }
                }
            }
        }
    }

    pub fn save_config(&self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = format!("{}/.config/liquid-glass", home);
        let conf_path = format!("{}/config.json", dir);
        let mut root_val = if let Ok(content) = std::fs::read_to_string(&conf_path) {
            serde_json::from_str::<serde_json::Value>(&content)
                .unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        let mut clock_obj = root_val
            .get("clock")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        clock_obj["edge_style"] = serde_json::json!(self.dock_mode.as_str());
        root_val["clock"] = clock_obj;
        if let Ok(formatted) = serde_json::to_string_pretty(&root_val) {
            let _ = std::fs::write(&conf_path, formatted);
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn init_gpu(&mut self, gpu: &SharedGpu) {
        let mut renderer = gpu
            .create_renderer(self.window_wrapper.clone(), self.width, self.height)
            .expect("Failed to create GlassRenderer for control hub");

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
        let is_light = if self.is_compact {
            let pill_h = 40.min(self.height) as usize;
            let row_bytes = (self.width * 4) as usize;
            let sub_len = (pill_h * row_bytes).min(crop.len());
            WallpaperManager::is_buffer_light_hysteresis(&crop[..sub_len], self.is_wallpaper_light)
        } else {
            WallpaperManager::is_buffer_light_hysteresis(&crop, self.is_wallpaper_light)
        };
        self.is_wallpaper_light = is_light;
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
    }

    pub fn expand(&mut self) {
        self.is_hidden_by_window = false;
        if self.is_compact {
            self.is_compact = false;
            self.size_spring.set_target(1.0);
            self.last_hover_time = Instant::now();
            self.needs_input_region_update = true;
        }
    }

    pub fn collapse(&mut self) {
        if !self.is_compact {
            self.is_compact = true;
            self.hovered_tab = None;
            self.hovered_row = None;
            self.size_spring.set_target(0.0);
            self.needs_input_region_update = true;
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.is_compact {
            self.expand();
        } else {
            self.collapse();
        }
    }

    pub fn current_cx(&self) -> f32 {
        let t = self.size_spring.value.clamp(0.0, 1.0);
        let compact_cx = 24.0 + COMPACT_W * 0.5; // 108.0 (anchored near top-left bezel)
        let expanded_cx = (self.width as f32) * 0.5; // 194.0
        compact_cx * (1.0 - t) + expanded_cx * t
    }

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        let cx = self.current_cx();
        let card_top = self.dock_mode.dock_y();
        let flare = if self.dock_mode.is_curved() && py <= (card_top + self.fillet_radius) {
            self.fillet_radius
        } else {
            0.0
        };
        if self.is_compact {
            let half_w = COMPACT_W * 0.5 + flare;
            px >= (cx - half_w)
                && px <= (cx + half_w)
                && (0.0..=(card_top + COMPACT_H)).contains(&py)
        } else {
            let half_w = CARD_W * 0.5;
            px >= (cx - half_w)
                && px <= (cx + half_w)
                && (card_top..=(card_top + CARD_H)).contains(&py)
        }
    }

    fn hit_test_tile(&self, px: f32, py: f32) -> Option<usize> {
        let card_top = self.dock_mode.dock_y();
        let cx = (self.width as f32) * 0.5;
        let tile_w = 160.0;
        let tile_h = 132.0;
        let centers = [
            (cx - 86.0, card_top + 74.0),
            (cx + 86.0, card_top + 74.0),
            (cx - 86.0, card_top + 218.0),
            (cx + 86.0, card_top + 218.0),
        ];
        for (i, (tcx, tcy)) in centers.iter().enumerate() {
            if (px - tcx).abs() <= tile_w * 0.5 && (py - tcy).abs() <= tile_h * 0.5 {
                return Some(i);
            }
        }
        None
    }

    fn hit_test_action_btn(&self, px: f32, py: f32) -> Option<usize> {
        let card_top = self.dock_mode.dock_y();
        let cx = (self.width as f32) * 0.5;
        let tx = cx + 86.0;
        let ty = card_top + 218.0;
        let btns = [
            (tx - 38.0, ty - 26.0),
            (tx + 38.0, ty - 26.0),
            (tx - 38.0, ty + 28.0),
            (tx + 38.0, ty + 28.0),
        ];
        for (i, (bx, by)) in btns.iter().enumerate() {
            let dx = px - bx;
            let dy = py - by;
            if (dx * dx + dy * dy) <= 24.0 * 24.0 {
                return Some(i);
            }
        }
        None
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        self.needs_input_region_update = false;
        let region = compositor.wl_compositor().create_region(qh, ());
        if !self.enabled {
            self.surface.set_input_region(Some(&region));
            region.destroy();
            self.surface.commit();
            return;
        }
        let cx = self.current_cx() as i32;
        let card_top = self.dock_mode.dock_y() as i32;
        if self.is_hidden_by_window && self.is_compact {
            // Invisible 2px hair strip at top edge for instant touch wake-up
            let pill_w = COMPACT_W as i32;
            let pill_x = cx - pill_w / 2;
            region.add(pill_x, 0, pill_w, 2);
            self.surface.set_input_region(Some(&region));
            region.destroy();
            self.surface.commit();
            return;
        }
        if self.is_compact {
            let flare = if self.dock_mode.is_curved() {
                self.fillet_radius as i32
            } else {
                0
            };
            let pill_w = COMPACT_W as i32 + flare * 2;
            let pill_h = card_top + COMPACT_H as i32;
            let pill_x = cx - COMPACT_W as i32 / 2 - flare;
            region.add(pill_x, 0, pill_w, pill_h);
        } else {
            let card_w = CARD_W as i32;
            let card_h = CARD_H as i32;
            let card_x = cx - card_w / 2;
            region.add(card_x, card_top, card_w, card_h);
        }
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.commit();
    }

    pub fn on_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event.kind {
            PointerEventKind::Enter { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                self.is_hidden_by_window = false;
                self.needs_redraw = true;
                self.needs_input_region_update = true;
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
                self.hovered_tab = None;
                self.hovered_row = None;
                self.hovered_tile = None;
                self.hovered_btn = None;
                self.power_held = None;
                self.last_hover_time = Instant::now();
                self.needs_redraw = true;
                if !self.is_compact {
                    self.collapse();
                }
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
                if self.is_hidden_by_window {
                    self.is_hidden_by_window = false;
                    self.needs_redraw = true;
                    self.needs_input_region_update = true;
                }
                if inside && self.is_compact && self.hover_to_expand {
                    self.expand();
                }

                if !self.is_compact {
                    self.hovered_tile = self.hit_test_tile(px, py);
                    self.hovered_btn = self.hit_test_action_btn(px, py);
                } else {
                    self.hovered_tile = None;
                    self.hovered_btn = None;
                }
                true
            }
            PointerEventKind::Press { button, .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                if !self.is_inside(px, py) {
                    return false;
                }
                // Right-click: cycle attachment mode (AttachedCurved→AttachedStraight→Floating)
                if button == 273 {
                    self.dock_mode = self.dock_mode.next();
                    self.save_config();
                    self.needs_input_region_update = true;
                    self.needs_redraw = true;
                    return true;
                }
                if self.is_compact {
                    self.expand();
                    return true;
                }

                self.last_hover_time = Instant::now();

                // Page indicator click (switch pages)
                let card_top = self.dock_mode.dock_y();
                if py >= (card_top + CARD_H - 24.0)
                    && (px - (self.width as f32 * 0.5)).abs() <= 40.0
                {
                    self.current_page = 1 - self.current_page;
                    self.page_spring.set_target(self.current_page as f32);
                    self.needs_redraw = true;
                    return true;
                }

                // Check 2x2 action buttons in Tile 3 (Bottom-Right)
                if let Some(btn_idx) = self.hit_test_action_btn(px, py) {
                    match btn_idx {
                        0 => {
                            let _ = Command::new("foot").spawn();
                            self.collapse();
                            return true;
                        }
                        1 => {
                            let _ = Command::new("brave").spawn();
                            self.collapse();
                            return true;
                        }
                        2 => {
                            let _ = Command::new("/home/zius/.local/bin/hyprlock")
                                .spawn()
                                .or_else(|_| Command::new("hyprlock").spawn());
                            self.collapse();
                            return true;
                        }
                        3 => {
                            // Hold to Power Off: start timer!
                            self.power_held = Some(Instant::now());
                            return true;
                        }
                        _ => {}
                    }
                }
                true
            }
            PointerEventKind::Release { .. } => {
                self.power_held = None;
                true
            }
            PointerEventKind::Axis {
                vertical,
                horizontal,
                ..
            } => {
                if !self.is_compact {
                    let v = vertical.absolute;
                    let h = horizontal.absolute;
                    if v > 0.5 || h > 0.5 {
                        self.current_page = 1;
                        self.page_spring.set_target(1.0);
                        self.needs_redraw = true;
                    } else if v < -0.5 || h < -0.5 {
                        self.current_page = 0;
                        self.page_spring.set_target(0.0);
                        self.needs_redraw = true;
                    }
                    return true;
                }
                false
            }
        }
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        let eff_dt = dt * self.anim_speed;
        self.size_spring.update(eff_dt);
        self.page_spring.update(eff_dt);

        let t = self.size_spring.value.clamp(0.0, 1.0);
        self.target_w = CARD_W;
        self.target_h = CARD_H;
        self.current_w = COMPACT_W + (self.target_w - COMPACT_W) * t;
        self.current_h = COMPACT_H + (self.target_h - COMPACT_H) * t;

        // Hold to Power Off progress update
        if let Some(start) = self.power_held {
            let elapsed = start.elapsed().as_secs_f32();
            self.power_hold_progress = (elapsed / 1.2).clamp(0.0, 1.0);
            if self.power_hold_progress >= 1.0 {
                let _ = Command::new("systemctl").arg("poweroff").spawn();
                self.power_held = None;
                self.power_hold_progress = 0.0;
                self.collapse();
            }
        } else {
            self.power_hold_progress = (self.power_hold_progress - eff_dt * 4.0).max(0.0);
        }

        // Auto-collapse after inactivity (do not collapse while holding power!)
        if !self.is_compact
            && !self.is_hovered
            && self.power_held.is_none()
            && self.last_hover_time.elapsed().as_secs_f32() > 4.0
        {
            self.collapse();
        }

        !self.size_spring.is_at_rest()
            || !self.page_spring.is_at_rest()
            || self.power_held.is_some()
            || self.power_hold_progress > 0.001
    }

    pub fn render_ui_overlay(&mut self, buf: &mut [u8]) {
        let col_shadow = [0, 0, 0, 0];
        let cx = self.current_cx();
        let t = self.size_spring.value.clamp(0.0, 1.0);
        let is_light = self.effective_is_light();

        let _ram_pct = if self.sys_stats.ram_total_gb > 0.0 {
            ((self.sys_stats.ram_used_gb / self.sys_stats.ram_total_gb) * 100.0).clamp(0.0, 100.0)
                as u32
        } else {
            0
        };

        // ══════════════════════════════════════════════════════════════
        // 1. COMPACT TELEMETRY PILL (t < 0.35) — 100% Symmetrical 3-Zone
        // ══════════════════════════════════════════════════════════════
        if t < 0.35 {
            let comp_alpha = ((0.35 - t) / 0.35).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * comp_alpha) as u8;
            let ink_col = if is_light {
                [10, 14, 24, alpha_u8]
            } else {
                [250, 252, 255, alpha_u8]
            };
            let sub_ink = if is_light {
                [45, 60, 80, (210.0 * comp_alpha) as u8]
            } else {
                [200, 215, 235, (200.0 * comp_alpha) as u8]
            };
            let div_ink = if is_light {
                [25, 40, 60, (26.0 * comp_alpha) as u8]
            } else {
                [255, 255, 255, (24.0 * comp_alpha) as u8]
            };

            let card_top = self.dock_mode.dock_y();
            let pill_cy = card_top + COMPACT_H * 0.5;

            let now = Local::now();

            // ── Symmetrical 3-column layout across 168px capsule ──
            // Center of surface is cx. Capsule spans cx - 84.0 to cx + 84.0.
            // Col 0 center: cx - 52.0
            // Divider 1: cx - 26.0
            // Col 1 center: cx
            // Divider 2: cx + 26.0
            // Col 2 center: cx + 52.0

            // Divider 1: subtle micro separator dot
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cx - 26.0,
                pill_cy,
                1.0,
                1.0,
                1.0,
                div_ink,
            );

            // Divider 2: subtle micro separator dot
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cx + 26.0,
                pill_cy,
                1.0,
                1.0,
                1.0,
                div_ink,
            );

            // ── Col 0: Time (HH:MM) ──
            let time_str = now.format("%H:%M").to_string();
            let time_size = 11.0f32;
            let time_w = self
                .font
                .measure_text(&time_str, time_size, FontWeight::Bold);
            let time_x = (cx - 52.0) - time_w * 0.5;
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &time_str,
                time_x,
                pill_cy,
                time_size,
                ink_col,
                col_shadow,
                FontWeight::Bold,
            );

            // ── Col 1: CPU % (Micro Icon + Value on same baseline) ──
            let cpu_str = format!("{}%", self.sys_stats.cpu_pct);
            let cpu_size = 10.5f32;
            let cpu_w = self
                .font
                .measure_text(&cpu_str, cpu_size, FontWeight::SemiBold);
            let icon_w = 11.0f32;
            let gap = 4.0f32;
            let col1_w = icon_w + gap + cpu_w;
            let col1_start_x = cx - col1_w * 0.5;
            let cpu_icon_cx = col1_start_x + icon_w * 0.5;
            let cpu_text_x = col1_start_x + icon_w + gap;

            UIFont::draw_cpu_icon(buf, self.width, self.height, cpu_icon_cx, pill_cy, sub_ink);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &cpu_str,
                cpu_text_x,
                pill_cy,
                cpu_size,
                ink_col,
                col_shadow,
                FontWeight::SemiBold,
            );

            // ── Col 2: Battery (Micro Icon + Value on same baseline) ──
            let bat_pct = self.sys_stats.bat_pct;
            let bat_str = format!("{}%", bat_pct);
            let bat_size = 10.5f32;
            let bat_w = self
                .font
                .measure_text(&bat_str, bat_size, FontWeight::SemiBold);
            let bat_icon_w = 13.0f32;
            let col2_w = bat_icon_w + gap + bat_w;
            let col2_start_x = (cx + 52.0) - col2_w * 0.5;
            let bat_icon_cx = col2_start_x + bat_icon_w * 0.5;
            let bat_text_x = col2_start_x + bat_icon_w + gap;

            UIFont::draw_battery_icon(
                buf,
                self.width,
                self.height,
                bat_icon_cx,
                pill_cy,
                bat_pct,
                self.sys_stats.bat_charging,
                ink_col,
            );
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &bat_str,
                bat_text_x,
                pill_cy,
                bat_size,
                ink_col,
                col_shadow,
                FontWeight::SemiBold,
            );
        }

        // ══════════════════════════════════════════════════════════════
        // 2. 2x2 MODULAR SQUIRCLE GLASS CONTROL CENTER (t > 0.05)
        // ══════════════════════════════════════════════════════════════
        if t > 0.05 {
            let exp_alpha = ((t - 0.1) / 0.9).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * exp_alpha) as u8;
            let slide_y = (1.0 - t) * -20.0;
            let card_top = self.dock_mode.dock_y();

            let col_p = if is_light {
                [10, 14, 22, alpha_u8]
            } else {
                [255, 255, 255, alpha_u8]
            };
            let col_sub = if is_light {
                [50, 65, 85, (230.0 * exp_alpha) as u8]
            } else {
                [215, 228, 245, (230.0 * exp_alpha) as u8]
            };
            let tile_base = if is_light { [0, 0, 0] } else { [255, 255, 255] };

            let now = Local::now();

            let tile_w = 160.0f32;
            let tile_h = 132.0f32;
            let tile_r = 18.0f32;

            let centers = [
                (cx - 86.0, card_top + 74.0 + slide_y), // Tile 0: Top-Left (Calendar & Session)
                (cx + 86.0, card_top + 74.0 + slide_y), // Tile 1: Top-Right (Hardware Telemetry)
                (cx - 86.0, card_top + 218.0 + slide_y), // Tile 2: Bottom-Left (Battery & Health Streak)
                (cx + 86.0, card_top + 218.0 + slide_y), // Tile 3: Bottom-Right (2x2 Quick Controls)
            ];

            // Draw subtle modular squircle glass card accents (underlying WGPU glass surfaces provide physical refraction & caustics)
            for (i, (tcx, tcy)) in centers.iter().enumerate() {
                let is_tile_hover = self.hovered_tile == Some(i);
                if is_tile_hover {
                    let hover_fill = if is_light {
                        [0, 0, 0, (14.0 * exp_alpha) as u8]
                    } else {
                        [255, 255, 255, (20.0 * exp_alpha) as u8]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        *tcx,
                        *tcy,
                        tile_w * 0.5,
                        tile_h * 0.5,
                        tile_r,
                        hover_fill,
                    );
                }
                let stroke_col = if is_tile_hover {
                    if is_light {
                        [0, 0, 0, (50.0 * exp_alpha) as u8]
                    } else {
                        [255, 255, 255, (90.0 * exp_alpha) as u8]
                    }
                } else if is_light {
                    [0, 0, 0, (18.0 * exp_alpha) as u8]
                } else {
                    [255, 255, 255, (26.0 * exp_alpha) as u8]
                };

                UIFont::draw_rounded_rect_stroke(
                    buf,
                    self.width,
                    self.height,
                    *tcx,
                    *tcy,
                    tile_w * 0.5,
                    tile_h * 0.5,
                    tile_r,
                    1.0,
                    stroke_col,
                );
            }

            // ─────────────────────────────────────────────────────────
            // TILE 0 (TOP-LEFT): DATE & ACTIVE SESSION
            // ─────────────────────────────────────────────────────────
            let (tx0, ty0) = centers[0];
            let day_name = now.format("%A").to_string().to_uppercase();
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &day_name,
                tx0 - 64.0,
                ty0 - 44.0,
                10.0,
                col_sub,
                col_shadow,
                FontWeight::Bold,
            );

            let day_num = now.format("%d").to_string();
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &day_num,
                tx0 - 64.0,
                ty0 - 10.0,
                34.0,
                col_p,
                col_shadow,
                FontWeight::Bold,
            );

            // Vertical Accent Divider
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                tx0 - 64.0,
                ty0 + 29.0,
                1.5,
                10.0,
                1.0,
                [col_p[0], col_p[1], col_p[2], (140.0 * exp_alpha) as u8],
            );
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "Active Session",
                tx0 - 54.0,
                ty0 + 23.0,
                10.5,
                col_p,
                col_shadow,
                FontWeight::Bold,
            );
            let up_str = format!(
                "Up {}h {}m",
                self.sys_stats.uptime_hrs, self.sys_stats.uptime_mins
            );
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &up_str,
                tx0 - 54.0,
                ty0 + 37.0,
                9.5,
                col_sub,
                col_shadow,
                FontWeight::Medium,
            );

            // ─────────────────────────────────────────────────────────
            // TILE 1 (TOP-RIGHT): HARDWARE TELEMETRY (CPU & RAM)
            // ─────────────────────────────────────────────────────────
            let (tx1, ty1) = centers[1];
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "CPU UTILIZATION",
                tx1 - 64.0,
                ty1 - 44.0,
                10.0,
                col_sub,
                col_shadow,
                FontWeight::Bold,
            );

            let cpu_val = format!("{}%", self.sys_stats.cpu_pct);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &cpu_val,
                tx1 - 64.0,
                ty1 - 10.0,
                30.0,
                col_p,
                col_shadow,
                FontWeight::Bold,
            );

            // CPU micro-chip
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                tx1 + 46.0,
                ty1 - 14.0,
                12.0,
                12.0,
                8.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (28.0 * exp_alpha) as u8,
                ],
            );
            UIFont::draw_cpu_icon(buf, self.width, self.height, tx1 + 46.0, ty1 - 14.0, col_p);

            // Memory line
            let ram_str = format!(
                "{:.1} / {:.1} GB RAM",
                self.sys_stats.ram_used_gb, self.sys_stats.ram_total_gb
            );
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &ram_str,
                tx1 - 64.0,
                ty1 + 20.0,
                10.0,
                col_p,
                col_shadow,
                FontWeight::Bold,
            );

            // Dual Progress Rail
            let rail_y = ty1 + 38.0;
            let rail_w = 128.0;
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                tx1,
                rail_y,
                rail_w * 0.5,
                2.5,
                2.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (75.0 * exp_alpha) as u8,
                ],
            );
            let cpu_fw = rail_w * (self.sys_stats.cpu_pct as f32 / 100.0).clamp(0.04, 1.0);
            let cpu_fcx = (tx1 - rail_w * 0.5) + cpu_fw * 0.5;
            let cpu_col = if self.sys_stats.cpu_pct > 80 {
                [255, 69, 58, alpha_u8]
            } else if is_light {
                [10, 14, 22, alpha_u8]
            } else {
                [255, 255, 255, alpha_u8]
            };
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cpu_fcx,
                rail_y,
                cpu_fw * 0.5,
                2.5,
                2.0,
                cpu_col,
            );

            // ─────────────────────────────────────────────────────────
            // TILE 2 (BOTTOM-LEFT): BATTERY & HEALTH STREAK
            // ─────────────────────────────────────────────────────────
            let (tx2, ty2) = centers[2];
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "POWER STATUS",
                tx2 - 64.0,
                ty2 - 44.0,
                10.0,
                col_sub,
                col_shadow,
                FontWeight::Bold,
            );

            let bat_val = format!("{}%", self.sys_stats.bat_pct);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &bat_val,
                tx2 - 64.0,
                ty2 - 10.0,
                30.0,
                col_p,
                col_shadow,
                FontWeight::Bold,
            );

            // Battery micro-chip
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                tx2 + 46.0,
                ty2 - 14.0,
                12.0,
                12.0,
                8.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (28.0 * exp_alpha) as u8,
                ],
            );
            UIFont::draw_battery_icon(
                buf,
                self.width,
                self.height,
                tx2 + 46.0,
                ty2 - 14.0,
                self.sys_stats.bat_pct,
                self.sys_stats.bat_charging,
                col_p,
            );

            let bat_sub = if self.sys_stats.bat_charging {
                "AC Power [Charging] ⚡"
            } else {
                "Battery Health 100%"
            };
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                bat_sub,
                tx2 - 64.0,
                ty2 + 18.0,
                9.5,
                col_sub,
                col_shadow,
                FontWeight::Medium,
            );

            // 7 Day Streak Dots (S M T W T F S)
            let streak_y_lbl = ty2 + 30.0;
            let streak_y_dot = ty2 + 44.0;
            let day_labels = ["S", "M", "T", "W", "T", "F", "S"];
            let today_idx = now.format("%w").to_string().parse::<usize>().unwrap_or(4);
            let dot_start_x = tx2 - 54.0;
            let dot_gap = 18.0f32;

            for (i, lbl) in day_labels.iter().enumerate() {
                let dx = dot_start_x + (i as f32) * dot_gap;
                let is_today = i == today_idx;
                let lbl_col = if is_today { col_p } else { col_sub };
                let lw = self.font.measure_text(lbl, 8.5, FontWeight::Bold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    lbl,
                    dx - lw * 0.5,
                    streak_y_lbl,
                    8.5,
                    lbl_col,
                    col_shadow,
                    FontWeight::Bold,
                );

                if i < today_idx {
                    // Past completed day: Apple Emerald Green badge with checkmark ✓
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        streak_y_dot,
                        6.5,
                        6.5,
                        6.5,
                        [52, 199, 89, (220.0 * exp_alpha) as u8],
                    );
                    UIFont::draw_check_icon(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        streak_y_dot,
                        [255, 255, 255, alpha_u8],
                    );
                } else if is_today {
                    // Today: crisp white / dark ring with active center
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        streak_y_dot,
                        6.5,
                        6.5,
                        6.5,
                        1.5,
                        [col_p[0], col_p[1], col_p[2], alpha_u8],
                    );
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        streak_y_dot,
                        2.5,
                        2.5,
                        2.5,
                        [col_p[0], col_p[1], col_p[2], alpha_u8],
                    );
                } else {
                    // Future day: subtle translucent ring
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        streak_y_dot,
                        6.5,
                        6.5,
                        6.5,
                        1.0,
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (65.0 * exp_alpha) as u8,
                        ],
                    );
                }
            }

            // ─────────────────────────────────────────────────────────
            // TILE 3 (BOTTOM-RIGHT): 2x2 QUICK ACTION CONTROLS & HOLD POWER
            // ─────────────────────────────────────────────────────────
            let (tx3, ty3) = centers[3];
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "QUICK CONTROLS",
                tx3 - 64.0,
                ty3 - 48.0,
                10.0,
                col_sub,
                col_shadow,
                FontWeight::Bold,
            );

            let action_btns = [
                (tx3 - 38.0, ty3 - 26.0, 0), // Terminal
                (tx3 + 38.0, ty3 - 26.0, 1), // Browser
                (tx3 - 38.0, ty3 + 28.0, 2), // Lock
                (tx3 + 38.0, ty3 + 28.0, 3), // Power Off (Hold)
            ];

            for (bx, by, b_idx) in action_btns {
                let is_hover = self.hovered_btn == Some(b_idx);
                let btn_r = 21.0f32;

                if b_idx == 3 && self.power_hold_progress > 0.01 {
                    // DYNAMIC HOLD-TO-POWER-OFF RADIAL PROGRESS
                    let hp = self.power_hold_progress;
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        by,
                        btn_r,
                        btn_r,
                        btn_r,
                        [255, 69, 58, (60.0 + hp * 180.0) as u8],
                    );
                    let halo_r = btn_r + hp * 6.0;
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        by,
                        halo_r,
                        halo_r,
                        halo_r,
                        2.0,
                        [255, 90, 80, (200.0 * hp) as u8],
                    );
                    UIFont::draw_power_icon(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        by,
                        [255, 255, 255, alpha_u8],
                    );

                    let hold_text = if hp > 0.85 { "RELEASE!" } else { "HOLD..." };
                    let tw = self.font.measure_text(hold_text, 8.5, FontWeight::Bold);
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        hold_text,
                        bx - tw * 0.5,
                        by + btn_r + 6.0,
                        8.5,
                        [255, 90, 80, alpha_u8],
                        col_shadow,
                        FontWeight::Bold,
                    );
                } else {
                    let btn_fill_a = if is_hover { 38.0 } else { 16.0 };
                    let btn_stroke_col = if is_hover {
                        if b_idx == 3 {
                            [255, 90, 90, alpha_u8]
                        } else if is_light {
                            [0, 0, 0, (70.0 * exp_alpha) as u8]
                        } else {
                            [255, 255, 255, (100.0 * exp_alpha) as u8]
                        }
                    } else {
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (35.0 * exp_alpha) as u8,
                        ]
                    };

                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        by,
                        btn_r,
                        btn_r,
                        btn_r,
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (btn_fill_a * exp_alpha) as u8,
                        ],
                    );
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        bx,
                        by,
                        btn_r,
                        btn_r,
                        btn_r,
                        1.0,
                        btn_stroke_col,
                    );

                    let icon_col = if is_hover && b_idx == 3 {
                        [255, 100, 100, alpha_u8]
                    } else {
                        col_p
                    };

                    match b_idx {
                        0 => UIFont::draw_terminal_icon(
                            buf,
                            self.width,
                            self.height,
                            bx,
                            by,
                            icon_col,
                        ),
                        1 => UIFont::draw_browser_icon(
                            buf,
                            self.width,
                            self.height,
                            bx,
                            by,
                            icon_col,
                        ),
                        2 => UIFont::draw_moon_icon(buf, self.width, self.height, bx, by, icon_col),
                        _ => {
                            UIFont::draw_power_icon(buf, self.width, self.height, bx, by, icon_col);
                            if is_hover {
                                let tip = "Hold to Off";
                                let tw = self.font.measure_text(tip, 8.0, FontWeight::Bold);
                                self.font.draw_text_centered_y(
                                    buf,
                                    self.width,
                                    self.height,
                                    tip,
                                    bx - tw * 0.5,
                                    by + btn_r + 6.0,
                                    8.0,
                                    [255, 120, 120, alpha_u8],
                                    col_shadow,
                                    FontWeight::Bold,
                                );
                            }
                        }
                    }
                }
            }

            // ── Page Indicator Dots (bottom of expanded panel) ──
            let dot_y = card_top + CARD_H - 12.0 + slide_y;
            let page_t = self.page_spring.value.clamp(0.0, 1.0);
            let active_w = 14.0f32;
            let inactive_w = 5.0f32;
            let dot_h = 4.0f32;

            // Dot 0 (Page 0: System & Telemetry)
            let w0 = active_w * (1.0 - page_t) + inactive_w * page_t;
            let dot0_col = if page_t < 0.5 {
                col_p
            } else {
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (70.0 * exp_alpha) as u8,
                ]
            };
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cx - 10.0,
                dot_y,
                w0 * 0.5,
                dot_h * 0.5,
                2.0,
                dot0_col,
            );

            // Dot 1 (Page 1: Quick Settings & Controls)
            let w1 = inactive_w * (1.0 - page_t) + active_w * page_t;
            let dot1_col = if page_t >= 0.5 {
                col_p
            } else {
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (70.0 * exp_alpha) as u8,
                ]
            };
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cx + 10.0,
                dot_y,
                w1 * 0.5,
                dot_h * 0.5,
                2.0,
                dot1_col,
            );
        }
    }

    pub fn draw(&mut self) -> bool {
        if !self.enabled || (self.is_hidden_by_window && self.is_compact) {
            if let Some(renderer) = &mut self.renderer {
                self.overlay_buffer.fill(0);
                renderer.update_overlay(self.width, self.height, &self.overlay_buffer);
                let _ = renderer.render(&[]);
            }
            return false;
        }

        self.sys_stats.poll();

        let req_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != req_len {
            self.overlay_buffer.resize(req_len, 0);
        }
        self.overlay_buffer.fill(0);
        let mut overlay_vec = std::mem::take(&mut self.overlay_buffer);
        self.render_ui_overlay(&mut overlay_vec);

        let is_light_f32 = if self.effective_is_light() { 1.0 } else { 0.0 };
        let cx = self.current_cx();

        let res = if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &overlay_vec);

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);

            let sw = self.width as f32;
            let sh = self.height as f32;
            let card_top = self.dock_mode.dock_y();
            let t = self.size_spring.value.clamp(0.0, 1.0);

            let surfaces: Vec<GlassSurface> = if t <= 0.05 {
                let card_left = cx - self.current_w * 0.5;
                let x_norm = card_left / sw;
                let y_norm = card_top / sh;
                let w_norm = self.current_w / sw;
                let h_norm = self.current_h / sh;

                vec![GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::Capsule)
                    .bounds(x_norm, y_norm, w_norm, h_norm)
                    .curvature(14.0)
                    .bevel(16.0)
                    .refraction(16.0)
                    .dispersion(0.12)
                    .blur(self.blur_radius)
                    .specular(1.15)
                    .fresnel(0.60)
                    .style_mode(self.style_mode)
                    .tint(0.0, 0.0, 1.0, is_light_f32)
                    .element_type(27.0)
                    .deformation(self.dock_mode.deformation())
                    .pointer_pos(px, py)]
            } else {
                let slide_y = (1.0 - t) * -20.0;
                let tile_w = 160.0f32;
                let tile_h = 132.0f32;
                let tile_r = 18.0f32;
                let centers = [
                    (cx - 86.0, card_top + 74.0 + slide_y),
                    (cx + 86.0, card_top + 74.0 + slide_y),
                    (cx - 86.0, card_top + 218.0 + slide_y),
                    (cx + 86.0, card_top + 218.0 + slide_y),
                ];

                centers
                    .iter()
                    .enumerate()
                    .map(|(i, (tcx, tcy))| {
                        let cur_cx = cx + (tcx - cx) * t;
                        let init_cy = card_top + COMPACT_H * 0.5;
                        let cur_cy = init_cy + (tcy - init_cy) * t;
                        let cur_w = (COMPACT_W * 0.45) * (1.0 - t) + tile_w * t;
                        let cur_h = 28.0 * (1.0 - t) + tile_h * t;
                        let cur_r = 14.0 * (1.0 - t) + tile_r * t;

                        let tx_norm = (cur_cx - cur_w * 0.5) / sw;
                        let ty_norm = (cur_cy - cur_h * 0.5) / sh;
                        let tw_norm = cur_w / sw;
                        let th_norm = cur_h / sh;

                        GlassSurface::new()
                            .material(Material::LiquidGlass)
                            .shape(Shape::RoundedRectangle { radius: cur_r })
                            .bounds(tx_norm, ty_norm, tw_norm, th_norm)
                            .curvature(16.0)
                            .bevel(14.0)
                            .refraction(16.0)
                            .dispersion(0.12)
                            .blur(self.blur_radius)
                            .specular(1.20)
                            .fresnel(0.65)
                            .style_mode(self.style_mode)
                            .tint(0.0, 0.0, 1.0, is_light_f32)
                            .roughness(if i == 2 {
                                self.sys_stats.bat_pct as f32
                            } else {
                                0.0
                            })
                            .element_type(27.0)
                            .deformation(0.0)
                            .pointer_pos(px, py)
                    })
                    .collect()
            };

            let _ = renderer.render(&surfaces);
            true
        } else {
            false
        };
        self.overlay_buffer = overlay_vec;
        res
    }
}
