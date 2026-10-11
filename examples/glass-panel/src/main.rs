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
    
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,
    frame_counter: u32,
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
        self.draw();
        surface.frame(qh, surface.clone());
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
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if let PointerEventKind::Motion { .. } = event.kind {
                if self.width > 0 && self.height > 0 {
                    let nx = event.position.0 as f32 / self.width as f32;
                    let ny = event.position.1 as f32 / self.height as f32;
                    self.cursor_velocity = [nx - self.cursor_pos[0], ny - self.cursor_pos[1]];
                    self.cursor_pos = [nx, ny];
                }
            }
        }
    }
}

impl AppData {
    fn draw(&mut self) {
        // Periodically check if wallpaper changed (every ~5 seconds at 60 FPS)
        self.frame_counter += 1;
        if self.frame_counter % 300 == 0 {
            if let Some((cw, ch, bg_data)) = load_wallpaper_region(40, 40, self.width, self.height) {
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_background(cw, ch, &bg_data);
                }
            }
        }

        // 3. Render true stretched liquid glass with nested subcomponents
        if let Some(renderer) = &mut self.renderer {
            let px = self.cursor_pos[0];
            let py = self.cursor_pos[1];
            let vx = self.cursor_velocity[0];
            let vy = self.cursor_velocity[1];

            // ── Surface 0: Base Liquid Glass Widget ──
            let panel = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 32.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.18)   // Edge highlight rim
                .bevel(36.0)       // Pill-bevel z-radius
                .refraction(0.72)  // Liquid refraction lensing
                .dispersion(0.04)  // Chromatic dispersion at perimeter
                .blur(5.0)         // Soft background vibrancy blur
                .tint(1.0, 1.0, 1.0, 0.0) // Transparent crystal
                .roughness(0.0)
                .specular(0.35)
                .fresnel(1.0)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 1: Top Status Pill (Dynamic Island) ──
            let header_pill = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::Capsule)
                .bounds(0.08, 0.06, 0.84, 0.08)
                .curvature(0.35)
                .bevel(10.0)
                .refraction(0.42)
                .dispersion(0.03)
                .blur(8.0)
                .tint(1.0, 1.0, 1.0, 0.09)
                .specular(0.75)
                .fresnel(0.85)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 2 & 3: Dual Metric Quick-Tiles ──
            let tile_left = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::RoundedRectangle { radius: 18.0 })
                .bounds(0.08, 0.17, 0.40, 0.16)
                .curvature(0.25)
                .bevel(12.0)
                .refraction(0.48)
                .dispersion(0.03)
                .blur(8.0)
                .tint(0.92, 0.96, 1.0, 0.07)
                .specular(0.6)
                .fresnel(0.75)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            let tile_right = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::RoundedRectangle { radius: 18.0 })
                .bounds(0.52, 0.17, 0.40, 0.16)
                .curvature(0.25)
                .bevel(12.0)
                .refraction(0.48)
                .dispersion(0.03)
                .blur(8.0)
                .tint(0.98, 0.94, 1.0, 0.07)
                .specular(0.6)
                .fresnel(0.75)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 4: Center Hero Feature Card ──
            let center_card = GlassSurface::new()
                .material(Material::FrostedGlass)
                .shape(Shape::RoundedRectangle { radius: 22.0 })
                .bounds(0.08, 0.36, 0.84, 0.28)
                .curvature(0.28)
                .bevel(16.0)
                .refraction(0.58)
                .dispersion(0.03)
                .blur(10.0)
                .tint(1.0, 1.0, 1.0, 0.06)
                .specular(0.55)
                .fresnel(0.8)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponents 5, 6, 7: Bottom Action Glass Buttons ──
            let btn_left = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::Capsule)
                .bounds(0.12, 0.68, 0.20, 0.12)
                .curvature(0.4)
                .bevel(10.0)
                .refraction(0.4)
                .dispersion(0.02)
                .blur(6.0)
                .tint(1.0, 1.0, 1.0, 0.10)
                .specular(0.85)
                .fresnel(0.75)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            let btn_center = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(0.38, 0.66, 0.24, 0.15)
                .curvature(0.5)
                .bevel(14.0)
                .refraction(0.65)
                .dispersion(0.04)
                .blur(8.0)
                .tint(1.0, 1.0, 1.0, 0.14)
                .specular(1.1)
                .fresnel(0.9)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            let btn_right = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::Capsule)
                .bounds(0.68, 0.68, 0.20, 0.12)
                .curvature(0.4)
                .bevel(10.0)
                .refraction(0.4)
                .dispersion(0.02)
                .blur(6.0)
                .tint(1.0, 1.0, 1.0, 0.10)
                .specular(0.85)
                .fresnel(0.75)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // ── Subcomponent 8: Bottom Slider Groove Track ──
            let slider_track = GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::Capsule)
                .bounds(0.12, 0.85, 0.76, 0.045)
                .curvature(0.2)
                .bevel(6.0)
                .refraction(0.32)
                .dispersion(0.02)
                .blur(4.0)
                .tint(1.0, 1.0, 1.0, 0.07)
                .specular(0.6)
                .fresnel(0.6)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            // Fluid damping for pointer velocity
            self.cursor_velocity[0] *= 0.88;
            self.cursor_velocity[1] *= 0.88;

            let surfaces = vec![
                panel,
                header_pill,
                tile_left,
                tile_right,
                center_card,
                btn_left,
                btn_center,
                btn_right,
                slider_track,
            ];
            let _ = renderer.render(&surfaces);
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
        cursor_pos: [0.5, 0.5],
        cursor_velocity: [0.0, 0.0],
        
        width: 360,
        height: 480,
        first_configure: true,
        exit: false,
        frame_counter: 0,
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
