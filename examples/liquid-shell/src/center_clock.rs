use chrono::{Local, Timelike};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu, Tokens};
use glass_platform::wallpaper::WallpaperManager;
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

pub const CENTER_HERO_W: u32 = 1100;
pub const CENTER_HERO_H: u32 = 420;

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

        // Positioned at optical center: 1920x1080 -> centered horizontally, y ~ 330
        let pos_x = (1920 - width as i32) / 2;
        let pos_y = 330i32;

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
        };

        widget.load_config();
        widget
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
        self.is_wallpaper_light =
            WallpaperManager::is_buffer_light_hysteresis(&crop, self.is_wallpaper_light);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
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

    fn render_ui_overlay(&mut self) {
        let buf_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != buf_len {
            self.overlay_buffer.resize(buf_len, 0);
        }
        self.overlay_buffer.fill(0);

        if !self.enabled {
            return;
        }

        let is_light = self.effective_is_light();
        let col_day = if is_light {
            [18, 22, 32, 245]
        } else {
            [248, 250, 255, 245]
        };
        let col_date = if is_light {
            [55, 68, 88, 210]
        } else {
            [205, 220, 245, 210]
        };
        let _col_ampm = if is_light {
            [90, 105, 128, 200]
        } else {
            [175, 195, 225, 200]
        };
        let col_shadow = if is_light {
            [255, 255, 255, 75]
        } else {
            [0, 0, 0, 180]
        };

        let now = Local::now();
        let wday_full = now.format("%A").to_string().to_uppercase();
        let date_full = now.format("%B %d, %Y").to_string().to_uppercase();
        let (_hour_12, _ampm) = match now.hour12() {
            (true, h) => (h, "PM"),
            (false, h) => (h, "AM"),
        };

        let cx = (self.width as f32) * 0.5;
        let cy = (self.height as f32) * 0.5;
        let buf = &mut self.overlay_buffer;

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
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_day_x,
                    day_cy,
                    62.0,
                    col_day,
                    col_shadow,
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
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_date_x,
                    date_cy,
                    22.0,
                    col_date,
                    col_shadow,
                    FontWeight::Medium,
                );
                cur_date_x += ch_w;
                if i < date_chars.len() - 1 {
                    cur_date_x += date_tracking;
                }
            }
        } else {
            // ── 1. Monumental Rice Day ("W E D N E S D A Y") ──
            let day_tracking = 12.0f32;
            let day_chars: Vec<char> = wday_full.chars().collect();
            let mut total_day_w = 0.0f32;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                total_day_w += self.font.measure_text(&s, 54.0, FontWeight::Bold);
                if i < day_chars.len() - 1 {
                    total_day_w += day_tracking;
                }
            }
            let mut cur_day_x = cx - total_day_w * 0.5;
            let day_cy = cy - 66.0;
            for (i, &ch) in day_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 54.0, FontWeight::Bold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_day_x,
                    day_cy,
                    54.0,
                    col_day,
                    col_shadow,
                    FontWeight::Bold,
                );
                cur_day_x += ch_w;
                if i < day_chars.len() - 1 {
                    cur_day_x += day_tracking;
                }
            }

            // ── 2. Refined Tracked Date ("NOVEMBER 11, 2026") ──
            let date_tracking = 5.5f32;
            let mut total_date_w = 0.0f32;
            let date_chars: Vec<char> = date_full.chars().collect();
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                total_date_w += self.font.measure_text(&s, 19.0, FontWeight::SemiBold);
                if i < date_chars.len() - 1 {
                    total_date_w += date_tracking;
                }
            }
            let mut cur_date_x = cx - total_date_w * 0.5;
            let date_cy = cy - 10.0;
            for (i, &ch) in date_chars.iter().enumerate() {
                let s = ch.to_string();
                let ch_w = self.font.measure_text(&s, 19.0, FontWeight::SemiBold);
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &s,
                    cur_date_x,
                    date_cy,
                    19.0,
                    col_date,
                    col_shadow,
                    FontWeight::SemiBold,
                );
                cur_date_x += ch_w;
                if i < date_chars.len() - 1 {
                    cur_date_x += date_tracking;
                }
            }

            // ── 3. The Classic Hero Time ("09:08 PM") - Old Design Itself ──
            let (hour_12, ampm) = match now.hour12() {
                (true, h) => (h, "PM"),
                (false, h) => (h, "AM"),
            };
            let time_str = format!("{:02}:{:02} {}", hour_12, now.minute(), ampm);
            let time_w = self.font.measure_text(&time_str, 84.0, FontWeight::Bold);
            let time_x = cx - time_w * 0.5;
            let time_cy = cy + 68.0;
            let col_time = if is_light {
                [18, 22, 32, 255]
            } else {
                [255, 255, 255, 255]
            };
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &time_str,
                time_x,
                time_cy,
                84.0,
                col_time,
                col_shadow,
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

        if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &self.overlay_buffer);

            let frameless_overlay = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 0.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .element_type(42.0); // Pure Frameless Overlay Pass!

            if self.show_glass_card {
                let card_w = 980.0f32;
                let card_h = if self.show_time { 350.0f32 } else { 240.0f32 };
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
                    .tint(0.0, 0.0, 1.0, is_light_f32)
                    .element_type(24.0)
                    .deformation(0.0);

                let _ = renderer.render(&[hero_card, frameless_overlay]);
            } else {
                let _ = renderer.render(&[frameless_overlay]);
            }
            true
        } else {
            false
        }
    }
}
