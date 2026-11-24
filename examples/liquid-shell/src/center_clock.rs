use chrono::{Local, Timelike};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu, Tokens};
use glass_platform::wallpaper::{OpticalZoneProfile, WallpaperManager, ZoneMetrics};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::compositor::{CompositorState, SurfaceData};
use smithay_client_toolkit::seat::pointer::PointerEvent;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
    LayerSurfaceData,
};
use smithay_client_toolkit::shell::WaylandSurface;
use std::sync::Arc;
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

#[path = "config.rs"]
pub mod config;
#[allow(unused_imports)]
pub use config::{CenterHeroConfig, ContrastPreset};

pub const CENTER_HERO_W: u32 = 1100;
pub const CENTER_HERO_H: u32 = 420;

#[allow(dead_code)]
pub const COL_DAY_BASE_LIGHT: [u8; 4] = [18, 22, 32, 248];
#[allow(dead_code)]
pub const COL_DAY_BASE_DARK: [u8; 4] = [248, 250, 255, 248];

#[allow(dead_code)]
pub const COL_DATE_BASE_LIGHT: [u8; 4] = [38, 48, 64, 235];
#[allow(dead_code)]
pub const COL_DATE_BASE_DARK: [u8; 4] = [238, 245, 255, 235];

#[allow(dead_code)]
pub const COL_AMPM_BASE_LIGHT: [u8; 4] = [32, 40, 52, 235];
#[allow(dead_code)]
pub const COL_AMPM_BASE_DARK: [u8; 4] = [235, 242, 255, 235];

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

pub struct CenterHeroClock {
    #[allow(dead_code)]
    pub layer_surface: Option<LayerSurface>,
    pub surface: wl_surface::WlSurface,
    pub window_wrapper: Arc<WaylandWindow>,
    pub renderer: Option<GlassRenderer>,

    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,

    pub font: UIFont,
    pub overlay_buffer: Vec<u8>,
    pub wallpaper_crop: Option<Vec<u8>>,
    pub is_wallpaper_light: bool,

    pub enabled: bool,
    pub show_glass_card: bool,
    pub show_time: bool,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    #[allow(dead_code)]
    pub last_minute: u32,

    // Optical Contrast & Wallpaper Adaptation (Milestone 1)
    pub zone_profile: Option<OpticalZoneProfile>,
    pub contrast_preset: String,
    pub rim_boost: f32,
    pub inner_glow: f32,
    pub luminance_threshold: f32,
    pub adaptive_halo: bool,
    pub placement: String,
    pub vertical_offset: i32,
    #[allow(dead_code)]
    pub screen_width: u32,
    #[allow(dead_code)]
    pub screen_height: u32,
    pub needs_redraw: bool,
}

impl CenterHeroClock {
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
        let width = CENTER_HERO_W;
        let height = CENTER_HERO_H;

        // Dynamic horizontal and vertical optical centering
        let screen_width = 1920u32;
        let screen_height = 1080u32;
        let pos_x = (screen_width as i32 - width as i32) / 2;
        let pos_y = ((screen_height as i32 - height as i32) / 2).max(0);

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Bottom, // Desktop wallpaper level below application windows
            Some("liquid-center-hero"),
            None,
        );

        // Center on screen
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

        let mut widget = Self {
            layer_surface: Some(layer_surface),
            surface,
            window_wrapper,
            renderer: None,

            width,
            height,
            pos_x,
            pos_y,

            font: UIFont::new(),
            overlay_buffer: Vec::new(),
            wallpaper_crop: None,
            is_wallpaper_light: false,

            enabled: true,
            show_glass_card: false,
            show_time: true,
            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 20.0,
            last_minute: 99,

            zone_profile: None,
            contrast_preset: "auto".to_string(),
            rim_boost: 1.0,
            inner_glow: 0.0,
            luminance_threshold: 0.35,
            adaptive_halo: true,
            placement: "center".to_string(),
            vertical_offset: 0,
            screen_width,
            screen_height,
            needs_redraw: true,
        };

        widget.load_config();
        widget
    }

    #[allow(dead_code)]
    pub fn set_screen_geometry(&mut self, screen_w: u32, screen_h: u32) {
        self.screen_width = screen_w;
        self.screen_height = screen_h;
        self.pos_x = (screen_w as i32 - self.width as i32) / 2;
        self.pos_y = (((screen_h as i32 - self.height as i32) / 2) + self.vertical_offset).max(0);
        if let Some(ls) = &self.layer_surface {
            ls.set_margin(self.pos_y, 0, 0, 0);
            ls.commit();
        }
    }

    pub fn load_config(&mut self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf_path = format!("{}/.config/liquid-glass/config.json", home);
        if let Ok(content) = std::fs::read_to_string(&conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(c) = val.get("center_hero").or_else(|| val.get("center_clock")) {
                    if let Some(e) = c.get("enabled").and_then(|v| v.as_bool()) {
                        self.enabled = e;
                    }
                    if let Some(g) = c
                        .get("show_glass_card")
                        .or_else(|| c.get("glass_card"))
                        .and_then(|v| v.as_bool())
                    {
                        self.show_glass_card = g;
                    }
                    if let Some(t) = c.get("show_time").and_then(|v| v.as_bool()) {
                        self.show_time = t;
                    }
                    if let Some(s) = c.get("style_mode").and_then(|v| v.as_f64()) {
                        self.style_mode = s as f32;
                    }
                    if let Some(b) = c.get("blur_radius").and_then(|v| v.as_f64()) {
                        self.blur_radius = b as f32;
                    }
                    if let Some(p) = c.get("contrast_preset").and_then(|v| v.as_str()) {
                        self.contrast_preset = p.to_string();
                    }
                    if let Some(r) = c.get("rim_boost").and_then(|v| v.as_f64()) {
                        self.rim_boost = r as f32;
                    }
                    if let Some(g) = c.get("inner_glow").and_then(|v| v.as_f64()) {
                        self.inner_glow = g as f32;
                    }
                    if let Some(th) = c.get("luminance_threshold").and_then(|v| v.as_f64()) {
                        self.luminance_threshold = th as f32;
                    }
                    if let Some(h) = c.get("adaptive_halo").and_then(|v| v.as_bool()) {
                        self.adaptive_halo = h;
                    }
                    if let Some(pl) = c.get("placement").and_then(|v| v.as_str()) {
                        self.placement = pl.to_string();
                    }
                    if let Some(vo) = c.get("vertical_offset").and_then(|v| v.as_i64()) {
                        self.vertical_offset = vo as i32;
                    }
                    if let Some(px) = c.get("pos_x").and_then(|v| v.as_i64()) {
                        self.pos_x = px as i32;
                    }
                    if let Some(py) = c.get("pos_y").and_then(|v| v.as_i64()) {
                        self.pos_y = py as i32;
                    }
                }
            }
        }

        // Also check config.toml if present using lightweight line parsing
        let toml_path = format!("{}/.config/liquid-glass/config.toml", home);
        if let Ok(toml_content) = std::fs::read_to_string(&toml_path) {
            for line in toml_content.lines() {
                let trimmed = line.trim();
                if let Some((k, v)) = trimmed.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    match k {
                        "contrast_preset" => self.contrast_preset = v.to_string(),
                        "rim_boost" => {
                            if let Ok(val) = v.parse::<f32>() {
                                self.rim_boost = val;
                            }
                        }
                        "inner_glow" => {
                            if let Ok(val) = v.parse::<f32>() {
                                self.inner_glow = val;
                            }
                        }
                        "luminance_threshold" => {
                            if let Ok(val) = v.parse::<f32>() {
                                self.luminance_threshold = val;
                            }
                        }
                        "adaptive_halo" => {
                            if let Ok(val) = v.parse::<bool>() {
                                self.adaptive_halo = val;
                            }
                        }
                        "placement" => self.placement = v.to_string(),
                        "vertical_offset" => {
                            if let Ok(val) = v.parse::<i32>() {
                                self.vertical_offset = val;
                            }
                        }
                        "enabled" => {
                            if let Ok(val) = v.parse::<bool>() {
                                self.enabled = val;
                            }
                        }
                        "show_time" => {
                            if let Ok(val) = v.parse::<bool>() {
                                self.show_time = val;
                            }
                        }
                        "show_glass_card" => {
                            if let Ok(val) = v.parse::<bool>() {
                                self.show_glass_card = val;
                            }
                        }
                        _ => {}
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
            .expect("Failed to create GlassRenderer for center hero clock");

        if let Some(crop) = &self.wallpaper_crop {
            renderer.update_background(self.width, self.height, crop);
        }
        self.renderer = Some(renderer);
    }

    pub fn update_wallpaper(&mut self, crop: Vec<u8>) {
        let profile = WallpaperManager::sample_widget_zones(&crop, self.width, self.height);
        self.is_wallpaper_light =
            WallpaperManager::is_buffer_light_hysteresis(&crop, self.is_wallpaper_light);
        self.zone_profile = Some(profile);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
        self.needs_redraw = true;
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        // 100% transparent click pass-through to wallpaper/desktop icons
        let region = compositor.wl_compositor().create_region(qh, ());
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.commit();
    }

    pub fn on_pointer_event(&mut self, _event: &PointerEvent) -> bool {
        // Clicks pass through
        false
    }

    pub fn zone_text_styling(
        &self,
        zone: Option<&ZoneMetrics>,
        base_light: [u8; 4],
        base_dark: [u8; 4],
    ) -> ([u8; 4], [u8; 4], Option<[u8; 4]>) {
        let is_multi_tone_overall = self.zone_profile.map(|p| p.is_multi_tone).unwrap_or(false);
        compute_zone_text_styling(
            self.theme_mode,
            self.luminance_threshold,
            self.is_wallpaper_light,
            self.adaptive_halo,
            zone,
            is_multi_tone_overall,
            base_light,
            base_dark,
        )
    }

    fn render_ui_overlay(&mut self) {
        let buf_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != buf_len {
            self.overlay_buffer.resize(buf_len, 0);
        }
        self.overlay_buffer.fill(0);

        if !self.enabled {
            return;
        }

        let (col_day, col_day_shadow, col_day_halo) = self.zone_text_styling(
            self.zone_profile.as_ref().map(|p| &p.day_zone),
            COL_DAY_BASE_LIGHT,
            COL_DAY_BASE_DARK,
        );
        let (col_date, col_date_shadow, col_date_halo) = self.zone_text_styling(
            self.zone_profile.as_ref().map(|p| &p.date_zone),
            COL_DATE_BASE_LIGHT,
            COL_DATE_BASE_DARK,
        );
        let (col_ampm, col_ampm_shadow, col_ampm_halo) = self.zone_text_styling(
            self.zone_profile.as_ref().map(|p| &p.numeral_zone),
            COL_AMPM_BASE_LIGHT,
            COL_AMPM_BASE_DARK,
        );

        let now = Local::now();
        let wday_full = now.format("%A").to_string().to_uppercase();
        let date_full = now.format("%B %d, %Y").to_string().to_uppercase();
        let (hour_12, ampm) = match now.hour12() {
            (true, h) => (h, "PM"),
            (false, h) => (h, "AM"),
        };

        let cx = (self.width as f32) * 0.5;
        let cy = (self.height as f32) * 0.5;
        let buf = &mut self.overlay_buffer;

        let draw_glyph = |buf: &mut [u8],
                          font: &UIFont,
                          w: u32,
                          h: u32,
                          s: &str,
                          x: f32,
                          cy: f32,
                          size: f32,
                          col: [u8; 4],
                          shadow: [u8; 4],
                          halo: Option<[u8; 4]>,
                          weight: FontWeight| {
            if let Some(h_col) = halo {
                font.draw_text_centered_y(
                    buf,
                    w,
                    h,
                    s,
                    x,
                    cy - 1.0,
                    size,
                    h_col,
                    [0, 0, 0, 0],
                    weight,
                );
            }
            font.draw_text_centered_y(buf, w, h, s, x, cy, size, col, shadow, weight);
        };

        // ── 1. Apple-Grade Monumental Day & Date ──
        if !self.show_time {
            // Pure Hero Day ("W E D N E S D A Y")
            let day_tracking = 11.0f32;
            let day_chars: Vec<char> = wday_full.chars().collect();
            let mut total_day_w = 0.0f32;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                total_day_w += self.font.measure_text(&s, 62.0, FontWeight::Bold);
                if i < day_chars.len() - 1 {
                    total_day_w += day_tracking;
                }
            }
            let mut cur_day_x = cx - total_day_w * 0.5;
            let day_cy = cy - 32.0;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 62.0, FontWeight::Bold);
                draw_glyph(
                    buf,
                    &self.font,
                    self.width,
                    self.height,
                    &s,
                    cur_day_x,
                    day_cy,
                    62.0,
                    col_day,
                    col_day_shadow,
                    col_day_halo,
                    FontWeight::Bold,
                );
                cur_day_x += ch_w;
                if i < day_chars.len() - 1 {
                    cur_day_x += day_tracking;
                }
            }

            // Elegant Tracked Date ("NOVEMBER 11, 2026")
            let date_tracking = 5.5f32;
            let mut total_date_w = 0.0f32;
            let date_chars: Vec<char> = date_full.chars().collect();
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                total_date_w += self.font.measure_text(&s, 22.0, FontWeight::Medium);
                if i < date_chars.len() - 1 {
                    total_date_w += date_tracking;
                }
            }
            let mut cur_date_x = cx - total_date_w * 0.5;
            let date_cy = cy + 46.0;
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 22.0, FontWeight::Medium);
                draw_glyph(
                    buf,
                    &self.font,
                    self.width,
                    self.height,
                    &s,
                    cur_date_x,
                    date_cy,
                    22.0,
                    col_date,
                    col_date_shadow,
                    col_date_halo,
                    FontWeight::Medium,
                );
                cur_date_x += ch_w;
                if i < date_chars.len() - 1 {
                    cur_date_x += date_tracking;
                }
            }
        } else {
            // Day & Date with Liquid Glass Time Numerals underneath
            let day_tracking = 8.0f32;
            let day_chars: Vec<char> = wday_full.chars().collect();
            let mut total_day_w = 0.0f32;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                total_day_w += self.font.measure_text(&s, 28.0, FontWeight::Bold);
                if i < day_chars.len() - 1 {
                    total_day_w += day_tracking;
                }
            }
            let mut cur_day_x = cx - total_day_w * 0.5;
            let day_cy = cy - 76.0;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 28.0, FontWeight::Bold);
                draw_glyph(
                    buf,
                    &self.font,
                    self.width,
                    self.height,
                    &s,
                    cur_day_x,
                    day_cy,
                    28.0,
                    col_day,
                    col_day_shadow,
                    col_day_halo,
                    FontWeight::Bold,
                );
                cur_day_x += ch_w;
                if i < day_chars.len() - 1 {
                    cur_day_x += day_tracking;
                }
            }

            let date_tracking = 4.5f32;
            let mut total_date_w = 0.0f32;
            let date_chars: Vec<char> = date_full.chars().collect();
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                total_date_w += self.font.measure_text(&s, 16.0, FontWeight::Medium);
                if i < date_chars.len() - 1 {
                    total_date_w += date_tracking;
                }
            }
            let mut cur_date_x = cx - total_date_w * 0.5;
            let date_cy = cy - 40.0;
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 16.0, FontWeight::Medium);
                draw_glyph(
                    buf,
                    &self.font,
                    self.width,
                    self.height,
                    &s,
                    cur_date_x,
                    date_cy,
                    16.0,
                    col_date,
                    col_date_shadow,
                    col_date_halo,
                    FontWeight::Medium,
                );
                cur_date_x += ch_w;
                if i < date_chars.len() - 1 {
                    cur_date_x += date_tracking;
                }
            }

            // AM/PM Crystalline Badge snuggly aligned with proportional optical digits
            let get_dig_bounds = |d: u32| -> (f32, f32) {
                if d == 1 {
                    (-1.3, 1.3)
                } else {
                    (-2.4, 2.4)
                }
            };
            let scale = 12.0f32;
            let cyl_r = 11.0f32;
            let gap = 10.0f32;
            let gap_col = 14.0f32;
            let col_r = 8.0f32;

            let h_tens = hour_12 / 10;
            let h_ones = hour_12 % 10;
            let m_tens = now.minute() / 10;
            let m_ones = now.minute() % 10;

            let b_m0 = get_dig_bounds(m_tens);
            let m0_x = col_r + gap_col + cyl_r - (b_m0.0 * scale);
            let m0_right = m0_x + b_m0.1 * scale + cyl_r;

            let b_m1 = get_dig_bounds(m_ones);
            let m1_x = m0_right + gap + cyl_r - (b_m1.0 * scale);
            let m1_right = m1_x + b_m1.1 * scale + cyl_r;

            let b_h1 = get_dig_bounds(h_ones);
            let h1_x = -col_r - gap_col - cyl_r - (b_h1.1 * scale);
            let h1_left = h1_x + b_h1.0 * scale - cyl_r;

            let b_h0 = get_dig_bounds(h_tens);
            let h0_x = h1_left - gap - cyl_r - (b_h0.1 * scale);
            let h0_left = h0_x + b_h0.0 * scale - cyl_r;

            let h_left = if h_tens > 0 { h0_left } else { h1_left };
            let x_shift = (m1_right + h_left) * 0.5;
            let shifted_right = m1_right - x_shift;

            let ampm_x = cx + shifted_right + 20.0;
            let ampm_cy = cy + 50.0;
            draw_glyph(
                buf,
                &self.font,
                self.width,
                self.height,
                ampm,
                ampm_x,
                ampm_cy,
                14.0,
                col_ampm,
                col_ampm_shadow,
                col_ampm_halo,
                FontWeight::Bold,
            );
        }
    }

    pub fn draw(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        self.render_ui_overlay();

        let is_light_f32 = if self.effective_is_light() { 1.0 } else { 0.0 };
        let contrast_mode_f32 = match self.contrast_preset.to_ascii_lowercase().as_str() {
            "high_contrast" | "highcontrast" | "high" => 1.0,
            "subtle" | "soft" => 2.0,
            "crystalline" | "crystal" => 3.0,
            _ => 0.0,
        };

        if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &self.overlay_buffer);

            let frameless_overlay = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 0.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .contrast_mode(contrast_mode_f32)
                .rim_boost(self.rim_boost)
                .inner_glow(self.inner_glow)
                .luminance_threshold(self.luminance_threshold)
                .element_type(42.0); // Pure Frameless Overlay Pass!

            if self.show_time {
                let now = Local::now();
                let (hour_12, _ampm) = match now.hour12() {
                    (true, h) => (h, "PM"),
                    (false, h) => (h, "AM"),
                };

                let liquid_text = GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::RoundedRectangle { radius: 0.0 })
                    .bounds(0.0, 0.0, 1.0, 1.0)
                    .curvature(18.0)
                    .bevel(14.0)
                    .refraction(22.0)
                    .dispersion(0.14)
                    .blur(if is_light_f32 > 0.5 { 38.0 } else { 28.0 })
                    .specular(1.30)
                    .fresnel(0.65)
                    .style_mode(2.0)
                    .contrast_mode(contrast_mode_f32)
                    .rim_boost(self.rim_boost)
                    .inner_glow(self.inner_glow)
                    .luminance_threshold(self.luminance_threshold)
                    .tint(
                        hour_12 as f32,
                        now.minute() as f32,
                        now.second() as f32,
                        is_light_f32,
                    )
                    .element_type(40.0)
                    .deformation(0.0);

                if self.show_glass_card {
                    let card_w = 980.0f32;
                    let card_h = 350.0f32;
                    let cx = (self.width as f32) * 0.5;
                    let cy = (self.height as f32) * 0.5;
                    let card_left = cx - card_w * 0.5;
                    let card_top = cy - card_h * 0.5;

                    let x_norm = card_left / (self.width as f32);
                    let y_norm = card_top / (self.height as f32);
                    let w_norm = card_w / (self.width as f32);
                    let h_norm = card_h / (self.height as f32);

                    let hero_card = GlassSurface::new()
                        .material(Material::LiquidGlass)
                        .shape(Shape::RoundedRectangle {
                            radius: Tokens::RADIUS_OUTER,
                        })
                        .bounds(x_norm, y_norm, w_norm, h_norm)
                        .curvature(18.0)
                        .bevel(12.0)
                        .refraction(14.0)
                        .dispersion(0.12)
                        .blur(self.blur_radius)
                        .specular(1.05)
                        .fresnel(0.60)
                        .style_mode(self.style_mode)
                        .contrast_mode(contrast_mode_f32)
                        .rim_boost(self.rim_boost)
                        .inner_glow(self.inner_glow)
                        .luminance_threshold(self.luminance_threshold)
                        .tint(0.0, 0.0, 1.0, is_light_f32)
                        .element_type(24.0)
                        .deformation(0.0);

                    let _ = renderer.render(&[hero_card, liquid_text]);
                } else {
                    let _ = renderer.render(&[liquid_text]);
                }
            } else if self.show_glass_card {
                let card_w = 980.0f32;
                let card_h = 240.0f32;
                let cx = (self.width as f32) * 0.5;
                let cy = (self.height as f32) * 0.5;
                let card_left = cx - card_w * 0.5;
                let card_top = cy - card_h * 0.5;

                let x_norm = card_left / (self.width as f32);
                let y_norm = card_top / (self.height as f32);
                let w_norm = card_w / (self.width as f32);
                let h_norm = card_h / (self.height as f32);

                let hero_card = GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::RoundedRectangle {
                        radius: Tokens::RADIUS_OUTER,
                    })
                    .bounds(x_norm, y_norm, w_norm, h_norm)
                    .curvature(18.0)
                    .bevel(12.0)
                    .refraction(14.0)
                    .dispersion(0.12)
                    .blur(self.blur_radius)
                    .specular(1.05)
                    .fresnel(0.60)
                    .style_mode(self.style_mode)
                    .contrast_mode(contrast_mode_f32)
                    .rim_boost(self.rim_boost)
                    .inner_glow(self.inner_glow)
                    .luminance_threshold(self.luminance_threshold)
                    .tint(0.0, 0.0, 1.0, is_light_f32)
                    .element_type(24.0)
                    .deformation(0.0);

                let _ = renderer.render(&[hero_card, frameless_overlay]);
            } else {
                // Pure frameless Day & Date floating on wallpaper
                let _ = renderer.render(&[frameless_overlay]);
            }
            true
        } else {
            false
        }
    }
}

/// Calculates high-contrast text color, primary protective shadow, and optional bilateral ambient halo
/// based on the optical characteristics and multi-tone variance of the underlying wallpaper zone.
#[allow(clippy::too_many_arguments)]
pub fn compute_zone_text_styling(
    theme_mode: u32,
    luminance_threshold: f32,
    is_wallpaper_light: bool,
    adaptive_halo: bool,
    zone: Option<&ZoneMetrics>,
    is_multi_tone_overall: bool,
    base_light: [u8; 4],
    base_dark: [u8; 4],
) -> ([u8; 4], [u8; 4], Option<[u8; 4]>) {
    let is_light = match theme_mode {
        1 => false, // Forced Dark
        2 => true,  // Forced Light
        _ => {
            if let Some(z) = zone {
                z.luma_mean > luminance_threshold
            } else {
                is_wallpaper_light
            }
        }
    };

    let base_col = if is_light { base_light } else { base_dark };

    // Subtle chromatic tint harmonic blended from wallpaper hue (5-12%, default 8%)
    let tint_vec = zone.map(|z| z.dominant_tint).unwrap_or([1.0, 1.0, 1.0]);
    let harmonic_weight = 0.08f32;
    let tint_srgb = [
        (tint_vec[0] * 255.0).clamp(0.0, 255.0),
        (tint_vec[1] * 255.0).clamp(0.0, 255.0),
        (tint_vec[2] * 255.0).clamp(0.0, 255.0),
    ];
    let col_r = ((1.0 - harmonic_weight) * base_col[0] as f32 + harmonic_weight * tint_srgb[0])
        .round() as u8;
    let col_g = ((1.0 - harmonic_weight) * base_col[1] as f32 + harmonic_weight * tint_srgb[1])
        .round() as u8;
    let col_b = ((1.0 - harmonic_weight) * base_col[2] as f32 + harmonic_weight * tint_srgb[2])
        .round() as u8;
    let text_col = [col_r, col_g, col_b, base_col[3]];

    // Primary protective shadow:
    // Light mode: clean soft light halo [255, 255, 255, 80]
    // Dark mode: deep ambient drop shadow [0, 0, 0, 190]
    let primary_shadow = if is_light {
        [255, 255, 255, 80]
    } else {
        [0, 0, 0, 190]
    };

    // Bilateral ambient halo:
    // When multi-tone variance is detected (is_multi_tone == true && adaptive_halo == true):
    // Render a bilateral opposite-phase halo so text pops across split/high-variance boundaries
    let is_multi_tone = zone.map(|z| z.is_multi_tone).unwrap_or(false) || is_multi_tone_overall;

    let secondary_halo = if is_multi_tone && adaptive_halo {
        if is_light {
            Some([0, 0, 0, 140])
        } else {
            Some([255, 255, 255, 110])
        }
    } else {
        None
    };

    (text_col, primary_shadow, secondary_halo)
}

/// Evaluates full optical styling for Day, Date, and AM/PM zones from an OpticalZoneProfile.
#[allow(dead_code, clippy::type_complexity)]
pub fn compute_clock_text_styling(
    theme_mode: u32,
    luminance_threshold: f32,
    is_wallpaper_light: bool,
    adaptive_halo: bool,
    profile: &OpticalZoneProfile,
) -> (
    ([u8; 4], [u8; 4], Option<[u8; 4]>), // day: (text, shadow, halo)
    ([u8; 4], [u8; 4], Option<[u8; 4]>), // date: (text, shadow, halo)
    ([u8; 4], [u8; 4], Option<[u8; 4]>), // ampm: (text, shadow, halo)
) {
    let day = compute_zone_text_styling(
        theme_mode,
        luminance_threshold,
        is_wallpaper_light,
        adaptive_halo,
        Some(&profile.day_zone),
        profile.is_multi_tone,
        COL_DAY_BASE_LIGHT,
        COL_DAY_BASE_DARK,
    );
    let date = compute_zone_text_styling(
        theme_mode,
        luminance_threshold,
        is_wallpaper_light,
        adaptive_halo,
        Some(&profile.date_zone),
        profile.is_multi_tone,
        COL_DATE_BASE_LIGHT,
        COL_DATE_BASE_DARK,
    );
    let ampm = compute_zone_text_styling(
        theme_mode,
        luminance_threshold,
        is_wallpaper_light,
        adaptive_halo,
        Some(&profile.numeral_zone),
        profile.is_multi_tone,
        COL_AMPM_BASE_LIGHT,
        COL_AMPM_BASE_DARK,
    );
    (day, date, ampm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_text_styling_contrast_switching() {
        // Light background zone
        let light_zone = ZoneMetrics {
            luma_mean: 0.85,
            luma_std_dev: 0.02,
            dominant_tint: [0.9, 0.9, 0.9],
            is_multi_tone: false,
            is_light: true,
        };
        let (col_light, shadow_light, halo_light) = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&light_zone),
            false,
            [18, 22, 32, 248],
            [248, 250, 255, 248],
        );
        // Light background should use dark text (low values)
        assert!(col_light[0] < 50 && col_light[1] < 50 && col_light[2] < 60);
        // Shadow should be light halo
        assert_eq!(shadow_light[0], 255);
        // No multi-tone -> no secondary halo
        assert!(halo_light.is_none());

        // Dark background zone
        let dark_zone = ZoneMetrics {
            luma_mean: 0.05,
            luma_std_dev: 0.01,
            dominant_tint: [0.05, 0.05, 0.05],
            is_multi_tone: false,
            is_light: false,
        };
        let (col_dark, shadow_dark, halo_dark) = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&dark_zone),
            false,
            [18, 22, 32, 248],
            [248, 250, 255, 248],
        );
        // Dark background should use light text (high values)
        assert!(col_dark[0] > 200 && col_dark[1] > 200 && col_dark[2] > 200);
        // Shadow should be dark drop shadow
        assert_eq!(shadow_dark[0], 0);
        assert!(halo_dark.is_none());
    }

    #[test]
    fn test_zone_text_styling_bilateral_halo_on_multi_tone() {
        let multi_tone_dark_zone = ZoneMetrics {
            luma_mean: 0.20,
            luma_std_dev: 0.25,
            dominant_tint: [0.2, 0.2, 0.2],
            is_multi_tone: true,
            is_light: false,
        };
        let (_col, _shadow, halo) = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&multi_tone_dark_zone),
            false,
            [18, 22, 32, 248],
            [248, 250, 255, 248],
        );
        // Multi-tone dark zone must activate secondary luminous halo
        assert!(halo.is_some());
        let halo_col = halo.unwrap();
        assert_eq!(halo_col[0], 255); // White luminous glow
    }

    #[test]
    fn test_contrast_preset_mapping() {
        assert_eq!(ContrastPreset::Auto.as_f32(), 0.0);
        assert_eq!(ContrastPreset::HighContrast.as_f32(), 1.0);
        assert_eq!(ContrastPreset::Subtle.as_f32(), 2.0);
        assert_eq!(ContrastPreset::Crystalline.as_f32(), 3.0);

        assert_eq!(
            ContrastPreset::parse_str("high_contrast"),
            ContrastPreset::HighContrast
        );
        assert_eq!(
            ContrastPreset::parse_str("crystalline"),
            ContrastPreset::Crystalline
        );
        assert_eq!(ContrastPreset::parse_str("subtle"), ContrastPreset::Subtle);
        assert_eq!(ContrastPreset::parse_str("auto"), ContrastPreset::Auto);
    }
}
