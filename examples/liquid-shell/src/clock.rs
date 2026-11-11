use chrono::{Datelike, Local, Timelike};
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
use std::sync::Arc;
use std::time::Instant;
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

pub const CARD_PAD: f32 = 24.0;
pub const CARD_W: f32 = 320.0;
pub const CARD_H: f32 = 224.0;
pub const COMPACT_W: f32 = 156.0;
pub const COMPACT_H: f32 = 36.0;

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

    pub battery_level: f32,
    pub last_battery_check: Instant,
    pub last_minute: u32,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    pub anim_speed: f32,

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
}

impl ClockWidget {
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
        let width = (CARD_W + CARD_PAD * 2.0) as u32; // 368
        let height = (CARD_H + CARD_PAD * 2.0) as u32; // 272
        let pos_x = 24i32;
        let pos_y = 0i32; // Flush against top screen edge

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Top,
            Some("liquid-glass-clock"),
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
            stiffness: 220.0,
            damping: 24.0,
            mass: 1.0,
            initial_velocity: 0.0,
            epsilon: 0.001,
        };

        Self {
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

            battery_level: 100.0,
            last_battery_check: Instant::now() - std::time::Duration::from_secs(60),
            last_minute: 99,
            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 6.0,
            anim_speed: 1.0,

            is_docked: true,
            is_compact: true,
            hover_to_expand: true,
            current_w: COMPACT_W,
            target_w: COMPACT_W,
            current_h: COMPACT_H,
            target_h: COMPACT_H,
            size_spring: Spring::with_params(0.0, spring_params),
            fillet_radius: 18.0,
            last_hover_time: Instant::now() - std::time::Duration::from_secs(60),
            needs_input_region_update: true,
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn init_gpu(&mut self, gpu: &SharedGpu) {
        let mut renderer = gpu
            .create_renderer(self.window_wrapper.clone(), self.width, self.height)
            .expect("Failed to create GlassRenderer for clock");

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
        }
    }

    pub fn collapse(&mut self) {
        if !self.is_compact {
            self.is_compact = true;
            self.size_spring.set_target(0.0);
            self.needs_input_region_update = true;
        }
    }

    #[allow(dead_code)]
    pub fn toggle_expand(&mut self) {
        if self.is_compact {
            self.expand();
        } else {
            self.collapse();
        }
    }

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        let flare = if self.is_docked && py <= self.fillet_radius {
            self.fillet_radius
        } else {
            0.0
        };
        let cx = (self.width as f32) * 0.5;
        let half_w = self.current_w * 0.5 + flare;
        px >= (cx - half_w) && px <= (cx + half_w) && py >= 0.0 && py <= (self.current_h + 6.0)
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        self.needs_input_region_update = false;
        let region = compositor.wl_compositor().create_region(qh, ());
        let flare = if self.is_docked {
            self.fillet_radius as i32
        } else {
            0
        };
        let cx = self.width as i32 / 2;
        if self.is_compact && self.size_spring.value < 0.1 {
            let pill_w = COMPACT_W as i32 + flare * 2;
            let pill_h = (COMPACT_H + 6.0) as i32;
            let pill_x = cx - pill_w / 2;
            region.add(pill_x, 0, pill_w, pill_h);
        } else {
            let card_w = CARD_W as i32 + flare * 2;
            let card_h = (CARD_H + 10.0) as i32;
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
                self.last_hover_time = Instant::now();
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
                } else if py <= COMPACT_H {
                    self.collapse();
                }
                true
            }
            _ => false,
        }
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        let eff_dt = dt * self.anim_speed;
        self.size_spring.update(eff_dt);

        let t = self.size_spring.value.clamp(0.0, 1.0);
        self.target_w = CARD_W;
        self.target_h = CARD_H;
        self.current_w = COMPACT_W + (self.target_w - COMPACT_W) * t;
        self.current_h = COMPACT_H + (self.target_h - COMPACT_H) * t;

        // Auto-collapse after inactivity
        if !self.is_compact
            && !self.is_hovered
            && self.last_hover_time.elapsed().as_secs_f32() > 2.5
        {
            self.collapse();
        }

        !self.size_spring.is_at_rest()
    }

    pub fn update_battery(&mut self) {
        if self.last_battery_check.elapsed().as_secs() < 30 {
            return;
        }
        self.last_battery_check = Instant::now();
        if let Ok(cap_str) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
            if let Ok(cap) = cap_str.trim().parse::<f32>() {
                self.battery_level = cap.clamp(0.0, 100.0);
            }
        }
    }

    pub fn render_ui_overlay(&mut self, buf: &mut [u8]) {
        let is_light = self.effective_is_light();

        let (date_col, _date_sec) = if is_light {
            ([8, 12, 20, 255], [18, 24, 34, 255])
        } else {
            ([255, 255, 255, 255], [235, 240, 252, 255])
        };

        let (time_col, ampm_col) = if is_light {
            ([8, 12, 20, 255], [18, 24, 34, 255])
        } else {
            ([255, 255, 255, 255], [235, 240, 252, 255])
        };

        let bat_col = if is_light {
            [8, 12, 20, 255]
        } else {
            [255, 255, 255, 255]
        };

        let col_shadow = [0, 0, 0, 0];

        let now = Local::now();
        let hr = now.hour();
        let mn = now.minute();
        let hr12 = if hr == 0 {
            12
        } else if hr > 12 {
            hr - 12
        } else {
            hr
        };
        let ampm = if hr >= 12 { "PM" } else { "AM" };
        let time_str = format!("{:02}:{:02}", hr12, mn);

        let wday_full = match now.weekday().num_days_from_sunday() {
            1 => "MONDAY",
            2 => "TUESDAY",
            3 => "WEDNESDAY",
            4 => "THURSDAY",
            5 => "FRIDAY",
            6 => "SATURDAY",
            _ => "SUNDAY",
        };

        let wday_short = match now.weekday().num_days_from_sunday() {
            1 => "MON",
            2 => "TUE",
            3 => "WED",
            4 => "THU",
            5 => "FRI",
            6 => "SAT",
            _ => "SUN",
        };

        let month_full = match now.month() {
            1 => "JANUARY",
            2 => "FEBRUARY",
            3 => "MARCH",
            4 => "APRIL",
            5 => "MAY",
            6 => "JUNE",
            7 => "JULY",
            8 => "AUGUST",
            9 => "SEPTEMBER",
            10 => "OCTOBER",
            11 => "NOVEMBER",
            _ => "DECEMBER",
        };

        let date_full = format!("{} {:02}, {}", month_full, now.day(), now.year());
        let bat_str = format!("{:.0}%", self.battery_level);

        let cx = (self.width as f32) * 0.5;
        let t = self.size_spring.value.clamp(0.0, 1.0);

        // ══════════════════════════════════════════════════════════════
        // 1. COMPACT OVERVIEW PILL (t < 0.35)
        // ══════════════════════════════════════════════════════════════
        if t < 0.35 {
            let comp_alpha = ((0.35 - t) / 0.35).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * comp_alpha) as u8;
            let ink_col = [time_col[0], time_col[1], time_col[2], alpha_u8];

            // Clock icon
            UIFont::draw_clock_icon(buf, self.width, self.height, cx - 44.0, 18.0, ink_col);

            // Time text: e.g. "WED 04:21"
            let compact_str = format!("{} {}", wday_short, time_str);
            let time_w = self.font.measure_text(&compact_str, 13.0, FontWeight::Bold);
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &compact_str,
                cx - 30.0,
                18.0,
                13.0,
                ink_col,
                col_shadow,
                FontWeight::Bold,
            );

            // Small battery indicator
            UIFont::draw_battery_capsule(
                buf,
                self.width,
                self.height,
                cx - 30.0 + time_w + 14.0,
                18.0,
                self.battery_level,
                ink_col,
            );
        }

        // ══════════════════════════════════════════════════════════════
        // 2. EXPANDED LIQUID FLOATING/EMERGING VIEW (t > 0.05)
        // ══════════════════════════════════════════════════════════════
        if t > 0.05 {
            let exp_alpha = ((t - 0.1) / 0.9).clamp(0.0, 1.0);
            let alpha_u8 = (255.0 * exp_alpha) as u8;
            let slide_y = (1.0 - t) * -22.0;

            let col_day = [date_col[0], date_col[1], date_col[2], alpha_u8];
            let col_time = [time_col[0], time_col[1], time_col[2], alpha_u8];
            let col_ampm = [
                ampm_col[0],
                ampm_col[1],
                ampm_col[2],
                (180.0 * exp_alpha) as u8,
            ];
            let col_bat = [bat_col[0], bat_col[1], bat_col[2], alpha_u8];

            // ── A. FUTURISTIC WIDE-TRACKED DAY (e.g. W E D N E S D A Y) ──
            // Reference Image 1: wide tracked uppercase lettering with generous character spacing
            let tracking = 7.0f32;
            let mut total_day_w = 0.0f32;
            let chars: Vec<char> = wday_full.chars().collect();
            for (i, &ch) in chars.iter().enumerate() {
                let s = ch.to_string();
                total_day_w += self.font.measure_text(&s, 16.0, FontWeight::Bold);
                if i < chars.len() - 1 {
                    total_day_w += tracking;
                }
            }

            let mut cur_x = cx - total_day_w * 0.5;
            let day_cy = 38.0 + slide_y;
            for (i, &ch) in chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 16.0, FontWeight::Bold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_x,
                    day_cy,
                    16.0,
                    col_day,
                    col_shadow,
                    FontWeight::Bold,
                );
                cur_x += ch_w;
                if i < chars.len() - 1 {
                    cur_x += tracking;
                }
            }

            // ── B. ELEGANT TRACKED DATE (e.g. NOVEMBER 11, 2026) ──
            let date_tracking = 2.5f32;
            let mut total_date_w = 0.0f32;
            let date_chars: Vec<char> = date_full.chars().collect();
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                total_date_w += self.font.measure_text(&s, 11.5, FontWeight::Medium);
                if i < date_chars.len() - 1 {
                    total_date_w += date_tracking;
                }
            }
            let mut cur_date_x = cx - total_date_w * 0.5;
            let date_cy = 60.0 + slide_y;
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 11.5, FontWeight::Medium);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_date_x,
                    date_cy,
                    11.5,
                    col_day,
                    col_shadow,
                    FontWeight::Medium,
                );
                cur_date_x += ch_w;
                if i < date_chars.len() - 1 {
                    cur_date_x += date_tracking;
                }
            }

            // ── C. HERO DIGITAL TIME (58px) + AM/PM BADGE ──
            let time_cy = 118.0 + slide_y;
            let time_baseline = self
                .font
                .vertical_center_baseline(time_cy, 58.0, FontWeight::Bold);
            let time_w = self.font.measure_text(&time_str, 58.0, FontWeight::Bold);
            let ampm_w = self.font.measure_text(ampm, 14.0, FontWeight::Bold);
            let total_time_block_w = time_w + 10.0 + ampm_w;
            let time_start_x = cx - total_time_block_w * 0.5;

            let end_time_x = self.font.draw_text_with_shadow(
                buf,
                self.width,
                self.height,
                &time_str,
                time_start_x,
                time_baseline,
                58.0,
                col_time,
                col_shadow,
                FontWeight::Bold,
            );

            // AM/PM badge
            self.font.draw_text_with_shadow(
                buf,
                self.width,
                self.height,
                ampm,
                end_time_x + 8.0,
                time_baseline - 6.0,
                14.0,
                col_ampm,
                col_shadow,
                FontWeight::Bold,
            );

            // ── D. LIQUID DEPTH REFLECTION (Reference Image 2) ──
            // Mirror the rasterized digits downwards below the horizon line with liquid optical falloff
            let baseline_y = time_baseline as usize;
            let reflect_h = 18usize;
            for dy in 1..=reflect_h {
                let src_y = baseline_y.saturating_sub(dy);
                let dst_y = baseline_y + dy;
                if dst_y >= self.height as usize {
                    break;
                }
                let falloff = (1.0 - (dy as f32 / reflect_h as f32)).powf(1.4) * 0.28 * exp_alpha;
                for x in 0..self.width as usize {
                    let src_idx = (src_y * self.width as usize + x) * 4;
                    let dst_idx = (dst_y * self.width as usize + x) * 4;
                    let src_a = buf[src_idx + 3] as f32 / 255.0;
                    if src_a > 0.01 {
                        let refl_a = (src_a * falloff * 255.0) as u8;
                        if refl_a > buf[dst_idx + 3] {
                            buf[dst_idx] = buf[src_idx];
                            buf[dst_idx + 1] = buf[src_idx + 1];
                            buf[dst_idx + 2] = buf[src_idx + 2];
                            buf[dst_idx + 3] = refl_a;
                        }
                    }
                }
            }

            // ── E. BATTERY CAPSULE & PERCENTAGE ──
            let bat_cy = 180.0 + slide_y;
            let bat_text_w = self.font.measure_text(&bat_str, 14.0, FontWeight::Bold);
            let bat_block_w = bat_text_w + 12.0 + 26.0;
            let bat_start_x = cx - bat_block_w * 0.5;

            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &bat_str,
                bat_start_x,
                bat_cy,
                14.0,
                col_bat,
                col_shadow,
                FontWeight::Bold,
            );

            UIFont::draw_battery_capsule(
                buf,
                self.width,
                self.height,
                bat_start_x + bat_text_w + 14.0,
                bat_cy,
                self.battery_level,
                col_bat,
            );
        }
    }

    pub fn draw(&mut self) -> bool {
        self.update_battery();

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

            let now = Local::now();
            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);

            let cx = (self.width as f32) * 0.5;
            let card_left = cx - self.current_w * 0.5;
            let x_norm = card_left / (self.width as f32);
            let y_norm = 0.0; // Flush with top screen edge
            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);

            let clock_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle {
                    radius: Tokens::RADIUS_OUTER,
                })
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(self.fillet_radius)
                .bevel(22.0)
                .refraction(20.0)
                .dispersion(0.14)
                .blur(self.blur_radius)
                .specular(1.10)
                .fresnel(0.60)
                .style_mode(self.style_mode)
                .tint(0.0, 0.0, 1.0, is_light_f32)
                .roughness(self.battery_level)
                .element_type(25.0)
                .deformation(if self.is_docked { 2.0 } else { 0.0 })
                .pointer_pos(px, py);

            let surfaces = vec![clock_card];
            let _ = renderer.render(&surfaces);
            self.last_minute = now.minute();
            true
        } else {
            false
        };
        self.overlay_buffer = overlay_vec;
        res
    }
}
