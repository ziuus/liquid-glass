use glass_core::{GlassRenderer, GlassSurface, Material, geometry::Shape};
mod capture;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure, KeyboardInteractivity,
        },
    },
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        SeatHandler, SeatState, Capability,
    },
    delegate_seat, delegate_pointer,
};
use std::sync::Arc;
use wayland_client::{
    backend::ObjectId,
    protocol::{wl_output, wl_surface, wl_seat, wl_pointer},
    Connection, QueueHandle, Proxy,
};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::delegate_shm;

struct AppData {
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
    cursor_velocity: [f32; 2],
    hover_pos: [f32; 8],
    hover_vel: [f32; 8],
    is_pressed: bool,
    brightness_progress: f32,
    volume_progress: f32,
    
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,
    frame_counter: u32,
    animating: bool,
    qh: Option<QueueHandle<Self>>,
}

struct WaylandWindow {
    surface: wl_surface::WlSurface,
    display: *mut std::ffi::c_void,
}

unsafe impl Send for WaylandWindow {}
unsafe impl Sync for WaylandWindow {}

impl HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = WaylandWindowHandle::new({
            let ptr = self.surface.id().as_ptr() as *mut _;
            std::ptr::NonNull::new(ptr).unwrap()
        });
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle)) })
    }
}

impl HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let handle = WaylandDisplayHandle::new({
            std::ptr::NonNull::new(self.display).unwrap()
        });
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(handle)) })
    }
}

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {}
    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let still_animating = self.draw();
        if still_animating {
            surface.frame(qh, surface.clone());
        } else {
            self.animating = false;
        }
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {}
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 360;
            self.height = 480;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.width, self.height);
        }

        if self.first_configure {
            self.first_configure = false;
        }
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self.seat_state.get_pointer(qh, &seat).expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for AppData {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let mut had_event = false;
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    had_event = true;
                    if self.width > 0 && self.height > 0 {
                        let nx = (event.position.0 as f32 / self.width as f32).clamp(0.0, 1.0);
                        let ny = (event.position.1 as f32 / self.height as f32).clamp(0.0, 1.0);
                        self.cursor_velocity = [nx - self.cursor_pos[0], ny - self.cursor_pos[1]];
                        self.cursor_pos = [nx, ny];
                    }
                }
                PointerEventKind::Leave { .. } => {
                    had_event = true;
                    self.cursor_pos = [-10.0, -10.0];
                    self.cursor_velocity = [0.0, 0.0];
                    self.is_pressed = false;
                }
                PointerEventKind::Press { .. } => {
                    had_event = true;
                    self.is_pressed = true;
                }
                PointerEventKind::Release { .. } => {
                    had_event = true;
                    self.is_pressed = false;
                }
                _ => {}
            }
        }

        if had_event {
            if !self.animating {
                self.animating = true;
                if let Some(wrapper) = &self.window_wrapper {
                    wrapper.surface.frame(qh, wrapper.surface.clone());
                }
            }
            self.draw();
        }
    }
}

impl AppData {
    fn draw(&mut self) -> bool {
        self.frame_counter += 1;
        if self.frame_counter % 60 == 1 {
            eprintln!("[GLASS] Rendered frame {}, cursor={:?}", self.frame_counter, self.cursor_pos);
        }
        if self.frame_counter % 300 == 0 {
            if let Some((cw, ch, bg_data)) = load_wallpaper_region(40, 40, self.width, self.height) {
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_background(cw, ch, &bg_data);
                }
            }
        }

        // Render clean, minimal Apple liquid glass with nested subcomponents
        if let Some(renderer) = &mut self.renderer {
            let px = self.cursor_pos[0];
            let py = self.cursor_pos[1];
            let vx = self.cursor_velocity[0];
            let vy = self.cursor_velocity[1];

            // Authentic macOS Control Center Layout Specifications: [cx, cy, base_w, base_h]
            let specs: [[f32; 4]; 8] = [
                [0.50, 0.08, 0.86, 0.07],   // 0: Dynamic Island Header
                [0.28, 0.21, 0.40, 0.15],   // 1: Wi-Fi Capsule
                [0.72, 0.21, 0.40, 0.15],   // 2: Bluetooth Capsule
                [0.50, 0.38, 0.86, 0.13],   // 3: Display Brightness Liquid Slider
                [0.50, 0.54, 0.86, 0.13],   // 4: Sound Volume Liquid Slider
                [0.22, 0.73, 0.24, 0.16],   // 5: Focus / Moon Liquid Lens
                [0.50, 0.73, 0.26, 0.17],   // 6: Screen Mirroring Hero Lens
                [0.78, 0.73, 0.24, 0.16],   // 7: Theme / Night Shift Lens
            ];

            // Continuous Gaussian proximity wave & smooth second-order spring dynamics
            let sigma = 0.15f32;
            let two_sigma_sq = 2.0 * sigma * sigma;
            let dt = 1.0f32 / 60.0f32;
            let spring_k = 260.0f32;
            let damping = 18.0f32;
            let mut active = false;

            for i in 0..8 {
                let [cx, cy, bw, bh] = specs[i];
                let in_box = px >= cx - bw * 0.5 && px <= cx + bw * 0.5 && py >= cy - bh * 0.5 && py <= cy + bh * 0.5;
                
                let dist_sq = (px - cx) * (px - cx) + (py - cy) * (py - cy);
                let proximity = (-dist_sq / two_sigma_sq).exp();
                
                let target = if in_box {
                    1.0f32
                } else if px >= 0.0 {
                    proximity * 0.35f32
                } else {
                    0.0f32
                };

                let force = -spring_k * (self.hover_pos[i] - target) - damping * self.hover_vel[i];
                self.hover_vel[i] += force * dt;
                self.hover_pos[i] += self.hover_vel[i] * dt;

                if (self.hover_pos[i] - target).abs() > 0.002 || self.hover_vel[i].abs() > 0.002 {
                    active = true;
                }
            }

            // Interactive continuous liquid slider dragging
            if self.is_pressed {
                active = true;
                if py >= 0.31 && py <= 0.45 {
                    self.brightness_progress = ((px - 0.10) / 0.80).clamp(0.06, 0.96);
                } else if py >= 0.47 && py <= 0.61 {
                    self.volume_progress = ((px - 0.10) / 0.80).clamp(0.06, 0.96);
                }
            }

            if self.cursor_velocity[0].abs() > 0.0001 || self.cursor_velocity[1].abs() > 0.0001 {
                active = true;
            }

            // Subtle, anchored Apple button scaling (clean geometric stability)
            let calc_stretch_bounds = |idx: usize, press: f32| -> [f32; 4] {
                let [cx, cy, bw, bh] = specs[idx];
                let t = self.hover_pos[idx].clamp(0.0, 1.0);
                let scale = 1.0 + t * 0.025 + press;
                let w = bw * scale;
                let h = bh * scale;
                [cx - w * 0.5, cy - h * 0.5, w, h]
            };

            // ── Surface 0: Base Liquid Glass Widget ──
            let panel = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 28.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.08)
                .bevel(10.0)
                .refraction(0.24)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .roughness(0.0)
                .specular(0.25)
                .fresnel(0.40)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 1: Top Status Pill (Dynamic Island) ──
            let t0 = self.hover_pos[0].clamp(0.0, 1.0);
            let b0 = calc_stretch_bounds(0, 0.0);
            let header_pill = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b0[0], b0[1], b0[2], b0[3])
                .curvature(0.08 + t0 * 0.02)
                .bevel(5.0 + t0 * 1.5)
                .refraction(0.18 + t0 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t0 * 0.12)
                .fresnel(0.35 + t0 * 0.08)
                .element_type(1.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 2: Wi-Fi Liquid Capsule ──
            let t1 = self.hover_pos[1].clamp(0.0, 1.0);
            let p1 = if self.is_pressed && t1 > 0.4 { -0.010 } else { 0.0 };
            let b1 = calc_stretch_bounds(1, p1);
            let wifi_capsule = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 18.0 })
                .bounds(b1[0], b1[1], b1[2], b1[3])
                .curvature(0.08 + t1 * 0.02)
                .bevel(5.0 + t1 * 1.5)
                .refraction(0.18 + t1 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t1 * 0.12)
                .fresnel(0.35 + t1 * 0.08)
                .element_type(2.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 3: Bluetooth Liquid Capsule ──
            let t2 = self.hover_pos[2].clamp(0.0, 1.0);
            let p2 = if self.is_pressed && t2 > 0.4 { -0.010 } else { 0.0 };
            let b2 = calc_stretch_bounds(2, p2);
            let bluetooth_capsule = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 18.0 })
                .bounds(b2[0], b2[1], b2[2], b2[3])
                .curvature(0.08 + t2 * 0.02)
                .bevel(5.0 + t2 * 1.5)
                .refraction(0.18 + t2 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t2 * 0.12)
                .fresnel(0.35 + t2 * 0.08)
                .element_type(3.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 4: Display Brightness Liquid Slider ──
            let t3 = self.hover_pos[3].clamp(0.0, 1.0);
            let b3 = calc_stretch_bounds(3, 0.0);
            let brightness_slider = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b3[0], b3[1], b3[2], b3[3])
                .curvature(0.08 + t3 * 0.02)
                .bevel(5.0 + t3 * 1.5)
                .refraction(0.18 + t3 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.28 + t3 * 0.10)
                .fresnel(0.35 + t3 * 0.08)
                .element_type(4.0)
                .progress(self.brightness_progress)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 5: Sound Volume Liquid Slider ──
            let t4 = self.hover_pos[4].clamp(0.0, 1.0);
            let b4 = calc_stretch_bounds(4, 0.0);
            let volume_slider = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b4[0], b4[1], b4[2], b4[3])
                .curvature(0.08 + t4 * 0.02)
                .bevel(5.0 + t4 * 1.5)
                .refraction(0.18 + t4 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.28 + t4 * 0.10)
                .fresnel(0.35 + t4 * 0.08)
                .element_type(5.0)
                .progress(self.volume_progress)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponents 6, 7, 8: Bottom Trio of Liquid Lenses ──
            let t5 = self.hover_pos[5].clamp(0.0, 1.0);
            let p5 = if self.is_pressed && t5 > 0.4 { -0.010 } else { 0.0 };
            let b5 = calc_stretch_bounds(5, p5);
            let focus_lens = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b5[0], b5[1], b5[2], b5[3])
                .curvature(0.08 + t5 * 0.02)
                .bevel(5.0 + t5 * 1.5)
                .refraction(0.18 + t5 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t5 * 0.12)
                .fresnel(0.35 + t5 * 0.08)
                .element_type(6.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            let t6 = self.hover_pos[6].clamp(0.0, 1.0);
            let p6 = if self.is_pressed && t6 > 0.4 { -0.012 } else { 0.0 };
            let b6 = calc_stretch_bounds(6, p6);
            let mirror_lens = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b6[0], b6[1], b6[2], b6[3])
                .curvature(0.08 + t6 * 0.02)
                .bevel(5.0 + t6 * 1.5)
                .refraction(0.18 + t6 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t6 * 0.12)
                .fresnel(0.35 + t6 * 0.08)
                .element_type(7.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            let t7 = self.hover_pos[7].clamp(0.0, 1.0);
            let p7 = if self.is_pressed && t7 > 0.4 { -0.010 } else { 0.0 };
            let b7 = calc_stretch_bounds(7, p7);
            let theme_lens = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b7[0], b7[1], b7[2], b7[3])
                .curvature(0.08 + t7 * 0.02)
                .bevel(5.0 + t7 * 1.5)
                .refraction(0.18 + t7 * 0.04)
                .dispersion(0.010)
                .blur(3.5)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.30 + t7 * 0.12)
                .fresnel(0.35 + t7 * 0.08)
                .element_type(8.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // Fluid damping for pointer velocity
            self.cursor_velocity[0] *= 0.85;
            self.cursor_velocity[1] *= 0.85;

            let surfaces = vec![
                panel,
                header_pill,
                wifi_capsule,
                bluetooth_capsule,
                brightness_slider,
                volume_slider,
                focus_lens,
                mirror_lens,
                theme_lens,
            ];
            let _ = renderer.render(&surfaces);
            active
        } else {
            false
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

fn load_wallpaper_region(x: u32, y: u32, w: u32, h: u32) -> Option<(u32, u32, Vec<u8>)> {
    let path = std::process::Command::new("awww")
        .arg("query")
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            s.split("image: ").nth(1).map(|p| p.trim().to_string())
        })
        .unwrap_or_else(|| "/home/zius/Pictures/wallpaper/Fantasy-Landscape3.png".to_string());

    let img = image::open(&path).ok()?.to_rgba8();
    let (img_w, img_h) = img.dimensions();
    
    let screen_w = 1920.0f32;
    let screen_h = 1080.0f32;
    let sx = (x as f32 / screen_w * img_w as f32).round() as u32;
    let sy = (y as f32 / screen_h * img_h as f32).round() as u32;
    let sw = ((w as f32 / screen_w * img_w as f32).round() as u32).min(img_w.saturating_sub(sx));
    let sh = ((h as f32 / screen_h * img_h as f32).round() as u32).min(img_h.saturating_sub(sy));
    
    if sw == 0 || sh == 0 {
        return None;
    }
    
    let sub = image::imageops::crop_imm(&img, sx, sy, sw, sh).to_image();
    let resized = image::imageops::resize(&sub, w, h, image::imageops::FilterType::Lanczos3);
    Some((w, h, resized.into_raw()))
}

fn main() {
    env_logger::init();
    
    let conn = Connection::connect_to_env().unwrap();
    let display = conn.display();
    let display_ptr = display.id().as_ptr() as *mut std::ffi::c_void;
    
    let (globals, mut event_queue) = wayland_client::globals::registry_queue_init::<AppData>(&conn).unwrap();
    let qh = event_queue.handle();
    
    let registry_state = RegistryState::new(&globals);
    let shm = Shm::bind(&globals, &qh).expect("wl_shm missing");
    let screencopy = capture::ScreencopyState::new(&registry_state, &qh, &shm);
    let seat_state = SeatState::new(&globals, &qh);

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
        cursor_pos: [-10.0, -10.0],
        cursor_velocity: [0.0, 0.0],
        hover_pos: [0.0; 8],
        hover_vel: [0.0; 8],
        is_pressed: false,
        brightness_progress: 0.78,
        volume_progress: 0.62,
        
        width: 360,
        height: 480,
        first_configure: true,
        exit: false,
        frame_counter: 0,
        animating: true,
        qh: Some(qh.clone()),
    };

    let surface = app_data.compositor_state.create_surface(&qh);
    
    let layer_surface = app_data.layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Bottom,
        Some("glass_panel"),
        None,
    );
    
    layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer_surface.set_size(app_data.width, app_data.height);
    layer_surface.set_margin(40, 0, 0, 40); // top, right, bottom, left
    layer_surface.set_exclusive_zone(0);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.commit();
    
    app_data.layer_surface = Some(layer_surface);
    
    let window_wrapper = Arc::new(WaylandWindow {
        surface: surface.clone(),
        display: display_ptr,
    });
    app_data.window_wrapper = Some(window_wrapper.clone());
    
    // Pump events to get initial configure
    while app_data.first_configure {
        event_queue.blocking_dispatch(&mut app_data).unwrap();
    }
    
    // Explicitly bind pointer if not already bound
    if app_data.pointer.is_none() {
        for seat in app_data.seat_state.seats() {
            if let Ok(ptr) = app_data.seat_state.get_pointer(&qh, &seat) {
                eprintln!("[GLASS] Successfully bound wl_pointer to seat!");
                app_data.pointer = Some(ptr);
                break;
            }
        }
    }
    eprintln!("[GLASS] Pointer initialized: {}", app_data.pointer.is_some());
    
    // Initialize WGPU renderer
    let mut renderer = pollster::block_on(GlassRenderer::new(
        window_wrapper,
        app_data.width,
        app_data.height
    )).expect("Failed to init WGPU");
    
    // 1. Primary: load authentic wallpaper background directly
    if let Some((cw, ch, bg_data)) = load_wallpaper_region(40, 40, app_data.width, app_data.height) {
        renderer.update_background(cw, ch, &bg_data);
    } else if let Some(output) = app_data.output_state.outputs().next() {
        // Fallback: screencopy capture
        app_data.screencopy.capture(&output, 40, 40, app_data.width as i32, app_data.height as i32, &qh);
        while !app_data.screencopy.done {
            event_queue.blocking_dispatch(&mut app_data).unwrap();
        }
        if let Some(bg_data) = app_data.screencopy.data.take() {
            let cw = app_data.screencopy.width;
            let ch = app_data.screencopy.height;
            renderer.update_background(cw, ch, &bg_data);
        }
    }
    
    app_data.renderer = Some(renderer);
    
    // Start drawing frame callback
    surface.frame(&qh, surface.clone());
    app_data.draw();
    
    while !app_data.exit {
        event_queue.blocking_dispatch(&mut app_data).unwrap();
    }
}
