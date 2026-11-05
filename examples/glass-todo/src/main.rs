use glass_core::font::{FontWeight, QuicksandFont, UIFont};
use glass_core::{geometry::Shape, GlassRenderer, GlassSurface, Material, Spring, SpringParams};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use serde::{Deserialize, Serialize};
use smithay_client_toolkit::delegate_shm;
use smithay_client_toolkit::seat::keyboard::Keysym;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat,
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
    protocol::{wl_keyboard, wl_pointer, wl_seat, wl_surface},
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

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub dateDisplay: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub isToday: bool,
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
    keyboard: Option<wl_keyboard::WlKeyboard>,
    cursor_pos: [f32; 2],
    is_hovered: bool,
    is_dragging: bool,
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
    is_wallpaper_light: bool,

    todos: TodoState,
    last_todo_check: Instant,
    last_todo_mtime: Option<std::time::SystemTime>,
    events: Vec<CalendarEvent>,
    last_calendar_check: Instant,
    last_calendar_mtime: Option<std::time::SystemTime>,
    active_tab: u32,    // 0 = Tasks, 1 = Agenda
    active_filter: u32, // 0 = All, 1 = Active, 2 = Done

    // Inline Editing
    is_editing: bool,
    input_buffer: String,
    edit_start_time: Instant,

    // Slide-to-Delete Interaction
    slide_offsets: [f32; 6],
    task_pressed_row: Option<usize>,
    press_start_pos: [f32; 2],
    is_sliding: bool,

    // Smooth Spring Animation States
    font: QuicksandFont,
    tab_spring: Spring,
    filter_spring: Spring,
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

impl KeyboardHandler for AppData {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
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
        if !self.is_editing {
            return;
        }

        match event.keysym {
            Keysym::Return => {
                let text = self.input_buffer.trim().to_string();
                if !text.is_empty() {
                    self.todos.tasks.push(TodoItem {
                        text,
                        done: false,
                        r#type: "todo".to_string(),
                        lastCompletedDate: String::new(),
                    });
                    self.save_todos();
                }
                self.is_editing = false;
                self.input_buffer.clear();
                if let Some(ls) = &self.layer_surface {
                    ls.set_keyboard_interactivity(KeyboardInteractivity::None);
                    ls.commit();
                }
                if let Some(w) = &self.window_wrapper {
                    w.surface.commit();
                }
                self.draw();
            }
            Keysym::Escape => {
                self.is_editing = false;
                self.input_buffer.clear();
                if let Some(ls) = &self.layer_surface {
                    ls.set_keyboard_interactivity(KeyboardInteractivity::None);
                    ls.commit();
                }
                if let Some(w) = &self.window_wrapper {
                    w.surface.commit();
                }
                self.draw();
            }
            Keysym::BackSpace => {
                self.input_buffer.pop();
                self.draw();
            }
            _ => {
                if let Some(chars) = event.utf8 {
                    for ch in chars.chars() {
                        if !ch.is_control() && self.input_buffer.len() < 36 {
                            self.input_buffer.push(ch);
                        }
                    }
                    self.draw();
                }
            }
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
                }
                PointerEventKind::Leave { .. } => {
                    had_event = true;
                    if !self.is_dragging {
                        self.is_hovered = false;
                        self.cursor_pos = [-10.0, -10.0];
                    }
                }
                PointerEventKind::Motion { .. } => {
                    had_event = true;
                    let px = event.position.0 as f32;
                    let py = event.position.1 as f32;
                    let new_x = px / self.width as f32;
                    let new_y = py / self.height as f32;

                    self.cursor_pos = [new_x, new_y];

                    if let Some(row) = self.task_pressed_row {
                        let dx = px - self.press_start_pos[0];
                        if dx < -6.0 || self.is_sliding {
                            self.is_sliding = true;
                            self.slide_offsets[row] = dx.clamp(-140.0, 0.0);
                        }
                    } else if self.is_dragging {
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
                                    self.is_wallpaper_light = is_buffer_light(&crop);
                                    renderer.update_background(self.width, self.height, &crop);
                                }
                            }
                        }
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    had_event = true;
                    let px = event.position.0 as f32;
                    let py = event.position.1 as f32;
                    let is_right = button == 0x111; // BTN_RIGHT (273)

                    let half_w = self.width as f32 * 0.5;
                    let half_h = self.height as f32 * 0.5;
                    let local_x = px - half_w;
                    let local_y = py - half_h;

                    // If active_tab == 0, check if touching a task row
                    let mut touched_row = None;
                    if self.active_tab == 0 && !is_right {
                        let visible_indices = self.filtered_indices();
                        for (display_row, _) in visible_indices.iter().enumerate().take(6) {
                            let row_y = -105.0 + (display_row as f32) * 44.0;
                            if (-156.0..=156.0).contains(&local_x)
                                && (local_y - row_y).abs() <= 18.0
                            {
                                touched_row = Some(display_row);
                                break;
                            }
                        }
                    }

                    if let Some(r) = touched_row {
                        self.task_pressed_row = Some(r);
                        self.press_start_pos = [px, py];
                        self.is_sliding = false;
                    } else {
                        let handled = self.handle_click(px, py, is_right);
                        if !handled {
                            self.is_dragging = true;
                            self.drag_grab_offset = [px as i32, py as i32];
                        }
                    }
                }
                PointerEventKind::Release { .. } => {
                    had_event = true;
                    if self.is_sliding {
                        if let Some(row) = self.task_pressed_row.take() {
                            let off = self.slide_offsets[row];
                            let visible = self.filtered_indices();
                            if off < -75.0 && row < visible.len() {
                                let actual_idx = visible[row];
                                if actual_idx < self.todos.tasks.len() {
                                    self.todos.tasks.remove(actual_idx);
                                    self.save_todos();
                                }
                            }
                            self.is_sliding = false;
                        }
                    } else if let Some(row) = self.task_pressed_row.take() {
                        // Regular tap / click -> toggle task done
                        let visible = self.filtered_indices();
                        if row < visible.len() {
                            let actual_idx = visible[row];
                            if actual_idx < self.todos.tasks.len() {
                                self.todos.tasks[actual_idx].done =
                                    !self.todos.tasks[actual_idx].done;
                                self.save_todos();
                            }
                        }
                    }
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
    fn filtered_indices(&self) -> Vec<usize> {
        self.todos
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| match self.active_filter {
                1 => !t.done,
                2 => t.done,
                _ => true,
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_click(&mut self, px: f32, py: f32, is_right: bool) -> bool {
        let half_w = self.width as f32 * 0.5;
        let half_h = self.height as f32 * 0.5;
        let local_x = px - half_w;
        let local_y = py - half_h;

        // Check Top Tabs (tab_y = -195.0, cy = 32.5)
        if (local_y - (-195.0)).abs() <= 22.0 {
            if self.is_editing {
                self.is_editing = false;
                self.input_buffer.clear();
                if let Some(ls) = &self.layer_surface {
                    ls.set_keyboard_interactivity(KeyboardInteractivity::None);
                    ls.commit();
                }
            }
            if local_x <= 0.0 {
                self.active_tab = 0;
                self.tab_spring.set_target(0.0);
            } else {
                self.active_tab = 1;
                self.tab_spring.set_target(1.0);
            }
            self.draw();
            return true;
        }

        // Check Filters (filt_y = -148.0, cy = 79.5)
        if self.active_tab == 0 && (local_y - (-148.0)).abs() <= 18.0 {
            if (local_x - 120.0).abs() <= 16.0 {
                // Refresh icon: reset daily tasks
                for t in &mut self.todos.tasks {
                    if t.r#type == "daily" {
                        t.done = false;
                    }
                }
                self.save_todos();
                self.draw();
                return true;
            } else if local_x < -55.0 {
                self.active_filter = 0; // All
                self.filter_spring.set_target(0.0);
            } else if local_x < 15.0 {
                self.active_filter = 1; // Active
                self.filter_spring.set_target(1.0);
            } else {
                self.active_filter = 2; // Done
                self.filter_spring.set_target(2.0);
            }
            self.draw();
            return true;
        }

        // Check task row right-click to delete (row_y = -105.0 + i * 44.0)
        if self.active_tab == 0 && is_right {
            let visible_indices = self.filtered_indices();
            for (display_row, &actual_idx) in visible_indices.iter().enumerate().take(6) {
                let row_y = -105.0 + (display_row as f32) * 44.0;
                if (-156.0..=156.0).contains(&local_x)
                    && (local_y - row_y).abs() <= 18.0
                    && actual_idx < self.todos.tasks.len()
                {
                    self.todos.tasks.remove(actual_idx);
                    self.save_todos();
                    self.draw();
                    return true;
                }
            }
        }

        // Check Add task button in Tasks view (bot_y = 185.0, cy = 412.5)
        if self.active_tab == 0 && (local_y - 185.0).abs() <= 24.0 {
            self.is_editing = true;
            self.input_buffer.clear();
            self.edit_start_time = Instant::now();
            if let Some(ls) = &self.layer_surface {
                ls.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                ls.commit();
            }
            if let Some(w) = &self.window_wrapper {
                w.surface.commit();
            }
            self.draw();
            return true;
        }

        // Check Calendar sync button in Agenda view (bot_y = 185.0, cy = 412.5)
        if self.active_tab == 1 && (local_y - 185.0).abs() <= 24.0 {
            let home = std::env::var("HOME").unwrap_or_default();
            let sync_script = format!("{}/.config/quickshell-todo-widget/calendar_sync.py", home);
            if std::path::Path::new(&sync_script).exists() {
                let _ = std::process::Command::new("python3")
                    .arg(&sync_script)
                    .spawn();
            }
            self.last_calendar_check = Instant::now() - std::time::Duration::from_secs(10);
            self.check_calendar();
            self.draw();
            return true;
        }

        // If clicking outside while editing, commit or close
        if self.is_editing {
            let text = self.input_buffer.trim().to_string();
            if !text.is_empty() {
                self.todos.tasks.push(TodoItem {
                    text,
                    done: false,
                    r#type: "todo".to_string(),
                    lastCompletedDate: String::new(),
                });
                self.save_todos();
            }
            self.is_editing = false;
            self.input_buffer.clear();
            if let Some(ls) = &self.layer_surface {
                ls.set_keyboard_interactivity(KeyboardInteractivity::None);
                ls.commit();
            }
            if let Some(w) = &self.window_wrapper {
                w.surface.commit();
            }
            self.draw();
            return true;
        }

        false
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
                            self.draw();
                        }
                    }
                }
            }
        }
    }

    fn check_calendar(&mut self) {
        if self.last_calendar_check.elapsed().as_millis() < 500 {
            return;
        }
        self.last_calendar_check = Instant::now();
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.local/state/calendar_events.json", home);
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(mtime) = meta.modified() {
                if Some(mtime) != self.last_calendar_mtime {
                    self.last_calendar_mtime = Some(mtime);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(evs) = serde_json::from_str::<Vec<CalendarEvent>>(&content) {
                            self.events = evs;
                            self.draw();
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
                    self.is_wallpaper_light = is_buffer_light(&crop);
                    renderer.update_background(self.width, self.height, &crop);
                }
                self.draw();
            }
        }
    }

    fn update_physics(&mut self, dt: f32) -> bool {
        self.tab_spring.update(dt);
        self.filter_spring.update(dt);

        let mut slide_animating = false;
        if !self.is_sliding {
            for off in &mut self.slide_offsets {
                if *off != 0.0 {
                    *off += (0.0 - *off) * (20.0 * dt).min(1.0);
                    if off.abs() < 0.5 {
                        *off = 0.0;
                    } else {
                        slide_animating = true;
                    }
                }
            }
        }

        !self.tab_spring.is_at_rest()
            || !self.filter_spring.is_at_rest()
            || self.is_editing
            || slide_animating
            || self.is_sliding
    }

    fn render_ui_overlay(&self, buf: &mut [u8]) {
        // Apple macOS Sequoia Typography: Deep Obsidian Slate on Light Mode, Pure Crisp White on Dark Mode
        let (col_primary, col_secondary, col_dimmed, _col_accent, col_shadow) =
            if self.is_wallpaper_light {
                (
                    [8, 12, 20, 255],     // Primary text: Deep Obsidian Ink (WCAG AAA contrast 18:1)
                    [28, 36, 48, 250],    // Secondary text: Refined Slate
                    [45, 55, 70, 245], // Tertiary/Dimmed: Muted Slate (High contrast, clearly visible)
                    [6, 10, 16, 255],  // Accent text
                    [255, 255, 255, 110], // Subtle 1px ambient bottom lift
                )
            } else {
                (
                    [255, 255, 255, 255], // Primary text: Crisp Pure White
                    [225, 232, 245, 230], // Secondary text: Soft White
                    [170, 178, 195, 200], // Tertiary/Dimmed: Dimmed White
                    [255, 255, 255, 255], // White accent
                    [10, 14, 22, 140],    // Downward soft drop shadow (0, +1)
                )
            };

        // 1. Top Tab Labels: "Tasks ({count})" (cx = 104.0) & "Agenda" (cx = 256.0), cy = 32.5
        let tasks_count = self.todos.tasks.len();
        let tasks_label = format!("Tasks ({})", tasks_count);
        // Active selected pill has bright near-white fill with dark ink; unselected uses regionally-adapted dimmed text
        let selected_ink = [8, 12, 20, 255];

        let tab1_is_light = self
            .wallpaper_cache
            .as_ref()
            .map_or(self.is_wallpaper_light, |cache| {
                sample_region_is_light(cache, self.pos_x + 50, self.pos_y + 20, 108, 25)
            });
        let tab2_is_light = self
            .wallpaper_cache
            .as_ref()
            .map_or(self.is_wallpaper_light, |cache| {
                sample_region_is_light(cache, self.pos_x + 220, self.pos_y + 20, 72, 25)
            });

        let tab1_unselected = if tab1_is_light {
            [18, 24, 34, 255]
        } else {
            [170, 178, 195, 230]
        };
        let tab2_unselected = if tab2_is_light {
            [18, 24, 34, 255]
        } else {
            [170, 178, 195, 230]
        };

        let tab1_col = if self.active_tab == 0 {
            selected_ink
        } else {
            tab1_unselected
        };
        self.font.draw_text_centered(
            buf,
            self.width,
            self.height,
            &tasks_label,
            104.0,
            32.5,
            15.5,
            tab1_col,
            [0, 0, 0, 0],
            FontWeight::Bold,
        );

        let agenda_label = "Agenda";
        let tab2_col = if self.active_tab == 1 {
            selected_ink
        } else {
            tab2_unselected
        };
        self.font.draw_text_centered(
            buf,
            self.width,
            self.height,
            agenda_label,
            256.0,
            32.5,
            15.5,
            tab2_col,
            [0, 0, 0, 0],
            FontWeight::Bold,
        );

        if self.active_tab == 0 {
            // 2. Filter Bar Labels: "All" (cx = 84.0), "Active" (cx = 156.0), "Done" (cx = 228.0), cy = 79.5
            let f0_is_light = self
                .wallpaper_cache
                .as_ref()
                .map_or(self.is_wallpaper_light, |cache| {
                    sample_region_is_light(cache, self.pos_x + 60, self.pos_y + 68, 50, 22)
                });
            let f1_is_light = self
                .wallpaper_cache
                .as_ref()
                .map_or(self.is_wallpaper_light, |cache| {
                    sample_region_is_light(cache, self.pos_x + 130, self.pos_y + 68, 55, 22)
                });
            let f2_is_light = self
                .wallpaper_cache
                .as_ref()
                .map_or(self.is_wallpaper_light, |cache| {
                    sample_region_is_light(cache, self.pos_x + 205, self.pos_y + 68, 50, 22)
                });

            let f0_unselected = if f0_is_light {
                [18, 24, 34, 255]
            } else {
                [170, 178, 195, 230]
            };
            let f1_unselected = if f1_is_light {
                [18, 24, 34, 255]
            } else {
                [170, 178, 195, 230]
            };
            let f2_unselected = if f2_is_light {
                [18, 24, 34, 255]
            } else {
                [170, 178, 195, 230]
            };

            let f0_col = if self.active_filter == 0 {
                selected_ink
            } else {
                f0_unselected
            };
            let f1_col = if self.active_filter == 1 {
                selected_ink
            } else {
                f1_unselected
            };
            let f2_col = if self.active_filter == 2 {
                selected_ink
            } else {
                f2_unselected
            };

            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                "All",
                84.0,
                79.5,
                14.0,
                f0_col,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                "Active",
                156.0,
                79.5,
                14.0,
                f1_col,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                "Done",
                228.0,
                79.5,
                14.0,
                f2_col,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );

            // 3. Task Rows (cy = 122.5 + row_idx * 44.0)
            let visible_indices = self.filtered_indices();
            if visible_indices.is_empty() {
                let msg = match self.active_filter {
                    1 => "All tasks completed",
                    2 => "No completed tasks yet",
                    _ => "No tasks found",
                };
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    msg,
                    180.0,
                    200.0,
                    14.5,
                    col_dimmed,
                    col_shadow,
                    FontWeight::Medium,
                );
            }
            for (row_idx, &actual_idx) in visible_indices.iter().enumerate().take(6) {
                let cy = 122.5 + (row_idx as f32) * 44.0;
                let task = &self.todos.tasks[actual_idx];
                let slide_x = self.slide_offsets[row_idx];

                // Regional luminance adaptation: sample the actual text's bounding box width
                let text_w = (self.font.measure_text(&task.text, 15.5, FontWeight::Bold) as u32)
                    .clamp(40, 240);
                let (mean_lum, spread, _) = if let Some(cache) = &self.wallpaper_cache {
                    sample_region_stats(
                        cache,
                        self.pos_x + 68,
                        (self.pos_y as f32 + cy - 13.0).max(0.0) as i32,
                        text_w,
                        26,
                    )
                } else {
                    (0.5, 0.0, self.is_wallpaper_light)
                };

                // Strict 0.179 luminance threshold
                let row_is_light = mean_lum > 0.179;

                // If spread is high (row crosses diagonal edge), apply local scrim shadow
                let text_shadow_col = if spread > 0.05 {
                    if row_is_light {
                        [255, 255, 255, 120] // Light halo under dark text
                    } else {
                        [10, 14, 22, 180] // Dark scrim under light text
                    }
                } else {
                    [0, 0, 0, 0]
                };

                let (row_primary, row_dimmed) = if row_is_light {
                    ([8, 12, 20, 255], [45, 55, 70, 245])
                } else {
                    ([255, 255, 255, 255], [170, 178, 195, 200])
                };

                let (text_col, weight) = if task.done {
                    (row_dimmed, FontWeight::Regular)
                } else {
                    (row_primary, FontWeight::Bold)
                };

                // Slide-to-Delete: Red delete reveal underneath and sliding card top
                if slide_x < -1.0 {
                    // Underneath: Red slide-to-delete background
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        180.0,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        [225, 45, 65, 235],
                    );
                    let label = if slide_x < -75.0 {
                        "Release to delete"
                    } else {
                        "Slide to delete"
                    };
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        label,
                        270.0,
                        cy,
                        11.5,
                        [255, 255, 255, 255],
                        [0, 0, 0, 0],
                        FontWeight::Bold,
                    );

                    // Top: Sliding glass card
                    let card_bg = if row_is_light {
                        [245, 248, 252, 240]
                    } else {
                        [24, 30, 42, 240]
                    };
                    let card_rim = if row_is_light {
                        [190, 200, 215, 200]
                    } else {
                        [75, 90, 120, 200]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        180.0 + slide_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        card_bg,
                    );
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        180.0 + slide_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        1.0,
                        card_rim,
                    );
                }

                // Precision anti-aliased vector checkbox
                let chk_cx = 44.0 + slide_x;
                if task.done {
                    let bg_box = if row_is_light {
                        [10, 14, 22, 255]
                    } else {
                        [255, 255, 255, 245]
                    };
                    let fg_chk = if row_is_light {
                        [255, 255, 255, 255]
                    } else {
                        [10, 14, 22, 255]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        self.width,
                        self.height,
                        chk_cx,
                        cy,
                        9.0,
                        9.0,
                        5.0,
                        bg_box,
                    );
                    UIFont::draw_checkmark(
                        buf,
                        self.width,
                        self.height,
                        chk_cx,
                        cy,
                        1.1,
                        1.6,
                        fg_chk,
                    );
                } else {
                    let stroke_box = if row_is_light {
                        [10, 14, 22, 230]
                    } else {
                        [255, 255, 255, 230]
                    };
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        self.width,
                        self.height,
                        chk_cx,
                        cy,
                        9.0,
                        9.0,
                        5.0,
                        1.5,
                        stroke_box,
                    );
                }

                // Truncate task text if too long for display
                let mut display_text = task.text.clone();
                if display_text.len() > 24 {
                    display_text = format!("{}...", &display_text[..22]);
                }

                let text_x = 68.0 + slide_x;
                let end_x = self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    &display_text,
                    text_x,
                    cy,
                    15.5,
                    text_col,
                    text_shadow_col,
                    weight,
                );

                if task.done {
                    // Strikethrough
                    QuicksandFont::draw_strikethrough(
                        buf,
                        self.width,
                        self.height,
                        57.0 + slide_x,
                        end_x + 2.0,
                        cy,
                        1.5,
                        col_dimmed,
                    );
                }
            }

            // 4. Bottom Add Task Bar (cy = 412.5)
            let add_is_light = self
                .wallpaper_cache
                .as_ref()
                .map_or(self.is_wallpaper_light, |cache| {
                    sample_region_is_light(cache, self.pos_x + 42, self.pos_y + 400, 200, 26)
                });
            let (add_primary, add_dimmed, add_secondary) = if add_is_light {
                ([8, 12, 20, 255], [45, 55, 70, 245], [18, 24, 34, 250])
            } else {
                (
                    [255, 255, 255, 255],
                    [170, 178, 195, 200],
                    [225, 232, 245, 230],
                )
            };

            if self.is_editing {
                let input_str = if self.input_buffer.is_empty() {
                    "Type new task..."
                } else {
                    &self.input_buffer
                };
                let input_col = if self.input_buffer.is_empty() {
                    add_dimmed
                } else {
                    add_primary
                };

                let end_x = self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    input_str,
                    42.0,
                    412.5,
                    15.5,
                    input_col,
                    col_shadow,
                    FontWeight::SemiBold,
                );

                // Blinking cursor
                let elapsed_ms = self.edit_start_time.elapsed().as_millis();
                if (elapsed_ms / 500).is_multiple_of(2) {
                    let cursor_x = if self.input_buffer.is_empty() {
                        42.0
                    } else {
                        end_x + 2.0
                    };
                    QuicksandFont::draw_cursor(
                        buf,
                        self.width,
                        self.height,
                        cursor_x,
                        412.5 - 9.0,
                        18.0,
                        2.0,
                        add_primary,
                    );
                }

                // Key hint: "↵ Enter"
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    "↵ Enter",
                    295.0,
                    412.5,
                    12.0,
                    add_dimmed,
                    col_shadow,
                    FontWeight::Medium,
                );
            } else {
                self.font.draw_text_centered_y(
                    buf,
                    self.width,
                    self.height,
                    "Add task",
                    42.0,
                    412.5,
                    15.5,
                    add_secondary,
                    col_shadow,
                    FontWeight::SemiBold,
                );

                // Dedicated '+' action button pill on right sharing input bar's ink
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    315.0,
                    412.5,
                    11.0,
                    11.0,
                    11.0,
                    if add_is_light {
                        [220, 226, 238, 140]
                    } else {
                        [50, 58, 75, 140]
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    "+",
                    315.0,
                    412.5,
                    16.0,
                    add_primary,
                    col_shadow,
                    FontWeight::Bold,
                );
            }
        } else {
            // Agenda View (Events from ~/.local/state/calendar_events.json, cy = 122.5 + idx * 44.0)
            if self.events.is_empty() {
                self.font.draw_text_centered(
                    buf,
                    self.width,
                    self.height,
                    "No upcoming events",
                    180.0,
                    200.0,
                    14.0,
                    col_dimmed,
                    col_shadow,
                    FontWeight::Medium,
                );
            } else {
                for (idx, item) in self.events.iter().enumerate().take(6) {
                    let cy = 122.5 + (idx as f32) * 44.0;

                    // Time (cx = 64.0)
                    let time_str = if !item.time.is_empty() {
                        item.time.as_str()
                    } else {
                        "ALL DAY"
                    };
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        time_str,
                        64.0,
                        cy,
                        11.0,
                        col_secondary,
                        col_shadow,
                        FontWeight::Medium,
                    );

                    // Title (starts at x = 108.0)
                    let mut title = item.title.clone();
                    if title.len() > 22 {
                        title = format!("{}...", &title[..20]);
                    }
                    self.font.draw_text_centered_y(
                        buf,
                        self.width,
                        self.height,
                        &title,
                        108.0,
                        cy,
                        13.5,
                        col_primary,
                        col_shadow,
                        FontWeight::SemiBold,
                    );

                    // Tag (cx = 304.0)
                    let tag_str = if item.isToday { "TODAY" } else { "EVENT" };
                    self.font.draw_text_centered(
                        buf,
                        self.width,
                        self.height,
                        tag_str,
                        304.0,
                        cy,
                        10.0,
                        col_dimmed,
                        col_shadow,
                        FontWeight::Bold,
                    );
                }
            }

            // Bottom Status Bar (y = +185.0, cy = 412.5): "CALENDAR • {N} EVENTS"
            let cal_status = format!("CALENDAR • {} EVENTS", self.events.len());
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                &cal_status,
                180.0,
                412.5,
                12.0,
                col_secondary,
                col_shadow,
                FontWeight::SemiBold,
            );
        }
    }

    fn draw(&mut self) -> bool {
        self.check_wallpaper();

        let (item_count, bitmask) = if self.active_tab == 0 {
            let visible_indices = self.filtered_indices();
            let mut bm = 0u32;
            for (display_row, &actual_idx) in visible_indices.iter().enumerate().take(6) {
                if actual_idx < self.todos.tasks.len() && self.todos.tasks[actual_idx].done {
                    bm |= 1 << display_row;
                }
            }
            (visible_indices.len().min(6) as u32, bm)
        } else {
            (self.events.len().min(6) as u32, 0u32)
        };

        // Pack item_count in upper 16 bits, bitmask in lower 16 bits
        let progress_packed = (((item_count & 0xFFFF) << 16) | (bitmask & 0xFFFF)) as f32;

        // Generate dynamic Quicksand font overlay
        let mut overlay_buf = vec![0u8; (self.width * self.height * 4) as usize];
        self.render_ui_overlay(&mut overlay_buf);

        if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &overlay_buf);

            let _accent = get_theme_accent();
            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);

            // Pass continuous spring-animated values to shader
            let tab_val = self.tab_spring.value;
            let filter_val = self.filter_spring.value;

            let is_light_f32 = if self.is_wallpaper_light { 1.0 } else { 0.0 };

            let todo_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 28.0 })
                .bounds(0.0, 0.0, 1.0, 1.0)
                .curvature(0.035)
                .bevel(22.0)
                .refraction(0.70)
                .dispersion(filter_val) // Continuous Filter spring
                .blur(5.0)
                .tint(0.0, progress_packed, 0.0, is_light_f32)
                .roughness(-1.0)
                .specular(1.45)
                .fresnel(0.80)
                .element_type(26.0)
                .deformation(tab_val) // Continuous Tab spring
                .pointer_pos(px, py);

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
delegate_keyboard!(AppData);

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn buffer_lum_stats(buf: &[u8]) -> (f32, f32, bool) {
    let mut total_lum = 0.0f64;
    let mut total_sq = 0.0f64;
    let mut count = 0usize;
    let step = 16 * 4;
    for i in (0..buf.len()).step_by(step) {
        if i + 2 < buf.len() {
            let r = buf[i] as f64 / 255.0;
            let g = buf[i + 1] as f64 / 255.0;
            let b = buf[i + 2] as f64 / 255.0;
            let to_lin = |c: f64| {
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            let l = 0.2126 * to_lin(r) + 0.7152 * to_lin(g) + 0.0722 * to_lin(b);
            total_lum += l;
            total_sq += l * l;
            count += 1;
        }
    }
    if count > 0 {
        let mean = total_lum / count as f64;
        let var = (total_sq / count as f64 - mean * mean).max(0.0);
        let spread = var.sqrt();
        (mean as f32, spread as f32, mean > 0.179)
    } else {
        (0.0, 0.0, false)
    }
}

fn sample_region_stats(cache: &[u8], gx: i32, gy: i32, gw: u32, gh: u32) -> (f32, f32, bool) {
    let crop = get_wallpaper_crop(cache, gx.max(0) as u32, gy.max(0) as u32, gw, gh);
    buffer_lum_stats(&crop)
}

fn is_buffer_light(buf: &[u8]) -> bool {
    buffer_lum_stats(buf).2
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
    let cache_dir = format!("{}/.cache/awww", home);
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for version_entry in entries.flatten() {
            if let Ok(mon_entries) = std::fs::read_dir(version_entry.path()) {
                for mon_entry in mon_entries.flatten() {
                    if let Ok(bytes) = std::fs::read(mon_entry.path()) {
                        let s = String::from_utf8_lossy(&bytes);
                        for part in s.split(|c: char| c == '\0' || c.is_whitespace()) {
                            let trimmed = part.trim();
                            if trimmed.starts_with('/') && std::path::Path::new(trimmed).exists() {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn load_wallpaper_image(path: Option<&str>) -> Option<Arc<Vec<u8>>> {
    let active_detected = get_active_wallpaper_path();
    let candidates = [
        path,
        active_detected.as_deref(),
        Some("/home/zius/Pictures/wallpaper/scott-webb-hDyO6rr3kqk-unsplash.jpg"),
        Some("/home/zius/Pictures/wallpaper/surreal-cherry-5120x2880-26233.jpg"),
        Some("/home/zius/Pictures/wallpaper/Fantasy-Landscape3.png"),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Ok(img) = image::open(candidate) {
            log::info!("Loaded wallpaper: {}", candidate);
            let rgba = img.to_rgba8();
            let (ow, oh) = (rgba.width() as f32, rgba.height() as f32);

            // Exactly match awww --resize crop --crop-gravity center
            let scale = (1920.0 / ow).max(1080.0 / oh);
            let sw = (ow * scale).round() as u32;
            let sh = (oh * scale).round() as u32;
            let scaled =
                image::imageops::resize(&rgba, sw, sh, image::imageops::FilterType::Triangle);
            let left = sw.saturating_sub(1920) / 2;
            let top = sh.saturating_sub(1080) / 2;
            let cropped = image::imageops::crop_imm(&scaled, left, top, 1920, 1080).to_image();
            return Some(Arc::new(cropped.into_raw()));
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

fn sample_region_is_light(cache: &[u8], gx: i32, gy: i32, gw: u32, gh: u32) -> bool {
    let crop = get_wallpaper_crop(cache, gx.max(0) as u32, gy.max(0) as u32, gw, gh);
    is_buffer_light(&crop)
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

    // Exact placement: 360x455, right: 28, top: 56
    let width = 360u32;
    let height = 455u32;
    let pos_x = 1532i32;
    let pos_y = 56i32;

    let (wallpaper_cache, current_wallpaper_path) = init_wallpaper_1080p();
    let is_light = if let Some(buf) = &wallpaper_cache {
        let crop = get_wallpaper_crop(buf, pos_x as u32, pos_y as u32, width, height);
        is_buffer_light(&crop)
    } else {
        false
    };

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

    let events = {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.local/state/calendar_events.json", home);
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str::<Vec<CalendarEvent>>(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let font = QuicksandFont::new();
    let spring_params = SpringParams::default_ui();

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
        keyboard: None,
        cursor_pos: [-10.0, -10.0],
        is_hovered: false,
        is_dragging: false,
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
        is_wallpaper_light: is_light,

        todos,
        last_todo_check: Instant::now(),
        last_todo_mtime: None,
        events,
        last_calendar_check: Instant::now(),
        last_calendar_mtime: None,
        active_tab: 0,
        active_filter: 0,

        is_editing: false,
        input_buffer: String::new(),
        edit_start_time: Instant::now(),

        slide_offsets: [0.0; 6],
        task_pressed_row: None,
        press_start_pos: [0.0, 0.0],
        is_sliding: false,

        font,
        tab_spring: Spring::with_params(0.0, spring_params),
        filter_spring: Spring::with_params(0.0, spring_params),
    };

    let surface = app_data.compositor_state.create_surface(&qh);
    let layer_surface = app_data.layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Bottom,
        Some("liquid-glass-todo"),
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

    if app_data.keyboard.is_none() {
        for seat in app_data.seat_state.seats() {
            if let Ok(kbd) = app_data.seat_state.get_keyboard(&qh, &seat, None) {
                app_data.keyboard = Some(kbd);
                break;
            }
        }
    }

    let mut last_frame = Instant::now();

    // High performance reactive event loop: 60fps during animation/typing, idle sleeping otherwise
    while !app_data.exit {
        let _ = event_queue.dispatch_pending(&mut app_data);
        let _ = conn.flush();

        app_data.check_todos();
        app_data.check_calendar();
        app_data.check_wallpaper();

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = now;

        let animating = app_data.update_physics(dt);

        if animating {
            app_data.draw();
            let _ = conn.flush();
        }

        let timeout_ms =
            if animating || app_data.is_hovered || app_data.is_dragging || app_data.is_editing {
                16
            } else {
                250
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
