use chrono::Timelike;
use glass_core::SharedGpu;
use glass_platform::wallpaper::WallpaperManager;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, registry_handlers,
};
use std::os::fd::{AsFd, AsRawFd};
use std::time::Instant;
use wayland_client::protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface};
use wayland_client::{Connection, Proxy, QueueHandle};

mod clock;
mod dock;
mod island;
mod todo;
use clock::ClockWidget;
use dock::DockWidget;
use island::{IslandMode, IslandWidget};
use todo::TodoWidget;

struct ShellApp {
    registry_state: RegistryState,
    output_state: OutputState,
    #[allow(dead_code)]
    compositor_state: CompositorState,
    #[allow(dead_code)]
    layer_shell: LayerShell,
    shm: Shm,
    seat_state: SeatState,

    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    gpu: SharedGpu,

    clock: ClockWidget,
    island: IslandWidget,
    todo: TodoWidget,
    dock: DockWidget,

    exit: bool,
    first_configure_count: u32,
    wallpaper_mgr: WallpaperManager,
}

impl CompositorHandler for ShellApp {
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

impl OutputHandler for ShellApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
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

impl LayerShellHandler for ShellApp {
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
        let sid = layer.wl_surface().id();
        if sid == self.clock.surface_id() {
            self.clock.configure(configure);
            if self.clock.renderer.is_none() {
                self.clock.init_gpu(&self.gpu);
                self.clock.apply_input_region(&self.compositor_state, _qh);
            }
            self.clock.draw();
            self.first_configure_count += 1;
        } else if sid == self.island.surface_id() {
            self.island.configure(configure);
            if self.island.renderer.is_none() {
                self.island.init_gpu(&self.gpu);
                self.island.apply_input_region(&self.compositor_state, _qh);
            }
            self.island.draw();
            self.first_configure_count += 1;
        } else if sid == self.todo.surface_id() {
            self.todo.configure(configure);
            if self.todo.renderer.is_none() {
                self.todo.init_gpu(&self.gpu);
                self.todo.apply_input_region(&self.compositor_state, _qh);
            }
            self.todo.draw();
            self.first_configure_count += 1;
        } else if sid == self.dock.surface_id() {
            self.dock.configure(configure);
            if self.dock.renderer.is_none() {
                self.dock.init_gpu(&self.gpu);
                self.dock.apply_input_region(&self.compositor_state, _qh);
            }
            self.dock.draw();
            self.first_configure_count += 1;
        }
    }
}

impl SeatHandler for ShellApp {
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
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            if let Ok(kbd) = self.seat_state.get_keyboard(qh, &seat, None) {
                self.keyboard = Some(kbd);
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
        if capability == Capability::Keyboard {
            if let Some(kbd) = self.keyboard.take() {
                kbd.release();
            }
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl KeyboardHandler for ShellApp {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if self.todo.on_keyboard_event(&event) {
            self.todo.draw();
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
    ) {
    }
}

impl PointerHandler for ShellApp {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let sid = event.surface.id();
            if sid == self.clock.surface_id() && self.clock.on_pointer_event(event) {
                if self.clock.needs_input_region_update {
                    self.clock.apply_input_region(&self.compositor_state, _qh);
                }
                self.clock.draw();
            } else if sid == self.island.surface_id() && self.island.on_pointer_event(event) {
                if self.island.needs_input_region_update {
                    self.island.apply_input_region(&self.compositor_state, _qh);
                }
                self.island.draw();
            } else if sid == self.todo.surface_id() && self.todo.on_pointer_event(event) {
                if self.todo.needs_input_region_update {
                    self.todo.apply_input_region(&self.compositor_state, _qh);
                }
                self.todo.draw();
            } else if sid == self.dock.surface_id() && self.dock.on_pointer_event(event) {
                if self.dock.needs_input_region_update {
                    self.dock.apply_input_region(&self.compositor_state, _qh);
                }
                self.dock.draw();
            }
        }
    }
}

impl ShmHandler for ShellApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(ShellApp);
delegate_output!(ShellApp);
delegate_layer!(ShellApp);
delegate_registry!(ShellApp);
delegate_shm!(ShellApp);
delegate_seat!(ShellApp);
delegate_keyboard!(ShellApp);
delegate_pointer!(ShellApp);

impl wayland_client::Dispatch<wayland_client::protocol::wl_region::WlRegion, ()> for ShellApp {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_region::WlRegion,
        _event: wayland_client::protocol::wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl ProvidesRegistryState for ShellApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn parse_theme_mode(val: &serde_json::Value) -> Option<u32> {
    if let Some(s) = val.get("theme").and_then(|v| v.as_str()) {
        match s.to_lowercase().as_str() {
            "dark" => return Some(1),
            "light" => return Some(2),
            "auto" => return Some(0),
            _ => {}
        }
    }
    if let Some(t) = val.get("theme_mode").and_then(|v| v.as_u64()) {
        return Some(t as u32);
    }
    None
}

impl ShellApp {
    pub fn reload_config(&mut self, conf_path: &str) {
        if let Ok(content) = std::fs::read_to_string(conf_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(t) = parse_theme_mode(&val) {
                    self.clock.theme_mode = t;
                    self.island.theme_mode = t;
                    self.todo.theme_mode = t;
                    self.dock.theme_mode = t;
                }
                if let Some(s) = val
                    .get("style_mode")
                    .or_else(|| val.get("style"))
                    .and_then(|v| v.as_f64())
                {
                    self.clock.style_mode = s as f32;
                    self.island.style_mode = s as f32;
                    self.todo.style_mode = s as f32;
                    self.dock.style_mode = s as f32;
                }
                if let Some(b) = val
                    .get("blur_radius")
                    .or_else(|| val.get("blur"))
                    .and_then(|v| v.as_f64())
                {
                    self.clock.blur_radius = b as f32;
                    self.island.blur_radius = b as f32;
                    self.todo.blur_radius = b as f32;
                    self.dock.blur_radius = b as f32;
                }

                if let Some(clk) = val.get("clock") {
                    if let Some(s) = clk.get("style_mode").and_then(|v| v.as_f64()) {
                        self.clock.style_mode = s as f32;
                    }
                    if let Some(t) = parse_theme_mode(clk) {
                        self.clock.theme_mode = t;
                    }
                    if let Some(b) = clk
                        .get("blur_radius")
                        .or_else(|| clk.get("blur"))
                        .and_then(|v| v.as_f64())
                    {
                        self.clock.blur_radius = b as f32;
                    }
                    if let Some(spd) = clk
                        .get("animation_speed")
                        .or_else(|| clk.get("speed"))
                        .and_then(|v| v.as_f64())
                    {
                        self.clock.anim_speed = spd as f32;
                    }
                    if let Some(dock) = clk
                        .get("edge_dock")
                        .or_else(|| clk.get("dock_to_edge"))
                        .and_then(|v| v.as_bool())
                    {
                        self.clock.is_docked = dock;
                    }
                    if let Some(hov) = clk.get("hover_to_expand").and_then(|v| v.as_bool()) {
                        self.clock.hover_to_expand = hov;
                    }
                }

                if let Some(td) = val.get("todo") {
                    if let Some(s) = td.get("style_mode").and_then(|v| v.as_f64()) {
                        self.todo.style_mode = s as f32;
                    }
                    if let Some(t) = parse_theme_mode(td) {
                        self.todo.theme_mode = t;
                    }
                    if let Some(b) = td
                        .get("blur_radius")
                        .or_else(|| td.get("blur"))
                        .and_then(|v| v.as_f64())
                    {
                        self.todo.blur_radius = b as f32;
                    }
                    if let Some(spd) = td
                        .get("animation_speed")
                        .or_else(|| td.get("speed"))
                        .and_then(|v| v.as_f64())
                    {
                        self.todo.anim_speed = spd as f32;
                    }
                    if let Some(dock) = td
                        .get("edge_dock")
                        .or_else(|| td.get("dock_to_edge"))
                        .and_then(|v| v.as_bool())
                    {
                        self.todo.is_docked = dock;
                    }
                    if let Some(hov) = td.get("hover_to_expand").and_then(|v| v.as_bool()) {
                        self.todo.hover_to_expand = hov;
                    }
                }

                if let Some(isl) = val.get("island") {
                    if let Some(s) = isl.get("style_mode").and_then(|v| v.as_f64()) {
                        self.island.style_mode = s as f32;
                    }
                    if let Some(t) = parse_theme_mode(isl) {
                        self.island.theme_mode = t;
                    }
                    if let Some(b) = isl
                        .get("blur_radius")
                        .or_else(|| isl.get("blur"))
                        .and_then(|v| v.as_f64())
                    {
                        self.island.blur_radius = b as f32;
                    }
                    if let Some(h) = isl.get("hover_to_expand").and_then(|v| v.as_bool()) {
                        self.island.hover_to_expand = h;
                    }
                }

                self.clock.draw();
                self.island.draw();
                self.todo.draw();
            }
        }
    }
}

fn main() {
    env_logger::init();
    log::info!("Starting unified liquid-shell (Phase 2 Shared GPU)...");

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland display");
    let display = conn.display();
    let display_ptr = display.id().as_ptr() as *mut std::ffi::c_void;

    let (globals, mut event_queue) =
        wayland_client::globals::registry_queue_init::<ShellApp>(&conn).unwrap();
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let output_state = OutputState::new(&globals, &qh);
    let compositor_state = CompositorState::bind(&globals, &qh).expect("wl_compositor missing");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr layer shell missing");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm missing");
    let seat_state = SeatState::new(&globals, &qh);

    let gpu =
        pollster::block_on(SharedGpu::new()).expect("Failed to initialize shared GPU context");
    let mut wallpaper_mgr = WallpaperManager::new();

    let mut pipe_fds = [0i32; 2];
    unsafe {
        libc::pipe(pipe_fds.as_mut_ptr());
        libc::fcntl(pipe_fds[0], libc::F_SETFL, libc::O_NONBLOCK);
    }
    let (wakeup_read_fd, wakeup_write_fd) = (pipe_fds[0], pipe_fds[1]);

    let mut clock = ClockWidget::new(&qh, &compositor_state, &layer_shell, display_ptr);
    let mut island = IslandWidget::new(
        &qh,
        &compositor_state,
        &layer_shell,
        display_ptr,
        Some(wakeup_write_fd),
    );
    let mut todo = TodoWidget::new(&qh, &compositor_state, &layer_shell, display_ptr);
    let mut dock = DockWidget::new(&qh, &compositor_state, &layer_shell, display_ptr);
    clock.style_mode = island.style_mode;
    clock.theme_mode = island.theme_mode;
    clock.blur_radius = island.blur_radius;
    todo.style_mode = island.style_mode;
    todo.theme_mode = island.theme_mode;
    todo.blur_radius = island.blur_radius;
    dock.style_mode = island.style_mode;
    dock.theme_mode = island.theme_mode;
    dock.blur_radius = island.blur_radius;

    let home = std::env::var("HOME").unwrap_or_default();
    let conf_path = format!("{}/.config/liquid-glass/config.json", home);

    if let Some(cache) = wallpaper_mgr.cached_buffer() {
        let clock_crop = WallpaperManager::get_wallpaper_crop(
            cache,
            clock.pos_x.max(0) as u32,
            clock.pos_y.max(0) as u32,
            clock.width,
            clock.height,
        );
        clock.update_wallpaper(clock_crop);

        let island_crop = WallpaperManager::get_wallpaper_crop(
            cache,
            island.pos_x.max(0) as u32,
            island.pos_y.max(0) as u32,
            island.width,
            island.height,
        );
        island.update_wallpaper(island_crop);

        let todo_crop = WallpaperManager::get_wallpaper_crop(
            cache,
            todo.pos_x.max(0) as u32,
            todo.pos_y.max(0) as u32,
            todo.width,
            todo.height,
        );
        todo.update_wallpaper(todo_crop);

        let dock_crop = WallpaperManager::get_wallpaper_crop(
            cache,
            dock.pos_x.max(0) as u32,
            dock.pos_y.max(0) as u32,
            dock.width,
            dock.height,
        );
        dock.update_wallpaper(dock_crop);
    }
    wallpaper_mgr.release_full_buffer();

    let mut app = ShellApp {
        registry_state,
        output_state,
        compositor_state,
        layer_shell,
        shm,
        seat_state,

        pointer: None,
        keyboard: None,
        gpu,
        clock,
        island,
        todo,
        dock,

        exit: false,
        first_configure_count: 0,
        wallpaper_mgr,
    };

    app.reload_config(&conf_path);

    while app.first_configure_count < 3 {
        event_queue.blocking_dispatch(&mut app).unwrap();
    }

    if app.pointer.is_none() {
        for seat in app.seat_state.seats() {
            if let Ok(ptr) = app.seat_state.get_pointer(&qh, &seat) {
                app.pointer = Some(ptr);
                break;
            }
        }
    }

    let mut last_minute = chrono::Local::now().minute();
    let mut last_frame = Instant::now();
    let mut last_config_check = Instant::now();
    let mut last_config_modified = std::fs::metadata(&conf_path)
        .and_then(|m| m.modified())
        .ok();

    while !app.exit {
        let _ = event_queue.dispatch_pending(&mut app);
        let _ = conn.flush();

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = now;

        if now.duration_since(last_config_check).as_millis() >= 300 {
            last_config_check = now;
            if let Ok(meta) = std::fs::metadata(&conf_path) {
                if let Ok(mod_time) = meta.modified() {
                    if last_config_modified != Some(mod_time) {
                        last_config_modified = Some(mod_time);
                        app.reload_config(&conf_path);
                    }
                }
            }
        }

        let (new_style, new_theme, new_blur, toggle_todo, toggle_clock, toggle_dock) =
            app.island.check_ipc();
        if toggle_todo {
            app.todo.toggle_expand();
            app.todo.draw();
        }
        if toggle_clock {
            app.clock.toggle_expand();
            app.clock.draw();
        }
        if toggle_dock {
            app.dock.toggle_expand();
            app.dock.draw();
        }
        if let Some(s) = new_style {
            app.clock.style_mode = s;
            app.todo.style_mode = s;
            app.dock.style_mode = s;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
            app.island.draw();
        }
        if let Some(t) = new_theme {
            app.clock.theme_mode = t;
            app.todo.theme_mode = t;
            app.dock.theme_mode = t;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
            app.island.draw();
        }
        if let Some(b) = new_blur {
            app.clock.blur_radius = b;
            app.todo.blur_radius = b;
            app.dock.blur_radius = b;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
            app.island.draw();
        }
        if app.clock.style_mode != app.island.style_mode {
            let s = app.island.style_mode;
            app.clock.style_mode = s;
            app.todo.style_mode = s;
            app.dock.style_mode = s;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
        }
        if app.clock.theme_mode != app.island.theme_mode {
            let t = app.island.theme_mode;
            app.clock.theme_mode = t;
            app.todo.theme_mode = t;
            app.dock.theme_mode = t;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
        }
        if app.clock.blur_radius != app.island.blur_radius {
            let b = app.island.blur_radius;
            app.clock.blur_radius = b;
            app.todo.blur_radius = b;
            app.dock.blur_radius = b;
            app.clock.draw();
            app.todo.draw();
            app.dock.draw();
        }
        app.island.check_music();
        app.island.check_window_overlap();
        app.todo.check_todos();
        app.todo.check_calendar();

        let island_active = app.island.update_physics(dt);
        let todo_active = app.todo.update_physics(dt);
        let clock_active = app.clock.update_physics(dt);
        let dock_active = app.dock.update_physics(dt);

        if app.island.needs_input_region_update {
            app.island.apply_input_region(&app.compositor_state, &qh);
        }
        if app.todo.needs_input_region_update {
            app.todo.apply_input_region(&app.compositor_state, &qh);
        }
        if app.clock.needs_input_region_update {
            app.clock.apply_input_region(&app.compositor_state, &qh);
        }
        if app.dock.needs_input_region_update {
            app.dock.apply_input_region(&app.compositor_state, &qh);
        }

        // Check wallpaper updates across all widgets
        if app.wallpaper_mgr.check_update() {
            if let Some(cache) = app.wallpaper_mgr.cached_buffer() {
                let c = WallpaperManager::get_wallpaper_crop(
                    cache,
                    app.clock.pos_x.max(0) as u32,
                    app.clock.pos_y.max(0) as u32,
                    app.clock.width,
                    app.clock.height,
                );
                app.clock.update_wallpaper(c);
                app.clock.draw();

                let i = WallpaperManager::get_wallpaper_crop(
                    cache,
                    app.island.pos_x.max(0) as u32,
                    app.island.pos_y.max(0) as u32,
                    app.island.width,
                    app.island.height,
                );
                app.island.update_wallpaper(i);
                app.island.draw();

                let t = WallpaperManager::get_wallpaper_crop(
                    cache,
                    app.todo.pos_x.max(0) as u32,
                    app.todo.pos_y.max(0) as u32,
                    app.todo.width,
                    app.todo.height,
                );
                app.todo.update_wallpaper(t);
                app.todo.draw();

                let d = WallpaperManager::get_wallpaper_crop(
                    cache,
                    app.dock.pos_x.max(0) as u32,
                    app.dock.pos_y.max(0) as u32,
                    app.dock.width,
                    app.dock.height,
                );
                app.dock.update_wallpaper(d);
                app.dock.draw();
            }
            app.wallpaper_mgr.release_full_buffer();
        }

        let now_local = chrono::Local::now();
        let cur_min = now_local.minute();
        if clock_active || app.clock.is_hovered || cur_min != last_minute {
            last_minute = cur_min;
            app.clock.draw();
            let _ = conn.flush();
        }

        if island_active || app.island.needs_redraw {
            app.island.draw();
            app.island.needs_redraw = false;
            let _ = conn.flush();
        }

        if todo_active {
            app.todo.draw();
            let _ = conn.flush();
        }

        if dock_active || app.dock.is_hovered {
            app.dock.draw();
            let _ = conn.flush();
        }

        let animating = island_active
            || todo_active
            || clock_active
            || dock_active
            || app.clock.is_hovered
            || app.island.is_hovered
            || app.todo.is_hovered
            || app.dock.is_hovered;

        let timeout_ms = if animating {
            16
        } else if !app.clock.is_compact
            || !app.todo.is_compact
            || !app.dock.is_compact
            || (app.island.mode != IslandMode::CompactPill && app.island.mode != IslandMode::Hidden)
        {
            50
        } else if app.island.mode == IslandMode::Hidden {
            250
        } else {
            let sec = now_local.second();
            let sub_ms = now_local.timestamp_subsec_millis();
            let ms_to_min = ((60 - sec) * 1000 - sub_ms).clamp(50, 60_000);
            (ms_to_min as i32).min(1000)
        };

        let guard = match conn.prepare_read() {
            Some(g) => g,
            None => {
                let _ = event_queue.dispatch_pending(&mut app);
                continue;
            }
        };

        let conn_fd = conn.as_fd().as_raw_fd();
        let mut pfds = [
            libc::pollfd {
                fd: conn_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wakeup_read_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let poll_res = unsafe { libc::poll(pfds.as_mut_ptr(), 2, timeout_ms) };
        if poll_res > 0 {
            if (pfds[0].revents & libc::POLLIN) != 0 {
                let _ = guard.read();
            } else {
                drop(guard);
            }
            if (pfds[1].revents & libc::POLLIN) != 0 {
                let mut buf = [0u8; 64];
                unsafe { libc::read(wakeup_read_fd, buf.as_mut_ptr() as *mut _, 64) };
            }
        } else {
            drop(guard);
        }

        let _ = event_queue.dispatch_pending(&mut app);
    }
}
