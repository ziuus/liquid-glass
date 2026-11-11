use crate::island::SysStats;
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
use std::time::Instant;
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

pub const CARD_PAD: f32 = 24.0;
pub const CARD_W: f32 = 340.0;
pub const CARD_H: f32 = 286.0;
pub const COMPACT_W: f32 = 176.0;
pub const COMPACT_H: f32 = 30.0;

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
    pub fillet_radius: f32,
    pub last_hover_time: Instant,
    pub needs_input_region_update: bool,

    // Multi-page sliding carousel
    pub current_page: usize,
    pub page_spring: Spring,
    pub hovered_tab: Option<usize>,
    pub hovered_row: Option<usize>,
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
        let pos_y = 0i32; // Flush against top screen edge

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Bottom,
            Some("liquid-glass-control-hub"),
            None,
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer_surface.set_size(width, height);
        layer_surface.set_margin(0, 0, 0, pos_x);
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

            is_docked: false,
            is_compact: true,
            hover_to_expand: true,
            current_w: COMPACT_W,
            target_w: COMPACT_W,
            current_h: COMPACT_H,
            target_h: COMPACT_H,
            size_spring: Spring::with_params(0.0, spring_params),
            fillet_radius: 12.0,
            last_hover_time: Instant::now(),
            needs_input_region_update: true,

            current_page: 0,
            page_spring: Spring::with_params(0.0, page_params),
            hovered_tab: None,
            hovered_row: None,
        };

        widget.load_config();
        widget
    }

    pub fn load_config(&mut self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf_path = format!("{}/.config/liquid-glass/config.json", home);
        if let Ok(content) = std::fs::read_to_string(&conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let obj = val.get("control_hub").or_else(|| val.get("clock"));
                if let Some(c) = obj {
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

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        let cx = (self.width as f32) * 0.5;
        if self.is_compact {
            let half_w = COMPACT_W * 0.5;
            px >= (cx - half_w) && px <= (cx + half_w) && (0.0..=COMPACT_H).contains(&py)
        } else {
            let half_w = CARD_W * 0.5;
            px >= (cx - half_w) && px <= (cx + half_w) && (0.0..=CARD_H).contains(&py)
        }
    }

    fn hit_test_tab(&self, px: f32, py: f32) -> Option<usize> {
        if py < 14.0 || py > 44.0 {
            return None;
        }
        let cx = (self.width as f32) * 0.5;
        let tab_w = 86.0;
        let start_x = cx - 129.0;
        for i in 0..3 {
            let bx = start_x + (i as f32) * tab_w;
            if px >= bx && px <= bx + tab_w {
                return Some(i);
            }
        }
        None
    }

    fn hit_test_row(&self, px: f32, py: f32) -> Option<usize> {
        let cx = (self.width as f32) * 0.5;
        if px < (cx - 150.0) || px > (cx + 150.0) {
            return None;
        }
        for i in 0..4 {
            let row_cy = 78.0 + (i as f32) * 44.0;
            if (py - row_cy).abs() <= 20.0 {
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
        let cx = self.width as i32 / 2;
        if self.is_compact {
            let pill_w = COMPACT_W as i32;
            let pill_h = COMPACT_H as i32;
            let pill_x = cx - pill_w / 2;
            region.add(pill_x, 0, pill_w, pill_h);
        } else {
            let card_w = CARD_W as i32;
            let card_h = CARD_H as i32;
            let card_x = cx - card_w / 2;
            region.add(card_x, 0, card_w, card_h);
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
                self.last_hover_time = Instant::now();
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
                if inside && self.is_compact && self.hover_to_expand {
                    self.expand();
                }

                if !self.is_compact {
                    self.hovered_tab = self.hit_test_tab(px, py);
                    self.hovered_row = self.hit_test_row(px, py);
                } else {
                    self.hovered_tab = None;
                    self.hovered_row = None;
                }
                true
            }
            PointerEventKind::Axis { vertical, .. } => {
                if !self.is_compact && vertical.absolute != 0.0 {
                    if vertical.absolute > 0.0 {
                        self.current_page = (self.current_page + 1) % 3;
                    } else {
                        self.current_page = (self.current_page + 2) % 3;
                    }
                    self.page_spring.set_target(self.current_page as f32);
                    self.last_hover_time = Instant::now();
                    return true;
                }
                false
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

                // Check tab switcher clicks
                if let Some(tab_idx) = self.hit_test_tab(px, py) {
                    self.current_page = tab_idx;
                    self.page_spring.set_target(tab_idx as f32);
                    return true;
                }

                // Check pagination dot clicks at bottom
                let cx = (self.width as f32) * 0.5;
                if (254.0..=276.0).contains(&py) {
                    for i in 0..3 {
                        let dot_x = cx - 24.0 + (i as f32) * 24.0;
                        if (px - dot_x).abs() <= 16.0 {
                            self.current_page = i;
                            self.page_spring.set_target(i as f32);
                            return true;
                        }
                    }
                }

                // Check action row clicks based on active page
                if let Some(row_idx) = self.hit_test_row(px, py) {
                    match self.current_page {
                        1 => {
                            // Applications Page
                            match row_idx {
                                0 => {
                                    let _ = Command::new("foot").spawn();
                                }
                                1 => {
                                    let _ = Command::new("brave").spawn();
                                }
                                2 => {
                                    let _ = Command::new("code").spawn();
                                }
                                3 => {
                                    let _ = Command::new("thunar").spawn();
                                }
                                _ => {}
                            }
                            self.collapse();
                        }
                        2 => {
                            // Power Page
                            match row_idx {
                                0 => {
                                    let _ = Command::new("/home/zius/.local/bin/hyprlock")
                                        .spawn()
                                        .or_else(|_| Command::new("hyprlock").spawn());
                                }
                                1 => {
                                    let _ = Command::new("systemctl").arg("suspend").spawn();
                                }
                                2 => {
                                    let _ = Command::new("systemctl").arg("reboot").spawn();
                                }
                                3 => {
                                    let _ = Command::new("systemctl").arg("poweroff").spawn();
                                }
                                _ => {}
                            }
                            self.collapse();
                        }
                        _ => {
                            // Vitals Page: click cycles to next page
                            self.current_page = 1;
                            self.page_spring.set_target(1.0);
                        }
                    }
                }
                true
            }
            _ => false,
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

        // Auto-collapse after inactivity
        if !self.is_compact
            && !self.is_hovered
            && self.last_hover_time.elapsed().as_secs_f32() > 4.0
        {
            self.collapse();
        }

        !self.size_spring.is_at_rest() || !self.page_spring.is_at_rest()
    }

    pub fn render_ui_overlay(&mut self, buf: &mut [u8]) {
        let col_shadow = [0, 0, 0, 0];
        let cx = (self.width as f32) * 0.5;
        let t = self.size_spring.value.clamp(0.0, 1.0);
        let is_light = self.effective_is_light();

        let ram_pct = if self.sys_stats.ram_total_gb > 0.0 {
            ((self.sys_stats.ram_used_gb / self.sys_stats.ram_total_gb) * 100.0).clamp(0.0, 100.0)
                as u32
        } else {
            0
        };

        // ══════════════════════════════════════════════════════════════
        // 1. COMPACT OVERVIEW PILL (t < 0.35)
        // ══════════════════════════════════════════════════════════════
        if t < 0.35 {
            let comp_alpha = ((0.35 - t) / 0.35).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * comp_alpha) as u8;
            let ink_col = if is_light {
                [10, 14, 22, alpha_u8]
            } else {
                [255, 255, 255, alpha_u8]
            };
            let sub_ink = if is_light {
                [60, 75, 95, (190.0 * comp_alpha) as u8]
            } else {
                [180, 195, 220, (170.0 * comp_alpha) as u8]
            };

            let pill_cy = 15.0;

            // CPU Segment
            UIFont::draw_cpu_icon(buf, self.width, self.height, cx - 68.0, pill_cy, ink_col);
            let cpu_str = format!("{}%", self.sys_stats.cpu_pct);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &cpu_str,
                cx - 58.0,
                pill_cy,
                10.5,
                ink_col,
                col_shadow,
                FontWeight::SemiBold,
            );

            // Divider 1
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "·",
                cx - 26.0,
                pill_cy,
                9.5,
                sub_ink,
                col_shadow,
                FontWeight::Bold,
            );

            // RAM Segment
            UIFont::draw_ram_icon(buf, self.width, self.height, cx - 15.0, pill_cy, ink_col);
            let ram_str = format!("{}%", ram_pct);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &ram_str,
                cx - 5.0,
                pill_cy,
                10.5,
                ink_col,
                col_shadow,
                FontWeight::SemiBold,
            );

            // Divider 2
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                "·",
                cx + 26.0,
                pill_cy,
                9.5,
                sub_ink,
                col_shadow,
                FontWeight::Bold,
            );

            // Battery Segment
            UIFont::draw_battery_icon(
                buf,
                self.width,
                self.height,
                cx + 37.0,
                pill_cy,
                self.sys_stats.bat_pct,
                self.sys_stats.bat_charging,
                ink_col,
            );
            let bat_str = format!("{}%", self.sys_stats.bat_pct);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &bat_str,
                cx + 49.0,
                pill_cy,
                10.5,
                ink_col,
                col_shadow,
                FontWeight::SemiBold,
            );
        }

        // ══════════════════════════════════════════════════════════════
        // 2. EXPANDED SLIDING CONTROL HUB (t > 0.05)
        // ══════════════════════════════════════════════════════════════
        if t > 0.05 {
            let exp_alpha = ((t - 0.1) / 0.9).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * exp_alpha) as u8;
            let slide_y = (1.0 - t) * -24.0;

            let col_primary = if is_light {
                [10, 14, 22, alpha_u8]
            } else {
                [255, 255, 255, alpha_u8]
            };
            let col_secondary = if is_light {
                [60, 75, 95, (200.0 * exp_alpha) as u8]
            } else {
                [170, 185, 215, (190.0 * exp_alpha) as u8]
            };
            let tile_base = if is_light { [0, 0, 0] } else { [255, 255, 255] };

            // ── SEGMENTED TOP TAB SWITCHER (VITALS | APPS | POWER) ──
            let tab_y = 28.0 + slide_y;
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                cx,
                tab_y,
                130.0,
                12.5,
                9.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (16.0 * exp_alpha) as u8,
                ],
            );

            // Smooth spring sliding indicator pill
            let ind_x = (cx - 86.0) + self.page_spring.value * 86.0;
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                ind_x,
                tab_y,
                42.0,
                11.5,
                8.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (45.0 * exp_alpha) as u8,
                ],
            );
            UIFont::draw_rounded_rect_stroke(
                buf,
                self.width,
                self.height,
                ind_x,
                tab_y,
                42.0,
                11.5,
                8.0,
                1.0,
                [
                    tile_base[0],
                    tile_base[1],
                    tile_base[2],
                    (80.0 * exp_alpha) as u8,
                ],
            );

            // Tab Labels
            let tabs = [("VITALS", cx - 86.0), ("APPS", cx), ("POWER", cx + 86.0)];
            for (idx, (label, tx)) in tabs.iter().enumerate() {
                let is_active = self.current_page == idx;
                let text_col = if is_active {
                    col_primary
                } else {
                    col_secondary
                };
                let tw = self.font.measure_text(label, 10.0, FontWeight::Bold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    label,
                    tx - tw * 0.5,
                    tab_y,
                    10.0,
                    text_col,
                    col_shadow,
                    FontWeight::Bold,
                );
            }

            // ── ACTIVE SLIDING PAGE CONTENT ──
            // Render pages with horizontal translation based on page_spring
            let cur_p = self.page_spring.value;

            // ─────────────────────────────────────────────────────────
            // PAGE 0: HARDWARE TELEMETRY
            // ─────────────────────────────────────────────────────────
            let off0 = (0.0 - cur_p) * CARD_W;
            if off0.abs() < CARD_W {
                let px_c = cx + off0;
                let page_alpha = (1.0 - (off0.abs() / CARD_W)).clamp(0.0, 1.0) * exp_alpha;
                let a8 = (255.0 * page_alpha) as u8;
                let col_p = if is_light {
                    [10, 14, 22, a8]
                } else {
                    [255, 255, 255, a8]
                };
                let col_p_sub = if is_light {
                    [60, 75, 95, (200.0 * page_alpha) as u8]
                } else {
                    [170, 185, 215, (190.0 * page_alpha) as u8]
                };

                // Subtitle Header
                let sub_y = 52.0 + slide_y;
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "SYSTEM HEALTH",
                    px_c - 146.0,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );
                let up_str = format!(
                    "UPTIME {}h {}m",
                    self.sys_stats.uptime_hrs, self.sys_stats.uptime_mins
                );
                let up_w = self.font.measure_text(&up_str, 10.5, FontWeight::Bold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &up_str,
                    px_c + 146.0 - up_w,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );

                // Row 1: CPU Metric Tile
                let cpu_y = 90.0 + slide_y;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    cpu_y,
                    146.0,
                    22.0,
                    10.0,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (14.0 * page_alpha) as u8,
                    ],
                );
                UIFont::draw_cpu_icon(
                    buf,
                    self.width,
                    self.height,
                    px_c - 128.0,
                    cpu_y - 4.0,
                    col_p,
                );
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "CPU UTILIZATION",
                    px_c - 110.0,
                    cpu_y - 4.0,
                    11.5,
                    col_p,
                    col_shadow,
                    FontWeight::Bold,
                );
                let cpu_val = format!("{}%", self.sys_stats.cpu_pct);
                let cpu_w = self.font.measure_text(&cpu_val, 13.5, FontWeight::Bold);
                let cpu_col = if self.sys_stats.cpu_pct > 80 {
                    [255, 85, 85, a8]
                } else if is_light {
                    [16, 115, 220, a8]
                } else {
                    [86, 185, 255, a8]
                };
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &cpu_val,
                    px_c + 138.0 - cpu_w,
                    cpu_y - 4.0,
                    13.5,
                    cpu_col,
                    col_shadow,
                    FontWeight::Bold,
                );
                // CPU Bar Track & Fill
                let track_y = cpu_y + 11.0;
                let track_w = 260.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    track_y,
                    track_w * 0.5,
                    2.5,
                    2.5,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (25.0 * page_alpha) as u8,
                    ],
                );
                let cpu_fw = track_w * (self.sys_stats.cpu_pct as f32 / 100.0).clamp(0.02, 1.0);
                let cpu_fcx = (px_c - track_w * 0.5) + cpu_fw * 0.5;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    cpu_fcx,
                    track_y,
                    cpu_fw * 0.5,
                    2.5,
                    2.5,
                    cpu_col,
                );

                // Row 2: MEMORY Metric Tile
                let ram_y = 146.0 + slide_y;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    ram_y,
                    146.0,
                    22.0,
                    10.0,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (14.0 * page_alpha) as u8,
                    ],
                );
                UIFont::draw_ram_icon(
                    buf,
                    self.width,
                    self.height,
                    px_c - 128.0,
                    ram_y - 4.0,
                    col_p,
                );
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "MEMORY ALLOCATION",
                    px_c - 110.0,
                    ram_y - 4.0,
                    11.5,
                    col_p,
                    col_shadow,
                    FontWeight::Bold,
                );
                let ram_val = format!(
                    "{:.1} / {:.1} GB",
                    self.sys_stats.ram_used_gb, self.sys_stats.ram_total_gb
                );
                let ram_w = self.font.measure_text(&ram_val, 12.5, FontWeight::Bold);
                let ram_col = if is_light {
                    [120, 60, 225, a8]
                } else {
                    [178, 135, 255, a8]
                };
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &ram_val,
                    px_c + 138.0 - ram_w,
                    ram_y - 4.0,
                    12.5,
                    ram_col,
                    col_shadow,
                    FontWeight::Bold,
                );
                let ram_track_y = ram_y + 11.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    ram_track_y,
                    track_w * 0.5,
                    2.5,
                    2.5,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (25.0 * page_alpha) as u8,
                    ],
                );
                let ram_fw = track_w * (ram_pct as f32 / 100.0).clamp(0.02, 1.0);
                let ram_fcx = (px_c - track_w * 0.5) + ram_fw * 0.5;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    ram_fcx,
                    ram_track_y,
                    ram_fw * 0.5,
                    2.5,
                    2.5,
                    ram_col,
                );

                // Row 3: BATTERY Metric Tile
                let bat_y = 202.0 + slide_y;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    bat_y,
                    146.0,
                    22.0,
                    10.0,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (14.0 * page_alpha) as u8,
                    ],
                );
                UIFont::draw_battery_icon(
                    buf,
                    self.width,
                    self.height,
                    px_c - 128.0,
                    bat_y - 4.0,
                    self.sys_stats.bat_pct,
                    self.sys_stats.bat_charging,
                    col_p,
                );
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "BATTERY STATUS",
                    px_c - 110.0,
                    bat_y - 4.0,
                    11.5,
                    col_p,
                    col_shadow,
                    FontWeight::Bold,
                );
                let bat_val = format!(
                    "{}%{}",
                    self.sys_stats.bat_pct,
                    if self.sys_stats.bat_charging {
                        " [CHARGING]"
                    } else {
                        ""
                    }
                );
                let bat_w = self.font.measure_text(&bat_val, 12.5, FontWeight::Bold);
                let bat_col = if self.sys_stats.bat_charging {
                    if is_light {
                        [16, 140, 65, a8]
                    } else {
                        [78, 216, 138, a8]
                    }
                } else if self.sys_stats.bat_pct <= 20 {
                    [255, 140, 10, a8]
                } else if is_light {
                    [20, 28, 42, a8]
                } else {
                    [245, 248, 255, a8]
                };
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &bat_val,
                    px_c + 138.0 - bat_w,
                    bat_y - 4.0,
                    12.5,
                    bat_col,
                    col_shadow,
                    FontWeight::Bold,
                );
                let bat_track_y = bat_y + 11.0;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    px_c,
                    bat_track_y,
                    track_w * 0.5,
                    2.5,
                    2.5,
                    [
                        tile_base[0],
                        tile_base[1],
                        tile_base[2],
                        (25.0 * page_alpha) as u8,
                    ],
                );
                let bat_fw = track_w * (self.sys_stats.bat_pct as f32 / 100.0).clamp(0.02, 1.0);
                let bat_fcx = (px_c - track_w * 0.5) + bat_fw * 0.5;
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    bat_fcx,
                    bat_track_y,
                    bat_fw * 0.5,
                    2.5,
                    2.5,
                    bat_col,
                );
            }

            // ─────────────────────────────────────────────────────────
            // PAGE 1: APPLICATIONS (QUICK LAUNCHERS)
            // ─────────────────────────────────────────────────────────
            let off1 = (1.0 - cur_p) * CARD_W;
            if off1.abs() < CARD_W {
                let px_c = cx + off1;
                let page_alpha = (1.0 - (off1.abs() / CARD_W)).clamp(0.0, 1.0) * exp_alpha;
                let a8 = (255.0 * page_alpha) as u8;
                let col_p = if is_light {
                    [10, 14, 22, a8]
                } else {
                    [255, 255, 255, a8]
                };
                let col_p_sub = if is_light {
                    [60, 75, 95, (200.0 * page_alpha) as u8]
                } else {
                    [170, 185, 215, (190.0 * page_alpha) as u8]
                };

                let sub_y = 52.0 + slide_y;
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "QUICK LAUNCH",
                    px_c - 146.0,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "CLICK TO LAUNCH",
                    px_c + 54.0,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );

                let apps = [
                    ("Terminal", "foot command shell", 0),
                    ("Browser", "brave web browser", 1),
                    ("Editor", "visual studio code", 2),
                    ("Files", "thunar file manager", 3),
                ];

                for (idx, (title, sub, icon_idx)) in apps.iter().enumerate() {
                    let row_y = 78.0 + (idx as f32) * 44.0 + slide_y;
                    let is_hover = self.current_page == 1 && self.hovered_row == Some(idx);
                    let row_bg = if is_hover {
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (40.0 * page_alpha) as u8,
                        ]
                    } else {
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (14.0 * page_alpha) as u8,
                        ]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        px_c,
                        row_y,
                        146.0,
                        18.0,
                        10.0,
                        row_bg,
                    );
                    if is_hover {
                        UIFont::draw_rounded_rect_stroke(
                            buf,
                            self.width,
                            self.height,
                            px_c,
                            row_y,
                            146.0,
                            18.0,
                            10.0,
                            1.0,
                            [
                                tile_base[0],
                                tile_base[1],
                                tile_base[2],
                                (90.0 * page_alpha) as u8,
                            ],
                        );
                    }

                    let icon_x = px_c - 128.0;
                    match icon_idx {
                        0 => UIFont::draw_terminal_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        1 => UIFont::draw_browser_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        2 => UIFont::draw_editor_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        _ => UIFont::draw_folder_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                    }

                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        title,
                        px_c - 108.0,
                        row_y - 6.0,
                        12.5,
                        col_p,
                        col_shadow,
                        FontWeight::Bold,
                    );
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        sub,
                        px_c - 108.0,
                        row_y + 7.5,
                        9.5,
                        col_p_sub,
                        col_shadow,
                        FontWeight::Medium,
                    );

                    let hint_col = if is_hover {
                        if is_light {
                            [16, 115, 220, a8]
                        } else {
                            [86, 185, 255, a8]
                        }
                    } else {
                        col_p_sub
                    };
                    let hint_w = self.font.measure_text("LAUNCH ↵", 10.0, FontWeight::Bold);
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        "LAUNCH ↵",
                        px_c + 140.0 - hint_w,
                        row_y,
                        10.0,
                        hint_col,
                        col_shadow,
                        FontWeight::Bold,
                    );
                }
            }

            // ─────────────────────────────────────────────────────────
            // PAGE 2: POWER & SESSION
            // ─────────────────────────────────────────────────────────
            let off2 = (2.0 - cur_p) * CARD_W;
            if off2.abs() < CARD_W {
                let px_c = cx + off2;
                let page_alpha = (1.0 - (off2.abs() / CARD_W)).clamp(0.0, 1.0) * exp_alpha;
                let a8 = (255.0 * page_alpha) as u8;
                let col_p = if is_light {
                    [10, 14, 22, a8]
                } else {
                    [255, 255, 255, a8]
                };
                let col_p_sub = if is_light {
                    [60, 75, 95, (200.0 * page_alpha) as u8]
                } else {
                    [170, 185, 215, (190.0 * page_alpha) as u8]
                };

                let sub_y = 52.0 + slide_y;
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "SYSTEM CONTROLS",
                    px_c - 146.0,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "SESSION POWER",
                    px_c + 60.0,
                    sub_y,
                    10.5,
                    col_p_sub,
                    col_shadow,
                    FontWeight::Bold,
                );

                let power_ops = [
                    ("Lock Session", "secure display with hyprlock", "LOCK", 0),
                    ("Suspend", "low-power system sleep", "SLEEP", 1),
                    ("Reboot", "restart operating system", "RESTART", 2),
                    ("Power Off", "shut down system hardware", "SHUTDOWN", 3),
                ];

                for (idx, (title, sub, action, icon_idx)) in power_ops.iter().enumerate() {
                    let row_y = 78.0 + (idx as f32) * 44.0 + slide_y;
                    let is_hover = self.current_page == 2 && self.hovered_row == Some(idx);
                    let row_bg = if is_hover {
                        if idx == 3 {
                            [255, 60, 60, (50.0 * page_alpha) as u8]
                        } else {
                            [
                                tile_base[0],
                                tile_base[1],
                                tile_base[2],
                                (40.0 * page_alpha) as u8,
                            ]
                        }
                    } else {
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (14.0 * page_alpha) as u8,
                        ]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        px_c,
                        row_y,
                        146.0,
                        18.0,
                        10.0,
                        row_bg,
                    );
                    if is_hover {
                        let stroke_col = if idx == 3 {
                            [255, 90, 90, a8]
                        } else {
                            [
                                tile_base[0],
                                tile_base[1],
                                tile_base[2],
                                (90.0 * page_alpha) as u8,
                            ]
                        };
                        UIFont::draw_rounded_rect_stroke(
                            buf,
                            self.width,
                            self.height,
                            px_c,
                            row_y,
                            146.0,
                            18.0,
                            10.0,
                            1.0,
                            stroke_col,
                        );
                    }

                    let icon_x = px_c - 128.0;
                    match icon_idx {
                        0 => UIFont::draw_lock_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        1 => UIFont::draw_moon_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        2 => UIFont::draw_restart_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            col_p,
                        ),
                        _ => UIFont::draw_power_icon(
                            buf,
                            self.width,
                            self.height,
                            icon_x,
                            row_y,
                            if is_hover { [255, 90, 90, a8] } else { col_p },
                        ),
                    }

                    let title_col = if is_hover && idx == 3 {
                        [255, 140, 140, a8]
                    } else {
                        col_p
                    };
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        title,
                        px_c - 108.0,
                        row_y - 6.0,
                        12.5,
                        title_col,
                        col_shadow,
                        FontWeight::Bold,
                    );
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        sub,
                        px_c - 108.0,
                        row_y + 7.5,
                        9.5,
                        col_p_sub,
                        col_shadow,
                        FontWeight::Medium,
                    );

                    let act_col = if is_hover && idx == 3 {
                        [255, 90, 90, a8]
                    } else if is_hover {
                        if is_light {
                            [16, 115, 220, a8]
                        } else {
                            [86, 185, 255, a8]
                        }
                    } else {
                        col_p_sub
                    };
                    let act_w = self.font.measure_text(action, 10.0, FontWeight::Bold);
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        action,
                        px_c + 140.0 - act_w,
                        row_y,
                        10.0,
                        act_col,
                        col_shadow,
                        FontWeight::Bold,
                    );
                }
            }

            // ── PAGINATION DOTS (BOTTOM CAROUSEL INDICATOR) ──
            let dot_y = 265.0 + slide_y;
            for i in 0..3 {
                let dx = cx - 24.0 + (i as f32) * 24.0;
                let is_cur = self.current_page == i;
                if is_cur {
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        dot_y,
                        10.0,
                        2.5,
                        2.5,
                        col_primary,
                    );
                } else {
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        dx,
                        dot_y,
                        2.5,
                        2.5,
                        2.5,
                        [
                            tile_base[0],
                            tile_base[1],
                            tile_base[2],
                            (65.0 * exp_alpha) as u8,
                        ],
                    );
                }
            }
        }
    }

    pub fn draw(&mut self) -> bool {
        if !self.enabled {
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

        let res = if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &overlay_vec);

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);

            let cx = (self.width as f32) * 0.5;
            let card_left = cx - self.current_w * 0.5;
            let x_norm = card_left / (self.width as f32);
            let y_norm = 0.0; // Flush with top screen edge
            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);

            let pill_radius = if self.is_compact {
                15.0
            } else {
                Tokens::RADIUS_OUTER
            };

            let control_hub_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle {
                    radius: pill_radius,
                })
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(self.fillet_radius)
                .bevel(16.0)
                .refraction(16.0)
                .dispersion(0.12)
                .blur(self.blur_radius)
                .specular(1.10)
                .fresnel(0.60)
                .style_mode(self.style_mode)
                .tint(0.0, 0.0, 1.0, is_light_f32)
                .roughness(self.sys_stats.bat_pct as f32)
                .element_type(0.0)
                .deformation(0.0)
                .pointer_pos(px, py);

            let surfaces = vec![control_hub_card];
            let _ = renderer.render(&surfaces);
            true
        } else {
            false
        };
        self.overlay_buffer = overlay_vec;
        res
    }
}
