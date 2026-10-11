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
    hover_pos: [f32; 3],
    hover_vel: [f32; 3],
    is_pressed: bool,
    selected_mode: usize,
    pill_offset_x: f32,
    pill_vel_x: f32,
    
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

        // Render authentic Liquid Glass Camera Toolbar (liquidglassdesign.com reference)
        if let Some(renderer) = &mut self.renderer {
            let px = self.cursor_pos[0];
            let py = self.cursor_pos[1];
            let vx = self.cursor_velocity[0];
            let vy = self.cursor_velocity[1];

            // Camera Toolbar Specifications: [cx, cy, base_w, base_h]
            // 0: Left Circular Camera Lens / Gallery Disc (diameter 56px)
            // 1: Center Glass Track Capsule (290px x 56px)
            // 2: Right Circular Switch Camera Button (diameter 56px)
            let specs: [[f32; 4]; 3] = [
                [68.0 / 540.0, 0.50, 56.0 / 540.0, 56.0 / 84.0],
                [0.50, 0.50, 290.0 / 540.0, 56.0 / 84.0],
                [(540.0 - 68.0) / 540.0, 0.50, 56.0 / 540.0, 56.0 / 84.0],
            ];

            let dt = 1.0f32 / 60.0f32;
            let spring_k = 260.0f32;
            let damping = 18.0f32;
            let mut active = false;

            // Interactive mode selection on click / drag
            if self.is_pressed {
                active = true;
                let track_min_x = 0.50 - (290.0 / 540.0) * 0.5;
                let track_max_x = 0.50 + (290.0 / 540.0) * 0.5;
                if px >= track_min_x && px <= track_max_x && py >= 0.15 && py <= 0.85 {
                    if px < 0.40 {
                        self.selected_mode = 0; // PHOTO
                    } else if px <= 0.60 {
                        self.selected_mode = 2; // VIDEO
                    } else {
                        self.selected_mode = 1; // CINEMATIC
                    }
                }
            }

            // Target offset in pixels: -72.5 for PHOTO (left), 72.5 for CINEMATIC (right), 0.0 for VIDEO (center)
            let target_pill_x = match self.selected_mode {
                0 => -72.5f32,
                1 => 72.5f32,
                _ => 0.0f32,
            };

            // Fluid second-order harmonic oscillator for sliding liquid pill
            let pill_force = -spring_k * (self.pill_offset_x - target_pill_x) - damping * self.pill_vel_x;
            self.pill_vel_x += pill_force * dt;
            self.pill_offset_x += self.pill_vel_x * dt;

            if (self.pill_offset_x - target_pill_x).abs() > 0.05 || self.pill_vel_x.abs() > 0.05 {
                active = true;
            }

            // Hover springs for the 3 main elements
            for i in 0..3 {
                let [cx, cy, bw, bh] = specs[i];
                let in_box = px >= cx - bw * 0.5 && px <= cx + bw * 0.5 && py >= cy - bh * 0.5 && py <= cy + bh * 0.5;
                let target = if in_box { 1.0f32 } else { 0.0f32 };
                let force = -spring_k * (self.hover_pos[i] - target) - damping * self.hover_vel[i];
                self.hover_vel[i] += force * dt;
                self.hover_pos[i] += self.hover_vel[i] * dt;

                if (self.hover_pos[i] - target).abs() > 0.002 || self.hover_vel[i].abs() > 0.002 {
                    active = true;
                }
            }

            if self.cursor_velocity[0].abs() > 0.0001 || self.cursor_velocity[1].abs() > 0.0001 {
                active = true;
            }

            // Reactive liquid surface tension stretch
            let calc_stretch_bounds = |idx: usize| -> [f32; 4] {
                let [cx, cy, bw, bh] = specs[idx];
                let t = self.hover_pos[idx].clamp(0.0, 1.0);
                let scale = 1.0 + t * 0.025;
                let pull_x = (px - cx) * t * 0.035;
                let pull_y = (py - cy) * t * 0.035;
                let w = bw * scale;
                let h = bh * scale;
                [cx + pull_x - w * 0.5, cy + pull_y - h * 0.5, w, h]
            };

            // 1. Left Circular Camera Gallery Disc Button
            let b0 = calc_stretch_bounds(0);
            let t0 = self.hover_pos[0].clamp(0.0, 1.0);
            let left_lens = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Circle)
                .bounds(b0[0], b0[1], b0[2], b0[3])
                .curvature(0.06)
                .bevel(5.5)
                .refraction(0.22)
                .dispersion(0.010)
                .blur(0.0)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.55 + t0 * 0.15)
                .fresnel(0.35)
                .element_type(10.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // 2. Center Outer Glass Track Capsule (Serene clear glass)
            let b1 = calc_stretch_bounds(1);
            let t1 = self.hover_pos[1].clamp(0.0, 1.0);
            let center_track = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b1[0], b1[1], b1[2], b1[3])
                .curvature(0.04)
                .bevel(4.5)
                .refraction(0.18)
                .dispersion(0.008)
                .blur(0.0)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.48 + t1 * 0.12)
                .fresnel(0.30)
                .element_type(11.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // 3. Right Circular Switch Camera Button
            let b2 = calc_stretch_bounds(2);
            let t2 = self.hover_pos[2].clamp(0.0, 1.0);
            let right_button = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Circle)
                .bounds(b2[0], b2[1], b2[2], b2[3])
                .curvature(0.06)
                .bevel(5.5)
                .refraction(0.22)
                .dispersion(0.010)
                .blur(0.0)
                .tint(1.0, 1.0, 1.0, 0.0)
                .specular(0.55 + t2 * 0.15)
                .fresnel(0.35)
                .element_type(13.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // 4. Center Active Sliding Liquid Glass Pill (Flush nested layer)
            let pill_cx = (270.0 + self.pill_offset_x) / 540.0;
            let pill_w = 145.0 / 540.0;
            let pill_h = 56.0 / 84.0;
            let pull_pill_x = (px - pill_cx) * t1 * 0.025;
            let pull_pill_y = (py - 0.50) * t1 * 0.025;
            let active_pill = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(pill_cx + pull_pill_x - pill_w * 0.5, 0.50 + pull_pill_y - pill_h * 0.5, pill_w, pill_h)
                .curvature(0.05)
                .bevel(5.0)
                .refraction(0.24)
                .dispersion(0.012)
                .blur(0.0)
                .tint(1.0, 1.0, 1.0, 0.03)
                .specular(0.58)
                .fresnel(0.35)
                .element_type(12.0)
                .progress(self.selected_mode as f32)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // Fluid damping for pointer velocity
            self.cursor_velocity[0] *= 0.85;
            self.cursor_velocity[1] *= 0.85;

            let surfaces = vec![
                left_lens,
                center_track,
                right_button,
                active_pill,
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

    let (img, sx, sy, sw, sh) = if std::env::var("GLASS_REF_BG").is_ok() {
        if let Ok(ref_img) = image::open("assets/camera_toolbar_ref.png") {
            let rgba = ref_img.to_rgba8();
            let (iw, ih) = rgba.dimensions();
            (rgba, 100, 580, (iw - 100).min(1720), (ih - 580).min(280))
        } else {
            let default_img = image::open(&path).ok()?.to_rgba8();
            let (img_w, img_h) = default_img.dimensions();
            let screen_w = 1920.0f32;
            let screen_h = 1080.0f32;
            let sx = (x as f32 / screen_w * img_w as f32).round() as u32;
            let sy = (y as f32 / screen_h * img_h as f32).round() as u32;
            let sw = ((w as f32 / screen_w * img_w as f32).round() as u32).min(img_w.saturating_sub(sx));
            let sh = ((h as f32 / screen_h * img_h as f32).round() as u32).min(img_h.saturating_sub(sy));
            (default_img, sx, sy, sw, sh)
        }
    } else {
        let default_img = image::open(&path).ok()?.to_rgba8();
        let (img_w, img_h) = default_img.dimensions();
        let screen_w = 1920.0f32;
        let screen_h = 1080.0f32;
        let sx = (x as f32 / screen_w * img_w as f32).round() as u32;
        let sy = (y as f32 / screen_h * img_h as f32).round() as u32;
        let sw = ((w as f32 / screen_w * img_w as f32).round() as u32).min(img_w.saturating_sub(sx));
        let sh = ((h as f32 / screen_h * img_h as f32).round() as u32).min(img_h.saturating_sub(sy));
        (default_img, sx, sy, sw, sh)
    };

    if sw == 0 || sh == 0 {
        return None;
    }
    
    let sub = image::imageops::crop_imm(&img, sx, sy, sw, sh).to_image();
    let mut resized = image::imageops::resize(&sub, w, h, image::imageops::FilterType::Lanczos3);

    // Composite circular avatar thumbnail behind left disc (center: 68, 42; top-left: 40, 14; 56x56)
    let avatar_candidates = [
        "assets/avatar.png",
        "../../assets/avatar.png",
        "/home/zius/Projects/liquid-glass/assets/avatar.png",
    ];
    for p in &avatar_candidates {
        if let Ok(avatar_img) = image::open(p) {
            let avatar_rgba = avatar_img.resize_exact(56, 56, image::imageops::FilterType::Lanczos3).to_rgba8();
            image::imageops::overlay(&mut resized, &avatar_rgba, 40, 14);
            break;
        }
    }

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
        cursor_pos: if let Ok(s) = std::env::var("GLASS_CURSOR") {
            let parts: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if parts.len() == 2 { [parts[0], parts[1]] } else { [-10.0, -10.0] }
        } else {
            [-10.0, -10.0]
        },
        cursor_velocity: [0.0, 0.0],
        hover_pos: [0.0; 3],
        hover_vel: [0.0; 3],
        is_pressed: false,
        selected_mode: 1, // CINEMATIC
        pill_offset_x: 72.5, // Aligns flush with right half of track (liquidglassdesign.com)
        pill_vel_x: 0.0,
        
        width: 540,
        height: 84,
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
    if std::env::var("GLASS_CURSOR").is_ok() {
        for _ in 0..12 {
            app_data.draw();
        }
    } else {
        app_data.draw();
    }
    
    while !app_data.exit {
        event_queue.blocking_dispatch(&mut app_data).unwrap();
    }
}
