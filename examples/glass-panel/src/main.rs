use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material};
use glass_platform::WallpaperManager;
mod capture;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::delegate_shm;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
};
use std::sync::Arc;
use wayland_client::{
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

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
    is_pressed: bool,

    // Media Playback State
    is_playing: bool,
    track_progress: f32,
    elapsed_sec: f32,
    total_duration_sec: f32,
    current_track_idx: usize,

    hover_play_pos: f32,
    hover_play_vel: f32,

    pos_x: i32,
    pos_y: i32,
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,
    frame_counter: u32,
    animating: bool,
    wallpaper_mgr: WallpaperManager,
    // Cached wl_output for screencopy (set on first new_output callback)
    primary_output: Option<wl_output::WlOutput>,
    // Frame counter for throttled screencopy (every N frames)
    #[allow(dead_code)]
    screencopy_ticker: u32,
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
            let ptr = self.display.cast();
            std::ptr::NonNull::new(ptr).unwrap()
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
    ) {
    }
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.primary_output.is_none() {
            self.primary_output = Some(output);
        }
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 420;
            self.height = 110;
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
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
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
        let mut had_event = false;
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    had_event = true;
                    if self.width > 0 && self.height > 0 {
                        let nx = (event.position.0 as f32 / self.width as f32).clamp(0.0, 1.0);
                        let ny = (event.position.1 as f32 / self.height as f32).clamp(0.0, 1.0);
                        self.cursor_pos = [nx, ny];
                    }
                }
                PointerEventKind::Leave { .. } => {
                    had_event = true;
                    self.cursor_pos = [-10.0, -10.0];
                    self.is_pressed = false;
                }
                PointerEventKind::Press { .. } => {
                    had_event = true;
                    self.is_pressed = true;
                    let sx = event.position.0 as i32;
                    let sy = event.position.1 as i32;

                    let cx = self.width as f32 * 0.5;
                    let cy = self.height as f32 * 0.5;
                    let lx = sx as f32 - cx;
                    let ly = sy as f32 - cy;

                    // Hit Test: Hero Play/Pause Button (center at x = 48.0, y = -10.0, r = 24.0)
                    let d_play = ((lx - 48.0).powi(2) + (ly - (-10.0)).powi(2)).sqrt();
                    // Hit Test: Next Button (center at x = 92.0, y = -10.0, r = 18.0)
                    let d_next = ((lx - 92.0).powi(2) + (ly - (-10.0)).powi(2)).sqrt();
                    // Hit Test: Prev Button (center at x = 4.0, y = -10.0, r = 18.0)
                    let d_prev = ((lx - 4.0).powi(2) + (ly - (-10.0)).powi(2)).sqrt();
                    // Hit Test: Vinyl Album Disc (center at x = -135.0, y = -4.0, r = 30.0)
                    let d_vinyl = ((lx - (-135.0)).powi(2) + (ly - (-4.0)).powi(2)).sqrt();
                    // Hit Test: Timeline Scrubber (y around 22.0, x from -20.0 to 140.0)
                    let on_scrubber = (12.0..=32.0).contains(&ly) && (-20.0..=140.0).contains(&lx);

                    if d_play <= 24.0 {
                        self.is_playing = !self.is_playing;
                        std::process::Command::new("playerctl")
                            .arg("play-pause")
                            .spawn()
                            .ok();
                    } else if d_next <= 18.0 || d_vinyl <= 30.0 {
                        self.current_track_idx = (self.current_track_idx + 1) % 4;
                        self.elapsed_sec = 0.0;
                        self.track_progress = 0.0;
                        std::process::Command::new("playerctl")
                            .arg("next")
                            .spawn()
                            .ok();
                    } else if d_prev <= 18.0 {
                        if self.elapsed_sec > 3.0 {
                            self.elapsed_sec = 0.0;
                            self.track_progress = 0.0;
                        } else {
                            self.current_track_idx = if self.current_track_idx == 0 {
                                3
                            } else {
                                self.current_track_idx - 1
                            };
                            self.elapsed_sec = 0.0;
                            self.track_progress = 0.0;
                        }
                        std::process::Command::new("playerctl")
                            .arg("previous")
                            .spawn()
                            .ok();
                    } else if on_scrubber {
                        self.track_progress = ((lx - (-15.0)) / 150.0).clamp(0.0, 1.0);
                        self.elapsed_sec = self.track_progress * self.total_duration_sec;
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

impl AppData {
    fn check_wallpaper(&mut self) {
        if self.wallpaper_mgr.check_update() {
            if let (Some(cache), Some(renderer)) =
                (self.wallpaper_mgr.cached_buffer(), &mut self.renderer)
            {
                let crop = WallpaperManager::get_wallpaper_crop(
                    cache,
                    self.pos_x as u32,
                    self.pos_y as u32,
                    self.width,
                    self.height,
                );
                renderer.update_background(self.width, self.height, &crop);
            }
        }
    }

    fn draw(&mut self) -> bool {
        self.frame_counter += 1;
        self.check_wallpaper();

        if let Some(renderer) = &mut self.renderer {
            let px = self.cursor_pos[0];
            let py = self.cursor_pos[1];

            let dt = 1.0f32 / 60.0f32;

            // Advance track playback when active
            if self.is_playing {
                self.elapsed_sec += dt;
                if self.elapsed_sec >= self.total_duration_sec {
                    self.elapsed_sec = 0.0;
                    self.current_track_idx = (self.current_track_idx + 1) % 4;
                }
                self.track_progress = (self.elapsed_sec / self.total_duration_sec).clamp(0.0, 1.0);
            }

            // Liquid Glass Island Dimensions (390px x 94px inside 420x110 surface)
            let spec: [f32; 4] = [0.50, 0.50, 390.0 / 420.0, 94.0 / 110.0];

            let spring_k = 260.0f32;
            let damping = 18.0f32;

            // Hover Spring for Play Button
            let cx = self.width as f32 * 0.5;
            let cy = self.height as f32 * 0.5;
            let lx = px * self.width as f32 - cx;
            let ly = py * self.height as f32 - cy;
            let d_btn = ((lx - 48.0).powi(2) + (ly - (-10.0)).powi(2)).sqrt();
            let target_play = if d_btn <= 24.0 { 1.0f32 } else { 0.0f32 };

            let force =
                -spring_k * (self.hover_play_pos - target_play) - damping * self.hover_play_vel;
            self.hover_play_vel += force * dt;
            self.hover_play_pos += self.hover_play_vel * dt;

            let in_box = px >= spec[0] - spec[2] * 0.5
                && px <= spec[0] + spec[2] * 0.5
                && py >= spec[1] - spec[3] * 0.5
                && py <= spec[1] + spec[3] * 0.5;

            // Active rendering while playing music, dragging, cursor moving, or spring settling
            let spring_moving = (self.hover_play_pos - target_play).abs() > 0.003
                || self.hover_play_vel.abs() > 0.003;
            let active = self.is_playing || in_box || spring_moving;

            let b0 = [
                spec[0] - spec[2] * 0.5,
                spec[1] - spec[3] * 0.5,
                spec[2],
                spec[3],
            ];
            let play_flag = if self.is_playing { 1.0f32 } else { 0.0f32 };

            // Liquid Glass Media Control Island (capsule height = 94px, bezel proportional at 12px, crisp blur = 8px)
            let island = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(b0[0], b0[1], b0[2], b0[3])
                .curvature(0.05)
                .bevel(12.0)
                .refraction(0.55)
                .dispersion(0.02)
                .blur(8.0)
                .tint(
                    play_flag,
                    if self.is_pressed { 1.0 } else { 0.0 },
                    self.hover_play_pos,
                    0.0,
                )
                .specular(0.80)
                .fresnel(0.55)
                .element_type(35.0)
                .pointer_pos(px, py);

            let surfaces = vec![island];
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

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();
    let display = conn.display();
    let display_ptr = display.id().as_ptr() as *mut std::ffi::c_void;

    let (globals, mut event_queue) =
        wayland_client::globals::registry_queue_init::<AppData>(&conn).unwrap();
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let shm = Shm::bind(&globals, &qh).expect("wl_shm missing");
    let screencopy = capture::ScreencopyState::new(&registry_state, &qh, &shm);
    let seat_state = SeatState::new(&globals, &qh);

    // Initial center position on 1920x1080: (1920 - 420) / 2 = 750, (1080 - 110) / 2 = 485
    let width = 420u32;
    let height = 110u32;
    let pos_x = (1920i32.saturating_sub(width as i32)) / 2;
    let pos_y = (1080i32.saturating_sub(height as i32)) / 2;

    let wallpaper_mgr = WallpaperManager::new();

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
        is_pressed: false,

        is_playing: true,
        track_progress: 0.38,
        elapsed_sec: 84.0,         // 1:24
        total_duration_sec: 225.0, // 3:45
        current_track_idx: 0,

        hover_play_pos: 0.0,
        hover_play_vel: 0.0,

        pos_x,
        pos_y,
        width,
        height,
        first_configure: true,
        exit: false,
        frame_counter: 0,
        animating: true,
        wallpaper_mgr,
        primary_output: None,
        screencopy_ticker: 0,
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
    layer_surface.set_margin(app_data.pos_y, 0, 0, app_data.pos_x);
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
                app_data.pointer = Some(ptr);
                break;
            }
        }
    }

    // Initialize WGPU renderer
    let mut renderer = pollster::block_on(GlassRenderer::new(
        window_wrapper,
        app_data.width,
        app_data.height,
    ))
    .expect("Failed to init WGPU");

    // Upload initial background crop from pre-cached 1080p wallpaper buffer (sub-millisecond)
    if let Some(cache) = app_data.wallpaper_mgr.cached_buffer() {
        let crop = WallpaperManager::get_wallpaper_crop(
            cache,
            app_data.pos_x as u32,
            app_data.pos_y as u32,
            app_data.width,
            app_data.height,
        );
        renderer.update_background(app_data.width, app_data.height, &crop);
    }

    app_data.renderer = Some(renderer);

    app_data.draw();

    let target_frame_time = std::time::Duration::from_millis(16);
    let mut last_frame = std::time::Instant::now();

    while !app_data.exit {
        let _ = event_queue.dispatch_pending(&mut app_data);

        if app_data.is_playing || app_data.animating {
            let still_animating = app_data.draw();
            app_data.animating = still_animating;

            let elapsed = last_frame.elapsed();
            if elapsed < target_frame_time {
                std::thread::sleep(target_frame_time - elapsed);
            }
            last_frame = std::time::Instant::now();
        } else {
            // High-efficiency idle mode: sleep in kernel epoll_wait when paused and stationary
            let _ = event_queue.blocking_dispatch(&mut app_data);
            last_frame = std::time::Instant::now();
        }
    }
}
