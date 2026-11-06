use chrono::{Datelike, Local, Timelike};
use glass_core::font::{FontWeight, QuicksandFont, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material};
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
use wayland_client::protocol::wl_surface;
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

pub const CARD_PAD: f32 = 28.0;
pub const CARD_W: f32 = 320.0;
pub const CARD_H: f32 = 218.0;

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
    pub date_is_light: bool,
    pub time_is_light: bool,
    pub bat_is_light: bool,

    pub battery_level: f32,
    pub last_battery_check: Instant,
    pub last_minute: u32,
}

impl ClockWidget {
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
        let width = (CARD_W + CARD_PAD * 2.0) as u32; // 376
        let height = (CARD_H + CARD_PAD * 2.0) as u32; // 274
        let pos_x = 0i32;
        let pos_y = 28i32;

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Bottom,
            Some("liquid-glass-clock"),
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
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn init_gpu(&mut self, gpu: &glass_core::SharedGpu) {
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

    pub fn on_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event.kind {
            PointerEventKind::Enter { .. } => {
                self.is_hovered = true;
                self.cursor_pos = [
                    (event.position.0 as f32) / (self.width as f32),
                    (event.position.1 as f32) / (self.height as f32),
                ];
                true
            }
            PointerEventKind::Leave { .. } => {
                self.is_hovered = false;
                self.cursor_pos = [-10.0, -10.0];
                true
            }
            PointerEventKind::Motion { .. } => {
                let nx = (event.position.0 as f32) / (self.width as f32);
                let ny = (event.position.1 as f32) / (self.height as f32);
                self.cursor_pos = [nx, ny];
                true
            }
            _ => false,
        }
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

    #[allow(clippy::too_many_arguments)]
    fn sample_subcrop_is_light(
        crop: &[u8],
        crop_w: u32,
        crop_h: u32,
        lx: u32,
        ly: u32,
        lw: u32,
        lh: u32,
        current_state: bool,
    ) -> bool {
        let mut sub = Vec::with_capacity((lw * lh * 4) as usize);
        for row in 0..lh {
            let sy = (ly + row).min(crop_h.saturating_sub(1));
            let sx = lx.min(crop_w.saturating_sub(1));
            let actual_w = lw.min(crop_w.saturating_sub(sx));
            let src_idx = ((sy * crop_w + sx) * 4) as usize;
            let end_idx = src_idx + (actual_w * 4) as usize;
            if end_idx <= crop.len() {
                sub.extend_from_slice(&crop[src_idx..end_idx]);
            }
        }
        WallpaperManager::is_buffer_light_hysteresis(&sub, current_state)
    }

    pub fn render_ui_overlay(&mut self, buf: &mut [u8]) {
        if let Some(crop) = &self.wallpaper_crop {
            self.date_is_light = Self::sample_subcrop_is_light(
                crop,
                self.width,
                self.height,
                (CARD_PAD + 30.0) as u32,
                (CARD_PAD + 15.0) as u32,
                260,
                50,
                self.date_is_light,
            );
            self.time_is_light = Self::sample_subcrop_is_light(
                crop,
                self.width,
                self.height,
                (CARD_PAD + 30.0) as u32,
                (CARD_PAD + 70.0) as u32,
                260,
                85,
                self.time_is_light,
            );
            self.bat_is_light = Self::sample_subcrop_is_light(
                crop,
                self.width,
                self.height,
                (CARD_PAD + 40.0) as u32,
                (CARD_PAD + 155.0) as u32,
                240,
                48,
                self.bat_is_light,
            );
        }

        let (date_col, date_sec) = if self.date_is_light {
            ([8, 12, 20, 255], [18, 24, 34, 255])
        } else {
            ([255, 255, 255, 255], [240, 245, 255, 255])
        };

        let (time_col, ampm_col) = if self.time_is_light {
            ([8, 12, 20, 255], [18, 24, 34, 255])
        } else {
            ([255, 255, 255, 255], [240, 245, 255, 255])
        };

        let bat_col = if self.bat_is_light {
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

        let wday_str = match now.weekday().num_days_from_sunday() {
            1 => "MON",
            2 => "TUE",
            3 => "WED",
            4 => "THU",
            5 => "FRI",
            6 => "SAT",
            _ => "SUN",
        };

        let month_str = match now.month() {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            _ => "Dec",
        };
        let date_str = format!("{} {:02}", month_str, now.day());
        let full_date_str = format!("{}, {}", wday_str, date_str);
        let bat_str = format!("{:.0}%", self.battery_level);

        let half_w = self.width as f32 * 0.5;
        let card_top = CARD_PAD;

        // Top Centered Date
        let date_w = self
            .font
            .measure_text(&full_date_str, 16.5, FontWeight::Bold);
        let date_total_w = 18.0 + date_w;
        let date_start_x = half_w - date_total_w * 0.5;
        UIFont::draw_calendar_icon(
            buf,
            self.width,
            self.height,
            date_start_x + 6.0,
            card_top + 44.0,
            date_sec,
        );
        self.font.draw_text_centered_y(
            buf,
            self.width,
            self.height,
            &full_date_str,
            date_start_x + 18.0,
            card_top + 44.0,
            16.5,
            date_col,
            col_shadow,
            FontWeight::Bold,
        );

        // Hero Digital Clock
        let time_baseline =
            self.font
                .vertical_center_baseline(card_top + 110.0, 68.0, FontWeight::Bold);
        let time_w = self.font.measure_text(&time_str, 68.0, FontWeight::Bold);
        let ampm_w = self.font.measure_text(ampm, 18.0, FontWeight::Bold);
        let total_w = time_w + 10.0 + ampm_w;
        let time_x = half_w - total_w * 0.5;
        let end_x = self.font.draw_text_with_shadow(
            buf,
            self.width,
            self.height,
            &time_str,
            time_x,
            time_baseline,
            68.0,
            time_col,
            col_shadow,
            FontWeight::Bold,
        );

        self.font.draw_text_with_shadow(
            buf,
            self.width,
            self.height,
            ampm,
            end_x + 10.0,
            time_baseline - 8.0,
            18.0,
            ampm_col,
            col_shadow,
            FontWeight::Bold,
        );

        // Battery Capsule
        let bat_text_w = self.font.measure_text(&bat_str, 15.0, FontWeight::Bold);
        let bat_total_w = bat_text_w + 12.0 + 26.0;
        let bat_start_x = half_w - bat_total_w * 0.5;
        self.font.draw_text_centered_y(
            buf,
            self.width,
            self.height,
            &bat_str,
            bat_start_x,
            card_top + 176.0,
            15.0,
            bat_col,
            col_shadow,
            FontWeight::Bold,
        );
        UIFont::draw_battery_capsule(
            buf,
            self.width,
            self.height,
            bat_start_x + bat_text_w + 18.0,
            card_top + 176.0,
            self.battery_level,
            bat_col,
        );
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

        let res = if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &overlay_vec);

            let now = Local::now();
            let wday = now.weekday().num_days_from_sunday() as f32;
            let month = now.month() as f32;
            let mday = now.day() as f32;
            let date_val = wday * 10000.0 + month * 100.0 + mday;

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);
            let is_light_f32 = if self.is_wallpaper_light { 1.0 } else { 0.0 };

            let x_norm = CARD_PAD / (self.width as f32);
            let y_norm = CARD_PAD / (self.height as f32);
            let w_norm = CARD_W / (self.width as f32);
            let h_norm = CARD_H / (self.height as f32);

            let clock_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 28.0 })
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(0.04)
                .bevel(14.0)
                .refraction(0.40)
                .dispersion(0.015)
                .blur(8.0)
                .specular(1.10)
                .fresnel(0.60)
                .tint(0.0, 0.0, 0.0, is_light_f32)
                .roughness(self.battery_level)
                .specular(0.85)
                .fresnel(0.65)
                .element_type(25.0)
                .deformation(date_val)
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
