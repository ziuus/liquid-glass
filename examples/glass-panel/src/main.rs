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
    hover_pos: [f32; 2], // [0]: Left dial, [1]: Right capsule
    hover_vel: [f32; 2],
    is_pressed: bool,
    is_12h: bool,
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
                    // Clicking the right capsule toggles between 12H and 24H mode
                    if self.cursor_pos[0] >= 0.22 && self.cursor_pos[0] <= 0.98 {
                        self.is_12h = !self.is_12h;
                    }
                }
                PointerEventKind::Release { .. } => {
                    had_event = true;
                    self.is_pressed = false;
                }
                _ => {}
            }
        }

        if had_event {
            self.animating = true;
        }
    }
}

fn get_battery_info() -> f32 {
    if let Ok(s) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
        if let Ok(val) = s.trim().parse::<f32>() {
            return val;
        }
    }
    if let Ok(s) = std::fs::read_to_string("/sys/class/power_supply/BAT1/capacity") {
        if let Ok(val) = s.trim().parse::<f32>() {
            return val;
        }
    }
    100.0
}

fn get_local_time() -> (f32, f32, f32, u32, u32) {
    let mut now_time: libc::time_t = 0;
    unsafe {
        libc::time(&mut now_time);
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now_time, &mut tm);
        
        // Subsecond microsecond precision for buttery 60 FPS sweeping second hand
        let mut tv: libc::timeval = std::mem::zeroed();
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
        let subsecond = (tv.tv_usec as f32) / 1_000_000.0;
        
        let hr = tm.tm_hour as f32;
        let min = tm.tm_min as f32;
        let sec = tm.tm_sec as f32 + subsecond;
        let wday = tm.tm_wday as u32; // 0 = Sunday
        let mday = tm.tm_mday as u32;
        (hr, min, sec, wday, mday)
    }
}

impl AppData {
    fn draw(&mut self) -> bool {
        self.frame_counter += 1;

        if let Some(renderer) = &mut self.renderer {
            let px = self.cursor_pos[0];
            let py = self.cursor_pos[1];
            let vx = self.cursor_velocity[0];
            let vy = self.cursor_velocity[1];

            let (hr, min, sec, wday, mday) = get_local_time();
            let bat_capacity = get_battery_info();

            // Refined Architectural Clock Widget Layout Specifications:
            // Total Window: 540 x 140
            // 0: Left Swiss Analog Dial: Circle diameter 120px (center x=78, y=70)
            // 1: Right Digital Clock + Battery Capsule: 380px x 120px (center x=334, y=70)
            let specs: [[f32; 4]; 2] = [
                [78.0 / 540.0, 0.50, 120.0 / 540.0, 120.0 / 140.0],
                [334.0 / 540.0, 0.50, 380.0 / 540.0, 120.0 / 140.0],
            ];

            let dt = 1.0f32 / 60.0f32;
            let spring_k = 260.0f32;
            let damping = 18.0f32;
            let mut active = true; // Continuous 60 FPS clock animation for sweeping second hand

            // Hover springs for the 2 main glass surfaces
            for i in 0..2 {
                let [cx, cy, bw, bh] = specs[i];
                let in_box = px >= cx - bw * 0.5 && px <= cx + bw * 0.5 && py >= cy - bh * 0.5 && py <= cy + bh * 0.5;
                let target = if in_box { 1.0f32 } else { 0.0f32 };
                let force = -spring_k * (self.hover_pos[i] - target) - damping * self.hover_vel[i];
                self.hover_vel[i] += force * dt;
                self.hover_pos[i] += self.hover_vel[i] * dt;
            }

            // Target seconds pill offset: anchored elegantly at the right end of the digital capsule
            let target_pill_x = 135.0f32;
            let pill_force = -spring_k * (self.pill_offset_x - target_pill_x) - damping * self.pill_vel_x;
            self.pill_vel_x += pill_force * dt;
            self.pill_offset_x += self.pill_vel_x * dt;

            // Rock-solid static widget positions (Zero displacement on hover)
            let b0 = [specs[0][0] - specs[0][2] * 0.5, specs[0][1] - specs[0][3] * 0.5, specs[0][2], specs[0][3]];
            let b1 = [specs[1][0] - specs[1][2] * 0.5, specs[1][1] - specs[1][3] * 0.5, specs[1][2], specs[1][3]];

            // Packed format: wday * 10000 + mday * 100 + bat_capacity
            let date_bat_code = (wday as f32 * 10000.0) + (mday as f32 * 100.0) + bat_capacity;
            let format_flag = if self.is_12h { 1.0f32 } else { 0.0f32 };

            // 1. Left Swiss Minimalist Precision Analog Dial (120px diameter)
            let analog_dial = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Circle)
                .bounds(b0[0], b0[1], b0[2], b0[3])
                .curvature(0.045)
                .bevel(5.0)
                .refraction(0.12)
                .dispersion(0.006)
                .blur(0.0)
                .tint(hr, min, sec, format_flag)
                .specular(0.55)
                .fresnel(0.32)
                .element_type(20.0)
                .progress(date_bat_code)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // 2. Right Digital Liquid Glass Clock + Battery Capsule
            let digital_capsule = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b1[0], b1[1], b1[2], b1[3])
                .curvature(0.04)
                .bevel(4.5)
                .refraction(0.12)
                .dispersion(0.006)
                .blur(0.0)
                .tint(hr, min, sec, format_flag)
                .specular(0.50)
                .fresnel(0.30)
                .element_type(21.0)
                .progress(date_bat_code)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // 3. Nested Seconds Liquid Glass Accent Pill (Radiant Gold #DCF842)
            // Positioned gracefully between the digital clock and the battery gauge
            let pill_cx = (334.0 + 38.0) / 540.0;
            let pill_cy = (70.0 - 24.0) / 140.0;
            let pill_w = 44.0 / 540.0;
            let pill_h = 24.0 / 140.0;
            let seconds_pill = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(pill_cx - pill_w * 0.5, pill_cy - pill_h * 0.5, pill_w, pill_h)
                .curvature(0.05)
                .bevel(3.5)
                .refraction(0.24)
                .dispersion(0.012)
                .blur(0.0)
                .tint(hr, min, sec, format_flag)
                .specular(0.60)
                .fresnel(0.35)
                .element_type(22.0)
                .progress(date_bat_code)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // Fluid damping for pointer velocity
            self.cursor_velocity[0] *= 0.85;
            self.cursor_velocity[1] *= 0.85;

            let surfaces = vec![
                analog_dial,
                digital_capsule,
                seconds_pill,
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
        cursor_pos: if let Ok(s) = std::env::var("GLASS_CURSOR") {
            let parts: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if parts.len() == 2 { [parts[0], parts[1]] } else { [-10.0, -10.0] }
        } else {
            [-10.0, -10.0]
        },
        cursor_velocity: [0.0, 0.0],
        hover_pos: [0.0; 2],
        hover_vel: [0.0; 2],
        is_pressed: false,
        is_12h: false,
        pill_offset_x: 108.0,
        pill_vel_x: 0.0,
        
        width: 540,
        height: 140,
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
        Layer::Top,
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
    
    app_data.draw();
    
    let target_frame_time = std::time::Duration::from_millis(16);
    let mut last_frame = std::time::Instant::now();

    while !app_data.exit {
        let _ = event_queue.dispatch_pending(&mut app_data);

        if app_data.animating {
            let still_animating = app_data.draw();
            app_data.animating = still_animating;

            let elapsed = last_frame.elapsed();
            if elapsed < target_frame_time {
                std::thread::sleep(target_frame_time - elapsed);
            }
            last_frame = std::time::Instant::now();
        } else {
            let _ = event_queue.blocking_dispatch(&mut app_data);
            last_frame = std::time::Instant::now();
        }
    }
}
