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
        // 1. Process any freshly captured background frame from screencopy
        if self.screencopy.done {
            if let Some(bg_data) = self.screencopy.data.take() {
                let cw = self.screencopy.width;
                let ch = self.screencopy.height;
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_background(cw, ch, &bg_data);
                }
            }
        }

        // 2. Schedule continuous background re-capture every 3 frames (~20 FPS desktop refresh)
        self.frame_counter += 1;
        if self.frame_counter % 3 == 0 && !self.screencopy.is_capturing {
            if let Some(qh) = &self.qh {
                if let Some(output) = self.output_state.outputs().next() {
                    self.screencopy.capture(&output, 40, 40, self.width as i32, self.height as i32, qh);
                }
            }
        }

        // 3. Render true stretched liquid glass
        if let Some(renderer) = &mut self.renderer {
            let panel = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 28.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.85) // True optical 3D convex dome
                .bevel(24.0)     // Smooth curved meniscus rim
                .refraction(1.85) // Strong optical stretching & barrel magnification
                .dispersion(0.04) // Subtle physically accurate chromatic separation
                .blur(22.0)      // Creamy 64-tap Vogel vibrancy blur
                .tint(1.0, 1.0, 1.0, 0.02) // Pure crystalline optical transmission
                .roughness(0.0)   // Silky glass polish
                .specular(1.2)   // Dual-light specular highlights
                .fresnel(0.9)    // Edge Fresnel rim luminescence
                .pointer_pos(self.cursor_pos[0], self.cursor_pos[1])
                .pointer_velocity(self.cursor_velocity[0], self.cursor_velocity[1]);
            
            // Fluid damping for pointer velocity
            self.cursor_velocity[0] *= 0.88;
            self.cursor_velocity[1] *= 0.88;
            
            let surfaces = vec![panel];
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
    
    // Capture background behind the panel
    if let Some(output) = app_data.output_state.outputs().next() {
        app_data.screencopy.capture(&output, 40, 40, app_data.width as i32, app_data.height as i32, &qh);
        
        // Pump events until capture is done
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
