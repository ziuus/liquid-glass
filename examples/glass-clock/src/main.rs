use chrono::{Datelike, Local, Timelike};
use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material};
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
use std::os::fd::{AsFd, AsRawFd};
use std::sync::Arc;
use std::time::Instant;
use wayland_client::{
    protocol::{wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

struct AppData {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    seat_state: SeatState,

    layer_surface: Option<LayerSurface>,
    renderer: Option<GlassRenderer>,
    window_wrapper: Option<Arc<WaylandWindow>>,

    pointer: Option<wl_pointer::WlPointer>,
    cursor_pos: [f32; 2],
    cursor_velocity: [f32; 2],
    is_hovered: bool,
    is_dragging: bool,
    drag_start_pos: [i32; 2],
    drag_grab_offset: [i32; 2],

    pos_x: i32,
    pos_y: i32,
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,

    wallpaper_cache: Option<Arc<Vec<u8>>>,
    current_wallpaper_path: Option<String>,
    last_wallpaper_check: Instant,
    last_second: u32,
    last_minute: u32,
    battery_level: f32,
    last_battery_check: Instant,
}

struct WaylandWindow {
    surface: wl_surface::WlSurface,
    display: *mut std::ffi::c_void,
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

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_scale: i32,
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
        _output: wayland_client::protocol::wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wayland_client::protocol::wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wayland_client::protocol::wl_output::WlOutput,
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
        if configure.new_size.0 != 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 != 0 {
            self.height = configure.new_size.1;
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.width, self.height);
            if let Some(cache) = &self.wallpaper_cache {
                let crop = get_wallpaper_crop(
                    cache,
                    self.pos_x as u32,
                    self.pos_y as u32,
                    self.width,
                    self.height,
                );
                renderer.update_background(self.width, self.height, &crop);
            }
        } else if let Some(wrapper) = &self.window_wrapper {
            let mut r =
                pollster::block_on(GlassRenderer::new(wrapper.clone(), self.width, self.height))
                    .expect("Failed to initialize WGPU GlassRenderer for glass-clock");

            if let Some(cache) = &self.wallpaper_cache {
                let crop = get_wallpaper_crop(
                    cache,
                    self.pos_x as u32,
                    self.pos_y as u32,
                    self.width,
                    self.height,
                );
                r.update_background(self.width, self.height, &crop);
            }

            self.renderer = Some(r);
        }

        self.first_configure = false;
        self.draw();
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(ptr) = self.seat_state.get_pointer(qh, &seat) {
                self.pointer = Some(ptr);
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(ptr) = self.pointer.take() {
                ptr.release();
            }
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

fn get_hyprland_cursor_pos() -> Option<(i32, i32)> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let his = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/run/user/{}", uid)
    });
    let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, his);
    let mut stream = UnixStream::connect(sock_path).ok()?;
    stream.write_all(b"cursorpos").ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    let mut parts = resp.trim().split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    Some((x, y))
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
                PointerEventKind::Enter { .. } => {
                    had_event = true;
                    self.is_hovered = true;
                    self.cursor_pos = [
                        (event.position.0 as f32) / (self.width as f32),
                        (event.position.1 as f32) / (self.height as f32),
                    ];
                }
                PointerEventKind::Leave { .. } => {
                    had_event = true;
                    if !self.is_dragging {
                        self.is_hovered = false;
                        self.cursor_pos = [-10.0, -10.0];
                        self.cursor_velocity = [0.0, 0.0];
                    }
                }
                PointerEventKind::Motion { .. } => {
                    had_event = true;
                    let nx = (event.position.0 as f32) / (self.width as f32);
                    let ny = (event.position.1 as f32) / (self.height as f32);
                    self.cursor_velocity = [nx - self.cursor_pos[0], ny - self.cursor_pos[1]];
                    self.cursor_pos = [nx, ny];

                    if self.is_dragging {
                        let new_pos = if let Some((gx, gy)) = get_hyprland_cursor_pos() {
                            let tx =
                                (gx - self.drag_grab_offset[0]).clamp(0, 1920 - self.width as i32);
                            let ty =
                                (gy - self.drag_grab_offset[1]).clamp(0, 1080 - self.height as i32);
                            Some((tx, ty))
                        } else {
                            let dx = event.position.0 as i32 - self.drag_grab_offset[0];
                            let dy = event.position.1 as i32 - self.drag_grab_offset[1];
                            let tx = (self.pos_x + dx).clamp(0, 1920 - self.width as i32);
                            let ty = (self.pos_y + dy).clamp(0, 1080 - self.height as i32);
                            Some((tx, ty))
                        };

                        if let Some((nx, ny)) = new_pos {
                            if nx != self.pos_x || ny != self.pos_y {
                                self.pos_x = nx;
                                self.pos_y = ny;
                                if let Some(ls) = &self.layer_surface {
                                    ls.set_margin(self.pos_y, 0, 0, self.pos_x);
                                    ls.commit();
                                }
                                if let Some(w) = &self.window_wrapper {
                                    w.surface.commit();
                                }
                                if let (Some(cache), Some(renderer)) =
                                    (&self.wallpaper_cache, &mut self.renderer)
                                {
                                    let crop = get_wallpaper_crop(
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
                    }
                }
                PointerEventKind::Press { .. } => {
                    had_event = true;
                    self.is_dragging = true;
                    self.drag_start_pos = [self.pos_x, self.pos_y];
                    self.drag_grab_offset = [event.position.0 as i32, event.position.1 as i32];
                }
                PointerEventKind::Release { .. } => {
                    had_event = true;
                    self.is_dragging = false;
                }
                _ => {}
            }
        }
        if had_event {
            self.draw();
        }
    }
}

impl AppData {
    fn check_wallpaper(&mut self) {
        if self.last_wallpaper_check.elapsed().as_millis() < 500 {
            return;
        }
        self.last_wallpaper_check = Instant::now();

        let queried = get_active_wallpaper_path();
        if queried != self.current_wallpaper_path && queried.is_some() {
            log::info!("Wallpaper changed to {:?}", queried);
            if let Some(buf) = load_wallpaper_image(queried.as_deref()) {
                self.current_wallpaper_path = queried;
                self.wallpaper_cache = Some(buf.clone());
                if let Some(renderer) = &mut self.renderer {
                    let crop = get_wallpaper_crop(
                        &buf,
                        self.pos_x as u32,
                        self.pos_y as u32,
                        self.width,
                        self.height,
                    );
                    renderer.update_background(self.width, self.height, &crop);
                }
            }
        }
    }

    fn update_battery(&mut self) {
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

    fn draw(&mut self) -> bool {
        self.check_wallpaper();
        self.update_battery();

        if let Some(renderer) = &mut self.renderer {
            let now = Local::now();
            let hr = now.hour() as f32;
            let mn = now.minute() as f32;
            let sec = now.second() as f32;
            let wday = now.weekday().num_days_from_sunday() as f32; // 0 = Sun, 6 = Sat
            let month = now.month() as f32;
            let mday = now.day() as f32;

            // Pack time into progress: hr * 10000 + mn * 100 + sec
            let time_val = hr * 10000.0 + mn * 100.0 + sec;
            // Pack date into deformation: wday * 10000 + month * 100 + mday
            let date_val = wday * 10000.0 + month * 100.0 + mday;

            let accent = get_theme_accent();

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);
            let (vx, vy) = (self.cursor_velocity[0], self.cursor_velocity[1]);

            // Glass Surface using shader element_type 25.0 (Clock Face)
            let clock_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 26.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.02)
                .bevel(2.5)
                .refraction(0.15)
                .dispersion(0.0)
                .blur(36.0)
                .tint(accent[0], accent[1], accent[2], accent[3])
                .roughness(self.battery_level)
                .specular(1.0)
                .fresnel(0.35)
                .element_type(25.0)
                .progress(time_val)
                .deformation(date_val)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            self.cursor_velocity[0] *= 0.85;
            self.cursor_velocity[1] *= 0.85;

            let surfaces = vec![clock_card];
            let _ = renderer.render(&surfaces);

            self.last_second = now.second();
            self.last_minute = now.minute();
            true
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

fn get_theme_accent() -> [f32; 4] {
    let home = std::env::var("HOME").unwrap_or_default();
    let theme_path = format!("{}/.config/quickshell-todo-widget/theme.json", home);
    if let Ok(content) = std::fs::read_to_string(&theme_path) {
        for line in content.lines() {
            if line.contains("colPrimary") {
                if let Some(pos) = line.find('#') {
                    if line.len() >= pos + 7 {
                        let hex = &line[pos + 1..pos + 7];
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            u8::from_str_radix(&hex[0..2], 16),
                            u8::from_str_radix(&hex[2..4], 16),
                            u8::from_str_radix(&hex[4..6], 16),
                        ) {
                            return [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
                        }
                    }
                }
            }
        }
    }
    // "otherwise just no color" (pure crystalline frosted glass)
    [0.0, 0.0, 0.0, 0.0]
}

fn get_active_wallpaper_path() -> Option<String> {
    // 1. Check awww monitor cache directly (0.0ms disk read)
    let awww_cache = dirs_cache_awww();
    if let Some(path) = awww_cache {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    // 2. Query awww command
    if let Ok(out) = std::process::Command::new("awww").arg("query").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if line.contains("image:") {
                if let Some(p) = line.split("image:").nth(1) {
                    let path = p.trim().to_string();
                    if std::path::Path::new(&path).exists() {
                        return Some(path);
                    }
                }
            }
        }
    }

    // 3. Current wallpaper cache
    let cache_path = format!(
        "{}/.cache/current_wallpaper",
        std::env::var("HOME").unwrap_or_default()
    );
    if let Ok(content) = std::fs::read_to_string(&cache_path) {
        let p = content.trim().to_string();
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }

    None
}

fn dirs_cache_awww() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let dir = format!("{}/.cache/awww/0.12.1/eDP-1", home);
    if let Ok(bytes) = std::fs::read(&dir) {
        let s = String::from_utf8_lossy(&bytes);
        for part in s.split(|c: char| c == '\0' || c.is_whitespace()) {
            let trimmed = part.trim();
            if trimmed.starts_with('/') && std::path::Path::new(trimmed).exists() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn load_wallpaper_image(path: Option<&str>) -> Option<Arc<Vec<u8>>> {
    let candidates = [
        path,
        Some("/home/zius/Pictures/wallpaper/surreal-cherry-5120x2880-26233.jpg"),
        Some("/home/zius/Pictures/wallpaper/scott-webb-hDyO6rr3kqk-unsplash.jpg"),
        Some("/home/zius/Pictures/wallpaper/Fantasy-Landscape3.png"),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Ok(img) = image::open(candidate) {
            log::info!("Loaded wallpaper: {}", candidate);
            let rgba = img.to_rgba8();
            let resized =
                image::imageops::resize(&rgba, 1920, 1080, image::imageops::FilterType::Triangle);
            return Some(Arc::new(resized.into_raw()));
        }
    }

    let mut fallback = vec![0u8; 1920 * 1080 * 4];
    for y in 0..1080 {
        for x in 0..1920 {
            let i = (y * 1920 + x) * 4;
            fallback[i] = 240;
            fallback[i + 1] = 242;
            fallback[i + 2] = 240;
            fallback[i + 3] = 255;
        }
    }
    Some(Arc::new(fallback))
}

fn init_wallpaper_1080p() -> (Option<Arc<Vec<u8>>>, Option<String>) {
    let path = get_active_wallpaper_path();
    let buf = load_wallpaper_image(path.as_deref());
    (buf, path)
}

fn get_wallpaper_crop(cache: &[u8], x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let mut crop = vec![0u8; (w * h * 4) as usize];
    for row in 0..h {
        let src_y = (y + row).min(1079);
        let src_x = x.min(1920u32.saturating_sub(w));
        let src_start = ((src_y * 1920 + src_x) * 4) as usize;
        let dst_start = (row * w * 4) as usize;
        let len = (w * 4) as usize;
        crop[dst_start..dst_start + len].copy_from_slice(&cache[src_start..src_start + len]);
    }
    crop
}

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland display");
    let display = conn.display();
    let display_ptr = display.id().as_ptr() as *mut std::ffi::c_void;

    let (globals, mut event_queue) =
        wayland_client::globals::registry_queue_init::<AppData>(&conn).unwrap();
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let shm = Shm::bind(&globals, &qh).expect("wl_shm missing");
    let seat_state = SeatState::new(&globals, &qh);

    // Exact placement matching quickshell-clock-widget: 320x218, x: 28, y: 56
    let width = 320u32;
    let height = 218u32;
    let pos_x = 28i32;
    let pos_y = 56i32;

    let (wallpaper_cache, current_wallpaper_path) = init_wallpaper_1080p();

    let mut app_data = AppData {
        registry_state,
        output_state: OutputState::new(&globals, &qh),
        compositor_state: CompositorState::bind(&globals, &qh).expect("wl_compositor missing"),
        layer_shell: LayerShell::bind(&globals, &qh).expect("wlr layer shell missing"),
        shm,
        seat_state,

        layer_surface: None,
        renderer: None,
        window_wrapper: None,

        pointer: None,
        cursor_pos: [-10.0, -10.0],
        cursor_velocity: [0.0, 0.0],
        is_hovered: false,
        is_dragging: false,
        drag_start_pos: [pos_x, pos_y],
        drag_grab_offset: [0, 0],

        pos_x,
        pos_y,
        width,
        height,
        first_configure: true,
        exit: false,

        wallpaper_cache,
        current_wallpaper_path,
        last_wallpaper_check: Instant::now(),
        last_second: 0,
        last_minute: 0,
        battery_level: 100.0,
        last_battery_check: Instant::now() - std::time::Duration::from_secs(60),
    };

    let surface = app_data.compositor_state.create_surface(&qh);

    let layer_surface = app_data.layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Bottom,
        Some("glass_clock"),
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

    while app_data.first_configure {
        event_queue.blocking_dispatch(&mut app_data).unwrap();
    }

    if app_data.pointer.is_none() {
        for seat in app_data.seat_state.seats() {
            if let Ok(ptr) = app_data.seat_state.get_pointer(&qh, &seat) {
                app_data.pointer = Some(ptr);
                break;
            }
        }
    }

    // High efficiency event loop:
    // Wakes on mouse events with zero latency (<0.1ms).
    // When idle, sleeps in kernel poll until the exact millisecond of the next second boundary.
    // 0.0% idle CPU, guaranteed instant ticking.
    let mut last_sec_tick = Local::now().second();

    while !app_data.exit {
        let _ = event_queue.dispatch_pending(&mut app_data);
        let _ = conn.flush();

        let now = Local::now();
        let cur_sec = now.second();
        if app_data.is_dragging || app_data.is_hovered || cur_sec != last_sec_tick {
            last_sec_tick = cur_sec;
            app_data.draw();
            let _ = conn.flush();
        }

        let now_after = Local::now();
        let sub_ms = now_after.timestamp_subsec_millis();
        let ms_to_next_sec = (1000u32.saturating_sub(sub_ms)).clamp(10, 1000);

        let timeout_ms = if app_data.is_hovered || app_data.is_dragging {
            16
        } else {
            ms_to_next_sec as i32
        };

        let guard = match conn.prepare_read() {
            Some(g) => g,
            None => {
                let _ = event_queue.dispatch_pending(&mut app_data);
                continue;
            }
        };

        let fd = conn.as_fd().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_res = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if poll_res > 0 && (pfd.revents & libc::POLLIN) != 0 {
            let _ = guard.read();
        } else {
            drop(guard);
        }

        let _ = event_queue.dispatch_pending(&mut app_data);
    }
}
