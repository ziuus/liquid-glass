use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use serde::{Deserialize, Serialize};
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

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub lastCompletedDate: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoState {
    #[serde(default)]
    pub lastResetDate: String,
    pub tasks: Vec<TodoItem>,
}

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

    pos_x: i32,
    pos_y: i32,
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,

    wallpaper_cache: Option<Arc<Vec<u8>>>,
    current_wallpaper_path: Option<String>,
    last_wallpaper_check: Instant,

    todos: TodoState,
    last_todo_check: Instant,
    last_todo_mtime: Option<std::time::SystemTime>,
    active_tab: u32,    // 0 = Tasks, 1 = Agenda
    active_filter: u32, // 0 = All, 1 = Active, 2 = Done
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
                    .expect("Failed to initialize WGPU GlassRenderer for glass-todo");

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

impl PointerHandler for AppData {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.is_hovered = true;
                }
                PointerEventKind::Leave { .. } => {
                    self.is_hovered = false;
                    self.cursor_pos = [-10.0, -10.0];
                    self.draw();
                }
                PointerEventKind::Motion { .. } => {
                    let old_x = self.cursor_pos[0];
                    let old_y = self.cursor_pos[1];
                    let new_x = event.position.0 as f32 / self.width as f32;
                    let new_y = event.position.1 as f32 / self.height as f32;

                    self.cursor_velocity[0] = (new_x - old_x) * 100.0;
                    self.cursor_velocity[1] = (new_y - old_y) * 100.0;
                    self.cursor_pos = [new_x, new_y];
                    self.draw();
                }
                PointerEventKind::Press { button: 0x110, .. } => {
                    // Left click button: test click hit targets
                    let px = event.position.0 as f32;
                    let py = event.position.1 as f32;
                    self.handle_click(px, py);
                }
                _ => {}
            }
        }
    }
}

impl AppData {
    fn handle_click(&mut self, px: f32, py: f32) {
        let half_w = self.width as f32 * 0.5;
        let half_h = self.height as f32 * 0.5;
        let local_x = px - half_w;
        let local_y = py - half_h;

        // Check task row checkboxes (y = -105 + i * 44, x = -135)
        for i in 0..6 {
            let row_y = -105.0 + (i as f32) * 44.0;
            // Checkbox hit box: x within [-150, -115], y within [row_y - 16, row_y + 16]
            if (-150.0..=-115.0).contains(&local_x)
                && (local_y - row_y).abs() <= 16.0
                && i < self.todos.tasks.len()
            {
                self.todos.tasks[i].done = !self.todos.tasks[i].done;
                self.save_todos();
                self.draw();
                return;
            }
        }

        // Check Tabs (y = -188, x = -76 or +76)
        if (local_y - (-188.0)).abs() <= 16.0 {
            if local_x <= 0.0 {
                self.active_tab = 0;
            } else {
                self.active_tab = 1;
            }
            self.draw();
            return;
        }

        // Check Filters (y = -148)
        if (local_y - (-148.0)).abs() <= 14.0 {
            if (-126.0..=-66.0).contains(&local_x) {
                self.active_filter = 0; // All
            } else if (-58.0..=10.0).contains(&local_x) {
                self.active_filter = 1; // Active
            } else if (18.0..=78.0).contains(&local_x) {
                self.active_filter = 2; // Done
            }
            self.draw();
        }
    }

    fn save_todos(&self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.local/state/todos.json", home);
        if let Ok(json) = serde_json::to_string_pretty(&self.todos) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn check_todos(&mut self) {
        if self.last_todo_check.elapsed().as_millis() < 500 {
            return;
        }
        self.last_todo_check = Instant::now();
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.local/state/todos.json", home);
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(mtime) = meta.modified() {
                if Some(mtime) != self.last_todo_mtime {
                    self.last_todo_mtime = Some(mtime);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(st) = serde_json::from_str::<TodoState>(&content) {
                            self.todos = st;
                        }
                    }
                }
            }
        }
    }

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

    fn draw(&mut self) -> bool {
        self.check_wallpaper();
        self.check_todos();

        if let Some(renderer) = &mut self.renderer {
            // Pack task completion bitmask into progress
            let mut bitmask = 0u32;
            for (idx, task) in self.todos.tasks.iter().enumerate().take(6) {
                if task.done {
                    bitmask |= 1 << idx;
                }
            }

            let deformation_val = (self.active_tab as f32) * 10.0 + (self.active_filter as f32);
            let accent = get_theme_accent();

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);
            let (vx, vy) = (self.cursor_velocity[0], self.cursor_velocity[1]);

            // Glass Surface using shader element_type 26.0 (Todo Widget)
            let todo_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 26.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.08)
                .bevel(10.0)
                .refraction(0.35)
                .dispersion(0.012)
                .blur(20.0)
                .tint(accent[0], accent[1], accent[2], accent[3])
                .roughness(-1.0) // Hover row
                .specular(1.0)
                .fresnel(0.65)
                .element_type(26.0)
                .progress(bitmask as f32)
                .deformation(deformation_val)
                .pointer_pos(px, py)
                .pointer_velocity(vx, vy);

            self.cursor_velocity[0] *= 0.85;
            self.cursor_velocity[1] *= 0.85;

            let surfaces = vec![todo_card];
            let _ = renderer.render(&surfaces);
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
    [0.0, 0.0, 0.0, 0.0]
}

fn get_active_wallpaper_path() -> Option<String> {
    let awww_cache = dirs_cache_awww();
    if let Some(path) = awww_cache {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

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

    // Exact placement matching quickshell-todo-widget: 360x455, right: 28, top: 56
    // 1920 - 360 - 28 = 1532
    let width = 360u32;
    let height = 455u32;
    let pos_x = 1532i32;
    let pos_y = 56i32;

    let (wallpaper_cache, current_wallpaper_path) = init_wallpaper_1080p();

    let home = std::env::var("HOME").unwrap_or_default();
    let todo_path = format!("{}/.local/state/todos.json", home);
    let todos = if let Ok(s) = std::fs::read_to_string(&todo_path) {
        serde_json::from_str::<TodoState>(&s).unwrap_or(TodoState {
            lastResetDate: String::new(),
            tasks: Vec::new(),
        })
    } else {
        TodoState {
            lastResetDate: String::new(),
            tasks: Vec::new(),
        }
    };

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

        pos_x,
        pos_y,
        width,
        height,
        first_configure: true,
        exit: false,

        wallpaper_cache,
        current_wallpaper_path,
        last_wallpaper_check: Instant::now(),

        todos,
        last_todo_check: Instant::now(),
        last_todo_mtime: None,
        active_tab: 0,
        active_filter: 0,
    };

    let surface = app_data.compositor_state.create_surface(&qh);
    let layer_surface = app_data.layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Bottom,
        Some("liquid-glass-todo"),
        None,
    );

    layer_surface.set_anchor(Anchor::TOP | Anchor::RIGHT);
    layer_surface.set_size(app_data.width, app_data.height);
    layer_surface.set_margin(app_data.pos_y, 28, 0, 0);
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

    // High efficiency event loop: sleeps in kernel poll, wakes instantly on mouse or timeout
    while !app_data.exit {
        let _ = event_queue.dispatch_pending(&mut app_data);
        let _ = conn.flush();

        // Check if external state changed every 500ms
        app_data.check_todos();
        app_data.check_wallpaper();

        let timeout_ms = if app_data.is_hovered { 16 } else { 500 };

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
