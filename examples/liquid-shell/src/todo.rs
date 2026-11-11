use glass_core::font::{FontWeight, QuicksandFont, UIFont};
use glass_core::geometry::Shape;
use glass_core::{GlassRenderer, GlassSurface, Material, SharedGpu, Spring, SpringParams, Tokens};
use glass_platform::WallpaperManager;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use serde::{Deserialize, Serialize};
use smithay_client_toolkit::compositor::{CompositorState, SurfaceData};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
    LayerSurfaceData,
};
use smithay_client_toolkit::shell::WaylandSurface;
use std::sync::Arc;
use std::time::Instant;
use wayland_backend::client::ObjectId;
use wayland_client::protocol::{wl_region, wl_surface};
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

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

pub struct WaylandWindow {
    pub surface: wl_surface::WlSurface,
    pub display: *mut std::ffi::c_void,
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

pub const CARD_W: f32 = 360.0;
pub const CARD_H: f32 = 455.0;
pub const CARD_PAD: f32 = 24.0;
pub const COMPACT_W: f32 = 148.0;
pub const COMPACT_H: f32 = 36.0;

pub struct TodoWidget {
    #[allow(dead_code)]
    pub layer_surface: Option<LayerSurface>,
    pub surface: wl_surface::WlSurface,
    pub renderer: Option<GlassRenderer>,
    pub window_wrapper: Arc<WaylandWindow>,

    pub cursor_pos: [f32; 2],
    pub is_hovered: bool,

    pub pos_x: i32,
    pub pos_y: i32,
    pub width: u32,
    pub height: u32,

    pub wallpaper_crop: Option<Vec<u8>>,
    pub overlay_buffer: Vec<u8>,
    pub is_wallpaper_light: bool,

    pub todos: TodoState,
    pub last_todo_check: Instant,
    pub last_todo_mtime: Option<std::time::SystemTime>,
    pub events: Vec<CalendarEvent>,
    pub last_calendar_check: Instant,
    pub last_calendar_mtime: Option<std::time::SystemTime>,
    pub last_calendar_sync: Instant,
    pub active_tab: u32,
    pub active_filter: u32,

    pub is_editing: bool,
    pub input_buffer: String,
    pub edit_start_time: Instant,

    pub is_cal_editing: bool,
    pub cal_input_buffer: String,
    pub cal_edit_start_time: Instant,

    pub slide_offsets: [f32; 6],
    pub task_pressed_row: Option<usize>,
    pub press_start_pos: [f32; 2],
    pub is_sliding: bool,

    pub font: QuicksandFont,
    pub tab_spring: Spring,
    pub filter_spring: Spring,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    pub anim_speed: f32,

    // Liquid Edge Emergence
    pub is_docked: bool,
    pub is_compact: bool,
    pub hover_to_expand: bool,
    pub current_w: f32,
    pub target_w: f32,
    pub current_h: f32,
    pub target_h: f32,
    pub size_spring: Spring,
    pub fillet_radius: f32,
    pub last_hover_time: Instant,
    pub needs_input_region_update: bool,
}

impl TodoWidget {
    pub fn effective_is_light(&self) -> bool {
        match self.theme_mode {
            1 => false,                   // Forced Dark
            2 => true,                    // Forced Light
            _ => self.is_wallpaper_light, // Auto based on wallpaper
        }
    }
    pub fn new<D>(
        qh: &QueueHandle<D>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        display_ptr: *mut std::ffi::c_void,
    ) -> Self
    where
        D: Dispatch<wl_surface::WlSurface, SurfaceData>
            + Dispatch<ZwlrLayerSurfaceV1, LayerSurfaceData>
            + 'static,
    {
        let width = (CARD_W + CARD_PAD * 2.0) as u32; // 408
        let height = (CARD_H + CARD_PAD * 2.0) as u32; // 503
        let pos_x = 1920i32 - width as i32 - 24; // 1488 (top right corner)
        let pos_y = 0i32; // flush against top screen edge

        let surface = compositor.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Top,
            Some("liquid-glass-todo"),
            None,
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer_surface.set_size(width, height);
        layer_surface.set_margin(0, 24, 0, 0);
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer_surface.commit();

        let window_wrapper = Arc::new(WaylandWindow {
            surface: surface.clone(),
            display: display_ptr,
        });

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
            let path = format!("{}/.local/state/calendar_events.json", home);
            if let Ok(content) = std::fs::read_to_string(&path) {
                serde_json::from_str::<Vec<CalendarEvent>>(&content).unwrap_or_default()
            } else {
                Vec::new()
            }
        };

        let font = QuicksandFont::new();
        let spring_params = SpringParams::bouncy();

        Self {
            layer_surface: Some(layer_surface),
            surface,
            renderer: None,
            window_wrapper,

            cursor_pos: [-10.0, -10.0],
            is_hovered: false,

            pos_x,
            pos_y,
            width,
            height,

            wallpaper_crop: None,
            overlay_buffer: Vec::new(),
            is_wallpaper_light: false,

            todos,
            last_todo_check: Instant::now(),
            last_todo_mtime: None,
            events,
            last_calendar_check: Instant::now(),
            last_calendar_mtime: None,
            last_calendar_sync: Instant::now() - std::time::Duration::from_secs(900),
            active_tab: 0,
            active_filter: 0,

            is_editing: false,
            input_buffer: String::new(),
            edit_start_time: Instant::now(),

            is_cal_editing: false,
            cal_input_buffer: String::new(),
            cal_edit_start_time: Instant::now(),

            slide_offsets: [0.0; 6],
            task_pressed_row: None,
            press_start_pos: [0.0, 0.0],
            is_sliding: false,

            font,
            tab_spring: Spring::with_params(0.0, spring_params),
            filter_spring: Spring::with_params(0.0, spring_params),
            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 6.0,
            anim_speed: 1.0,

            is_docked: true,
            is_compact: true,
            hover_to_expand: true,
            current_w: COMPACT_W,
            target_w: COMPACT_W,
            current_h: COMPACT_H,
            target_h: COMPACT_H,
            size_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 220.0,
                    damping: 24.0,
                    mass: 1.0,
                    initial_velocity: 0.0,
                    epsilon: 0.001,
                },
            ),
            fillet_radius: 18.0,
            last_hover_time: Instant::now() - std::time::Duration::from_secs(60),
            needs_input_region_update: true,
        }
    }

    pub fn surface_id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn init_gpu(&mut self, gpu: &SharedGpu) {
        let mut renderer = gpu
            .create_renderer(self.window_wrapper.clone(), self.width, self.height)
            .expect("Failed to create GlassRenderer for todo");

        if let Some(crop) = &self.wallpaper_crop {
            renderer.update_background(self.width, self.height, crop);
        }
        self.renderer = Some(renderer);
    }

    pub fn configure(&mut self, configure: LayerSurfaceConfigure) {
        if configure.new_size.0 != 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 != 0 {
            self.height = configure.new_size.1;
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.width, self.height);
            if let Some(crop) = &self.wallpaper_crop {
                renderer.update_background(self.width, self.height, crop);
            }
        }
    }

    pub fn update_wallpaper(&mut self, crop: Vec<u8>) {
        self.is_wallpaper_light =
            WallpaperManager::is_buffer_light_hysteresis(&crop, self.is_wallpaper_light);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_background(self.width, self.height, &crop);
        }
        self.wallpaper_crop = Some(crop);
    }

    pub fn expand(&mut self) {
        if self.is_compact {
            self.is_compact = false;
            self.size_spring.set_target(1.0);
            self.last_hover_time = Instant::now();
            self.needs_input_region_update = true;
        }
    }

    pub fn collapse(&mut self) {
        if !self.is_compact {
            self.is_compact = true;
            self.size_spring.set_target(0.0);
            self.needs_input_region_update = true;
        }
    }

    #[allow(dead_code)]
    pub fn toggle_expand(&mut self) {
        if self.is_compact {
            self.expand();
        } else {
            self.collapse();
        }
    }

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        let flare = if self.is_docked && py <= self.fillet_radius {
            self.fillet_radius
        } else {
            0.0
        };
        let cx = (self.width as f32) * 0.5;
        let half_w = self.current_w * 0.5 + flare;
        px >= (cx - half_w) && px <= (cx + half_w) && py >= 0.0 && py <= (self.current_h + 6.0)
    }

    pub fn apply_input_region<D>(&mut self, compositor: &CompositorState, qh: &QueueHandle<D>)
    where
        D: Dispatch<wl_region::WlRegion, ()> + 'static,
    {
        self.needs_input_region_update = false;
        let region = compositor.wl_compositor().create_region(qh, ());
        let flare = if self.is_docked {
            self.fillet_radius as i32
        } else {
            0
        };
        let cx = self.width as i32 / 2;
        if self.is_compact && self.size_spring.value < 0.1 {
            let pill_w = COMPACT_W as i32 + flare * 2;
            let pill_h = (COMPACT_H + 6.0) as i32;
            let pill_x = cx - pill_w / 2;
            region.add(pill_x, 0, pill_w, pill_h);
        } else {
            let card_w = CARD_W as i32 + flare * 2;
            let card_h = (self.target_h + 10.0) as i32;
            let card_x = cx - card_w / 2;
            region.add(card_x, 0, card_w, card_h);
        }
        self.surface.set_input_region(Some(&region));
        region.destroy();
        self.surface.commit();
    }

    pub fn on_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event.kind {
            PointerEventKind::Enter { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                let inside = self.is_inside(px, py);
                self.is_hovered = inside;
                if inside {
                    self.last_hover_time = Instant::now();
                    if self.is_compact && self.hover_to_expand {
                        self.expand();
                    }
                }
                true
            }
            PointerEventKind::Leave { .. } => {
                self.is_hovered = false;
                self.cursor_pos = [-10.0, -10.0];
                self.last_hover_time = Instant::now();
                true
            }
            PointerEventKind::Motion { .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                let new_x = px / self.width as f32;
                let new_y = py / self.height as f32;
                self.cursor_pos = [new_x, new_y];
                self.last_hover_time = Instant::now();

                let inside = self.is_inside(px, py);
                self.is_hovered = inside;
                if inside && self.is_compact && self.hover_to_expand {
                    self.expand();
                }

                if let Some(row) = self.task_pressed_row {
                    let cx = (self.width as f32) * 0.5;
                    let lx = px - (cx - CARD_W * 0.5);
                    let dx = lx - self.press_start_pos[0];
                    if dx < -25.0 || self.is_sliding {
                        self.is_sliding = true;
                        self.slide_offsets[row] = dx.clamp(-140.0, 0.0);
                    }
                }
                true
            }
            PointerEventKind::Press { button, .. } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                if !self.is_inside(px, py) {
                    return false;
                }
                if self.is_compact {
                    self.expand();
                    return true;
                }
                self.last_hover_time = Instant::now();

                let cx = (self.width as f32) * 0.5;
                let lx = px - (cx - CARD_W * 0.5);
                let ly = py;
                let is_right = button == 0x111;

                if lx < 0.0 || lx > CARD_W || ly < 0.0 || ly > self.get_card_height() {
                    return false;
                }

                let half_w = CARD_W * 0.5;
                let half_h = CARD_H * 0.5;
                let local_x = lx - half_w;
                let local_y = ly - half_h;

                let mut touched_row = None;
                if self.active_tab == 0 && !is_right {
                    let visible_indices = self.filtered_indices();
                    for (display_row, _) in visible_indices.iter().enumerate().take(6) {
                        let row_y = -105.0 + (display_row as f32) * 44.0;
                        if (-156.0..=156.0).contains(&local_x) && (local_y - row_y).abs() <= 18.0 {
                            touched_row = Some(display_row);
                            break;
                        }
                    }
                }

                if let Some(r) = touched_row {
                    self.task_pressed_row = Some(r);
                    self.press_start_pos = [lx, ly];
                    self.is_sliding = false;
                } else {
                    self.handle_click(lx, ly, is_right);
                }
                true
            }
            PointerEventKind::Release { .. } => {
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
                            self.slide_offsets.fill(0.0);
                        }
                        self.is_sliding = false;
                    }
                } else if let Some(row) = self.task_pressed_row.take() {
                    let visible = self.filtered_indices();
                    if row < visible.len() {
                        let actual_idx = visible[row];
                        if actual_idx < self.todos.tasks.len() {
                            self.todos.tasks[actual_idx].done = !self.todos.tasks[actual_idx].done;
                            self.save_todos();
                        }
                    }
                }
                true
            }
            PointerEventKind::Axis {
                horizontal,
                vertical,
                ..
            } => {
                let px = event.position.0 as f32;
                let py = event.position.1 as f32;
                let cx = (self.width as f32) * 0.5;
                let lx = px - (cx - CARD_W * 0.5);
                let ly = py;
                if lx < 0.0 || lx > CARD_W || ly < 0.0 || ly > self.get_card_height() {
                    return false;
                }

                let delta = if vertical.discrete != 0 {
                    vertical.discrete as f64
                } else if horizontal.discrete != 0 {
                    horizontal.discrete as f64
                } else if vertical.absolute.abs() > 0.5 {
                    vertical.absolute
                } else if horizontal.absolute.abs() > 0.5 {
                    horizontal.absolute
                } else {
                    0.0
                };

                if delta.abs() > 0.1 {
                    if ly < 55.0 {
                        // Top tabs: scroll switches between Tasks and Agenda
                        if delta > 0.0 && self.active_tab == 0 {
                            self.active_tab = 1;
                            self.tab_spring.set_target(1.0);
                            return true;
                        } else if delta < 0.0 && self.active_tab == 1 {
                            self.active_tab = 0;
                            self.tab_spring.set_target(0.0);
                            return true;
                        }
                    } else if self.active_tab == 0 {
                        // Filter pills: scroll cycles All -> Active -> Done
                        if delta > 0.0 {
                            self.active_filter = (self.active_filter + 1) % 3;
                        } else {
                            self.active_filter = if self.active_filter == 0 {
                                2
                            } else {
                                self.active_filter - 1
                            };
                        }
                        self.filter_spring.set_target(self.active_filter as f32);
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn on_keyboard_event(&mut self, event: &KeyEvent) -> bool {
        if self.is_cal_editing {
            match event.keysym {
                Keysym::Return => {
                    let url = self.cal_input_buffer.trim().to_string();
                    if !url.is_empty() {
                        let home = std::env::var("HOME").unwrap_or_default();
                        let cfg_path = format!("{}/.local/state/calendar_config.json", home);
                        let json = serde_json::json!({ "ics_url": url });
                        let _ = std::fs::write(&cfg_path, json.to_string());
                    }
                    self.is_cal_editing = false;
                    self.cal_input_buffer.clear();
                    true
                }
                Keysym::Escape => {
                    self.is_cal_editing = false;
                    self.cal_input_buffer.clear();
                    true
                }
                Keysym::BackSpace => {
                    self.cal_input_buffer.pop();
                    true
                }
                _ => {
                    if let Some(txt) = &event.utf8 {
                        for ch in txt.chars() {
                            if !ch.is_control() {
                                self.cal_input_buffer.push(ch);
                            }
                        }
                        true
                    } else {
                        false
                    }
                }
            }
        } else if self.is_editing {
            match event.keysym {
                Keysym::Return => {
                    let text = self.input_buffer.trim().to_string();
                    if !text.is_empty() {
                        self.todos.tasks.push(TodoItem {
                            text,
                            done: false,
                            r#type: "general".to_string(),
                            lastCompletedDate: String::new(),
                        });
                        self.save_todos();
                    }
                    self.is_editing = false;
                    self.input_buffer.clear();
                    true
                }
                Keysym::Escape => {
                    self.is_editing = false;
                    self.input_buffer.clear();
                    true
                }
                Keysym::BackSpace => {
                    self.input_buffer.pop();
                    true
                }
                _ => {
                    if let Some(txt) = &event.utf8 {
                        for ch in txt.chars() {
                            if !ch.is_control() {
                                self.input_buffer.push(ch);
                            }
                        }
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            false
        }
    }
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

    fn get_tasks_height(&self) -> f32 {
        let count = self.filtered_indices().len().min(6);
        if count == 0 {
            230.0
        } else {
            (122.5 + (count as f32) * 44.0 + 34.0).clamp(220.0, 455.0)
        }
    }

    fn get_agenda_height(&self) -> f32 {
        let count = self.events.len().min(6);
        if count == 0 {
            250.0
        } else {
            (122.5 + (count as f32) * 44.0 + 34.0).clamp(220.0, 455.0)
        }
    }

    fn get_card_height(&self) -> f32 {
        let h_tasks = self.get_tasks_height();
        let h_agenda = self.get_agenda_height();
        let t = self.tab_spring.value.clamp(0.0, 1.0);
        h_tasks * (1.0 - t) + h_agenda * t
    }

    fn get_tasks_add_bar_cy(&self) -> f32 {
        let count = self.filtered_indices().len().min(6);
        if count == 0 {
            175.0
        } else {
            122.5 + (count as f32) * 44.0
        }
    }

    fn get_agenda_add_bar_cy(&self) -> f32 {
        let count = self.events.len().min(6);
        if count == 0 {
            210.0
        } else {
            122.5 + (count as f32) * 44.0
        }
    }

    fn get_add_bar_cy(&self) -> f32 {
        if self.active_tab == 0 {
            self.get_tasks_add_bar_cy()
        } else {
            self.get_agenda_add_bar_cy()
        }
    }

    fn handle_click(&mut self, lx: f32, ly: f32, is_right: bool) -> bool {
        let half_w = CARD_W * 0.5;
        let half_h = CARD_H * 0.5;
        let local_x = lx - half_w;
        let local_y = ly - half_h;

        // Check Top Tabs & Header Settings Button (tab_y = -195.0, cy = 32.5)
        if (local_y - (-195.0)).abs() <= 22.0 {
            // Header Settings button on top-right (px > 310.0, local_x > 130.0)
            if local_x > 125.0 {
                self.active_tab = 1;
                self.is_cal_editing = !self.is_cal_editing;
                if self.is_cal_editing {
                    self.cal_input_buffer.clear();
                    self.cal_edit_start_time = Instant::now();
                }
                self.draw();
                return true;
            }

            if self.is_editing {
                self.is_editing = false;
                self.input_buffer.clear();
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

        // Check Add task button in Tasks view (dynamic cy hug-content)
        let add_bar_cy = self.get_add_bar_cy();
        let add_local_y = add_bar_cy - half_h;
        if self.active_tab == 0 && (local_y - add_local_y).abs() <= 24.0 {
            self.is_editing = true;
            self.input_buffer.clear();
            self.edit_start_time = Instant::now();
            self.draw();
            return true;
        }

        // Check Agenda bottom controls (dynamic cy hug-content)
        if self.active_tab == 1 && (local_y - add_local_y).abs() <= 24.0 {
            if self.is_cal_editing {
                // If clicked Save on right (local_x > 90.0)
                if local_x > 90.0 {
                    let url = self.cal_input_buffer.trim().to_string();
                    if !url.is_empty() {
                        let home = std::env::var("HOME").unwrap_or_default();
                        let cfg_path = format!("{}/.local/state/calendar_config.json", home);
                        let json = serde_json::json!({
                            "ics_url": url,
                            "_comment": "Google Calendar iCal private link"
                        });
                        if let Ok(s) = serde_json::to_string_pretty(&json) {
                            let _ = std::fs::write(&cfg_path, s);
                        }
                        let sync_script =
                            format!("{}/.config/quickshell-todo-widget/calendar_sync.py", home);
                        if std::path::Path::new(&sync_script).exists() {
                            let _ = std::process::Command::new("python3")
                                .arg(&sync_script)
                                .spawn();
                        }
                        self.last_calendar_check =
                            Instant::now() - std::time::Duration::from_secs(10);
                        self.check_calendar();
                    }
                }
                self.is_cal_editing = false;
                self.cal_input_buffer.clear();
                self.draw();
                return true;
            }

            if local_x > 80.0 {
                // Clicked ⚙ Settings button (cx = 308.0)
                if is_right {
                    let home = std::env::var("HOME").unwrap_or_default();
                    let cfg_path = format!("{}/.local/state/calendar_config.json", home);
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&cfg_path)
                        .spawn();
                } else {
                    self.is_cal_editing = true;
                    self.cal_edit_start_time = Instant::now();
                    self.cal_input_buffer.clear();

                    // Pre-fill from clipboard if it contains a calendar URL
                    if let Ok(out) = std::process::Command::new("wl-paste")
                        .arg("--no-newline")
                        .output()
                    {
                        if let Ok(clip) = String::from_utf8(out.stdout) {
                            let clean = clip.trim();
                            if clean.starts_with("http") && clean.contains(".ics") {
                                self.cal_input_buffer.push_str(clean);
                            }
                        }
                    }
                }
                self.draw();
                return true;
            } else if local_x < -40.0 {
                // Clicked ↻ Sync button (cx = 64.0)
                let home = std::env::var("HOME").unwrap_or_default();
                let sync_script =
                    format!("{}/.config/quickshell-todo-widget/calendar_sync.py", home);
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
        }

        // If clicking outside while editing, commit or close
        if self.is_cal_editing {
            self.is_cal_editing = false;
            self.cal_input_buffer.clear();
            self.draw();
            return true;
        }

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

    pub fn check_todos(&mut self) {
        let interval_ms = if self.is_editing { 200 } else { 3000 };
        if self.last_todo_check.elapsed().as_millis() < interval_ms {
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

    pub fn check_calendar(&mut self) {
        let interval_ms = if self.is_cal_editing { 200 } else { 3000 };
        if self.last_calendar_check.elapsed().as_millis() < interval_ms {
            return;
        }
        self.last_calendar_check = Instant::now();

        // Auto-sync Google Calendar via calendar_sync.py every 1 hour
        if self.last_calendar_sync.elapsed().as_secs() >= 3600 {
            self.last_calendar_sync = Instant::now();
            let home = std::env::var("HOME").unwrap_or_default();
            let sync_script = format!("{}/.config/quickshell-todo-widget/calendar_sync.py", home);
            if std::path::Path::new(&sync_script).exists() {
                let _ = std::process::Command::new("python3")
                    .arg(&sync_script)
                    .spawn();
            }
        }

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

    pub fn update_physics(&mut self, dt: f32) -> bool {
        let eff_dt = dt * self.anim_speed;
        self.tab_spring.update(eff_dt);
        self.filter_spring.update(eff_dt);
        self.size_spring.update(eff_dt);

        let t = self.size_spring.value.clamp(0.0, 1.0);
        let card_h = self.get_card_height();
        self.target_h = card_h;
        self.target_w = CARD_W;
        self.current_w = COMPACT_W + (self.target_w - COMPACT_W) * t;
        self.current_h = COMPACT_H + (self.target_h - COMPACT_H) * t;

        // Auto-collapse when unhovered after inactivity (2.5 seconds)
        if !self.is_compact
            && !self.is_hovered
            && !self.is_editing
            && !self.is_cal_editing
            && self.last_hover_time.elapsed().as_secs_f32() > 2.5
        {
            self.collapse();
        }

        let mut slide_animating = false;
        if !self.is_sliding {
            for off in &mut self.slide_offsets {
                if *off != 0.0 {
                    *off += (0.0 - *off) * (20.0 * eff_dt).min(1.0);
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
            || !self.size_spring.is_at_rest()
            || self.is_editing
            || self.is_cal_editing
            || slide_animating
            || self.is_sliding
    }

    fn render_ui_overlay(&self, buf: &mut [u8]) {
        let content_w = CARD_W as u32;
        let content_h = CARD_H as u32;
        let tasks_add_bar_cy = self.get_tasks_add_bar_cy();
        let agenda_add_bar_cy = self.get_agenda_add_bar_cy();
        let tasks_offset_x = -self.tab_spring.value * CARD_W;
        let agenda_offset_x = (1.0 - self.tab_spring.value) * CARD_W;

        let is_light = self.effective_is_light();

        // Apple macOS Typography & Contrast
        let (col_primary, col_secondary, col_dimmed, _col_accent, col_shadow) = if is_light {
            (
                [8, 12, 20, 255],  // Primary text: Deep Obsidian Ink (> 15:1)
                [18, 24, 34, 255], // Secondary text: Refined Near-Black Slate
                [28, 36, 50, 255], // Tertiary/Dimmed: Dark Slate
                [8, 12, 18, 255],  // Accent text
                [0, 0, 0, 0],      // Zero shadow in light mode to prevent bleaching
            )
        } else {
            (
                Tokens::TEXT_PRIMARY,   // Primary text: Crisp Pure White
                Tokens::TEXT_SECONDARY, // Secondary text: Soft White
                Tokens::TEXT_TERTIARY,  // Tertiary/Dimmed: Dimmed White
                Tokens::TEXT_PRIMARY,   // White accent
                Tokens::TEXT_SHADOW,    // Downward soft drop shadow (0, +1)
            )
        };

        // ── Top Segmented Tabs: "Tasks ({count})" & "Agenda" (cy = 32.5) ──
        // Clean Apple / VisionOS Active Ink: Pure White on dark, Obsidian on light
        let selected_ink = if is_light {
            [12, 16, 24, 255]
        } else {
            [255, 255, 255, 255]
        };
        let tab_t = self.tab_spring.value.clamp(0.0, 1.0);

        // Segmented Track (cx = 160.0, cy = 32.5, half_w = 138.0, half_h = 16.0, r = 10.0)
        UIFont::draw_rounded_rect_fill(
            buf,
            content_w,
            content_h,
            160.0,
            32.5,
            138.0,
            16.0,
            10.0,
            if is_light {
                [0, 0, 0, 16]
            } else {
                [255, 255, 255, 14]
            },
        );

        // Sliding Active Tab Pill (Liquid Glass Lens)
        let tab_pill_cx = 96.0 * (1.0 - tab_t) + 224.0 * tab_t;
        let pill_fill = if is_light {
            [255, 255, 255, 110]
        } else {
            [255, 255, 255, 48]
        };
        UIFont::draw_rounded_rect_fill(
            buf,
            content_w,
            content_h,
            tab_pill_cx,
            32.5,
            64.0,
            13.5,
            8.0,
            pill_fill,
        );

        // Top Right: Settings Icon Button
        let is_settings_active = self.is_cal_editing;
        let settings_bg = if is_settings_active {
            if is_light {
                Tokens::INNER_TILE_BG_HOVER_LIGHT
            } else {
                Tokens::INNER_TILE_BG_HOVER_DARK
            }
        } else if is_light {
            Tokens::INNER_TILE_BG_LIGHT
        } else {
            Tokens::INNER_TILE_BG_DARK
        };
        UIFont::draw_rounded_rect_fill(
            buf,
            content_w,
            content_h,
            330.0,
            32.5,
            13.0,
            13.0,
            8.0,
            settings_bg,
        );
        let settings_col = if is_light {
            [12, 16, 24, 255]
        } else {
            Tokens::TEXT_PRIMARY
        };
        UIFont::draw_settings_icon(buf, content_w, content_h, 330.0, 32.5, settings_col);

        let tasks_count = self.todos.tasks.len();
        let tasks_label = format!("Tasks ({})", tasks_count);
        let tab1_col = if tab_t < 0.5 {
            selected_ink
        } else if is_light {
            [18, 24, 34, 255]
        } else {
            [210, 220, 235, 230]
        };
        self.font.draw_text_centered(
            buf,
            content_w,
            content_h,
            &tasks_label,
            96.0,
            32.5,
            14.5,
            tab1_col,
            [0, 0, 0, 0],
            FontWeight::SemiBold,
        );

        let agenda_label = "Agenda";
        let tab2_col = if tab_t >= 0.5 {
            selected_ink
        } else if is_light {
            [18, 24, 34, 255]
        } else {
            [210, 220, 235, 230]
        };
        self.font.draw_text_centered(
            buf,
            content_w,
            content_h,
            agenda_label,
            224.0,
            32.5,
            14.5,
            tab2_col,
            [0, 0, 0, 0],
            FontWeight::SemiBold,
        );

        if tasks_offset_x > -(content_w as f32) + 2.0 {
            let add_bar_cy = tasks_add_bar_cy;

            // 2. Filter Bar (cy = 79.5)
            let filter_t = self.filter_spring.value.clamp(0.0, 2.0);
            let filt_cx = 84.0 + filter_t * 72.0;

            // Subtle filter track
            UIFont::draw_rounded_rect_fill(
                buf,
                content_w,
                content_h,
                156.0 + tasks_offset_x,
                79.5,
                108.0,
                13.0,
                8.0,
                if is_light {
                    [0, 0, 0, 14]
                } else {
                    [255, 255, 255, 14]
                },
            );

            // Sliding active filter pill (Liquid Glass Lens)
            let filter_pill_fill = if is_light {
                [255, 255, 255, 110]
            } else {
                [255, 255, 255, 48]
            };
            UIFont::draw_rounded_rect_fill(
                buf,
                content_w,
                content_h,
                filt_cx + tasks_offset_x,
                79.5,
                32.0,
                11.0,
                7.0,
                filter_pill_fill,
            );

            // Filter Bar Labels: "All" (cx = 84.0), "Active" (cx = 156.0), "Done" (cx = 228.0), cy = 79.5
            let filter_unselected = if is_light {
                [35, 45, 60, 245]
            } else {
                [205, 215, 230, 230]
            };

            let f0_col = if self.active_filter == 0 {
                selected_ink
            } else {
                filter_unselected
            };
            let f1_col = if self.active_filter == 1 {
                selected_ink
            } else {
                filter_unselected
            };
            let f2_col = if self.active_filter == 2 {
                selected_ink
            } else {
                filter_unselected
            };

            self.font.draw_text_centered(
                buf,
                content_w,
                content_h,
                "All",
                84.0 + tasks_offset_x,
                79.5,
                14.0,
                f0_col,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                content_w,
                content_h,
                "Active",
                156.0 + tasks_offset_x,
                79.5,
                14.0,
                f1_col,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                content_w,
                content_h,
                "Done",
                228.0 + tasks_offset_x,
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
                    content_w,
                    content_h,
                    msg,
                    180.0 + tasks_offset_x,
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
                let (_mean_lum, _spread, _) = if let Some(crop) = &self.wallpaper_crop {
                    sample_subcrop_stats(
                        crop,
                        self.width,
                        self.height,
                        68,
                        (cy - 13.0).max(0.0) as u32,
                        text_w,
                        26,
                    )
                } else {
                    (0.5, 0.0, self.is_wallpaper_light)
                };

                // Unified coherent typography across the entire card
                let row_is_light = is_light;

                // Subtle scrim under text for 100% legibility across light and dark backdrops
                let text_shadow_col = if row_is_light {
                    [0, 0, 0, 0]
                } else {
                    [10, 14, 22, 160]
                };

                let (row_primary, row_dimmed) = if row_is_light {
                    ([12, 16, 24, 255], [100, 110, 125, 230])
                } else {
                    ([255, 255, 255, 255], [180, 190, 205, 230])
                };

                let (text_col, weight) = if task.done {
                    (row_dimmed, FontWeight::Regular)
                } else {
                    (row_primary, FontWeight::Bold)
                };

                // Slide-to-Delete: Pure optical translucent liquid glass
                if slide_x < -1.0 {
                    let reveal_t = (-slide_x / 80.0).clamp(0.0, 1.0);
                    let glass_red_fill = [235, 45, 65, (reveal_t * 52.0) as u8];
                    let glass_red_rim = [255, 90, 115, (reveal_t * 95.0) as u8];

                    UIFont::draw_rounded_rect_fill(
                        buf,
                        content_w,
                        content_h,
                        180.0 + tasks_offset_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        glass_red_fill,
                    );
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        content_w,
                        content_h,
                        180.0 + tasks_offset_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        1.2,
                        glass_red_rim,
                    );

                    let label = if slide_x < -75.0 {
                        "Release to delete"
                    } else {
                        "Slide to delete"
                    };
                    self.font.draw_text_centered(
                        buf,
                        content_w,
                        content_h,
                        label,
                        270.0 + tasks_offset_x,
                        cy,
                        11.5,
                        [255, 235, 240, 220],
                        [0, 0, 0, 0],
                        FontWeight::Bold,
                    );

                    // Top: Translucent sliding liquid glass row
                    let slide_rim = if row_is_light {
                        [160, 175, 200, 70]
                    } else {
                        [255, 255, 255, 75]
                    };
                    let slide_fill = if row_is_light {
                        [255, 255, 255, 20]
                    } else {
                        [255, 255, 255, 14]
                    };
                    UIFont::draw_rounded_rect_fill(
                        buf,
                        content_w,
                        content_h,
                        180.0 + slide_x + tasks_offset_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        slide_fill,
                    );
                    UIFont::draw_rounded_rect_stroke(
                        buf,
                        content_w,
                        content_h,
                        180.0 + slide_x + tasks_offset_x,
                        cy,
                        156.0,
                        18.0,
                        9.0,
                        1.0,
                        slide_rim,
                    );
                } else {
                    let cur_lx = self.cursor_pos[0] * (self.width as f32);
                    let cur_ly = self.cursor_pos[1] * (self.height as f32);
                    let is_row_hover = self.is_hovered
                        && (cur_lx - (180.0 + slide_x + tasks_offset_x)).abs() <= 156.0
                        && (cur_ly - cy).abs() <= 18.0;

                    if is_row_hover {
                        let hover_fill = if row_is_light {
                            [0, 0, 0, 14]
                        } else {
                            [255, 255, 255, 20]
                        };
                        UIFont::draw_rounded_rect_fill(
                            buf,
                            content_w,
                            content_h,
                            180.0 + slide_x + tasks_offset_x,
                            cy,
                            156.0,
                            18.0,
                            Tokens::RADIUS_INNER_TILE,
                            hover_fill,
                        );
                    }

                    // Delicate hairline divider between rows (clean Apple Reminders / Linear list design)
                    if row_idx < visible_indices.len().min(6) - 1 {
                        let div_col = if row_is_light {
                            [0, 0, 0, 14]
                        } else {
                            [255, 255, 255, 14]
                        };
                        UIFont::draw_segment(
                            buf,
                            content_w,
                            content_h,
                            (180.0 + slide_x + tasks_offset_x - 145.0, cy + 22.0),
                            (180.0 + slide_x + tasks_offset_x + 145.0, cy + 22.0),
                            1.0,
                            div_col,
                        );
                    }
                }

                // Precision anti-aliased vector checkbox
                let chk_cx = 44.0 + slide_x + tasks_offset_x;
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
                        buf, content_w, content_h, chk_cx, cy, 9.0, 9.0, 5.0, bg_box,
                    );
                    UIFont::draw_checkmark(buf, content_w, content_h, chk_cx, cy, 1.1, 1.6, fg_chk);
                } else {
                    let stroke_box = if row_is_light {
                        [10, 14, 22, 230]
                    } else {
                        [255, 255, 255, 230]
                    };
                    UIFont::draw_rounded_rect_stroke(
                        buf, content_w, content_h, chk_cx, cy, 9.0, 9.0, 5.0, 1.3, stroke_box,
                    );
                }

                // Truncate task text dynamically based on measured width
                let max_text_w = 245.0f32;
                let mut display_text = task.text.clone();
                let text_x = 68.0 + slide_x + tasks_offset_x;
                if self.font.measure_text(&display_text, 14.5, weight) > max_text_w {
                    while !display_text.is_empty()
                        && self
                            .font
                            .measure_text(&format!("{display_text}…"), 14.5, weight)
                            > max_text_w
                    {
                        display_text.pop();
                    }
                    display_text.push('…');
                }

                let end_x = self.font.draw_text_centered_y(
                    buf,
                    content_w,
                    content_h,
                    &display_text,
                    text_x,
                    cy,
                    14.5,
                    text_col,
                    text_shadow_col,
                    weight,
                );

                if task.done {
                    // Strikethrough
                    QuicksandFont::draw_strikethrough(
                        buf,
                        content_w,
                        content_h,
                        57.0 + slide_x + tasks_offset_x,
                        end_x + 2.0,
                        cy,
                        1.5,
                        col_dimmed,
                    );
                }
            }

            // 4. Bottom Add Task Bar (hug-content dynamic cy)
            let (add_primary, add_dimmed, add_secondary) = if is_light {
                (
                    [12, 16, 24, 255],
                    [35, 45, 60, 245], // Ink at ~75% equivalent (> 5.5:1 contrast against light background)
                    [35, 45, 60, 245],
                )
            } else {
                (
                    [255, 255, 255, 255],
                    [180, 190, 205, 230],
                    [175, 185, 200, 210],
                )
            };

            let cur_lx = self.cursor_pos[0] * (self.width as f32);
            let cur_ly = self.cursor_pos[1] * (self.height as f32);
            let is_add_hover = self.is_hovered
                && (cur_lx - (180.0 + tasks_offset_x)).abs() <= 156.0
                && (cur_ly - add_bar_cy).abs() <= 16.0;

            if self.is_editing {
                let edit_bg = if is_light {
                    [0, 0, 0, 12]
                } else {
                    [255, 255, 255, 18]
                };
                let edit_stroke = if is_light {
                    [0, 0, 0, 30]
                } else {
                    [255, 255, 255, 60]
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    content_w,
                    content_h,
                    180.0 + tasks_offset_x,
                    add_bar_cy,
                    156.0,
                    15.0,
                    Tokens::RADIUS_INNER_TILE,
                    edit_bg,
                );
                UIFont::draw_rounded_rect_stroke(
                    buf,
                    content_w,
                    content_h,
                    180.0 + tasks_offset_x,
                    add_bar_cy,
                    156.0,
                    15.0,
                    Tokens::RADIUS_INNER_TILE,
                    1.0,
                    edit_stroke,
                );
            } else if is_add_hover {
                let hover_bg = if is_light {
                    [0, 0, 0, 10]
                } else {
                    [255, 255, 255, 14]
                };
                UIFont::draw_rounded_rect_fill(
                    buf,
                    content_w,
                    content_h,
                    180.0 + tasks_offset_x,
                    add_bar_cy,
                    156.0,
                    15.0,
                    Tokens::RADIUS_INNER_TILE,
                    hover_bg,
                );
            }

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
                    content_w,
                    content_h,
                    input_str,
                    42.0 + tasks_offset_x,
                    add_bar_cy,
                    14.5,
                    input_col,
                    col_shadow,
                    FontWeight::Medium,
                );

                // Blinking cursor
                let elapsed_ms = self.edit_start_time.elapsed().as_millis();
                if (elapsed_ms / 500).is_multiple_of(2) {
                    let cursor_x = if self.input_buffer.is_empty() {
                        42.0 + tasks_offset_x
                    } else {
                        end_x + 2.0
                    };
                    QuicksandFont::draw_cursor(
                        buf,
                        content_w,
                        content_h,
                        cursor_x,
                        add_bar_cy - 9.0,
                        18.0,
                        2.0,
                        add_primary,
                    );
                }

                // Key hint: "↵ Enter"
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "↵ Enter",
                    295.0 + tasks_offset_x,
                    add_bar_cy,
                    12.0,
                    add_dimmed,
                    col_shadow,
                    FontWeight::Medium,
                );
            } else {
                self.font.draw_text_centered_y(
                    buf,
                    content_w,
                    content_h,
                    "Add task",
                    42.0 + tasks_offset_x,
                    add_bar_cy,
                    14.5,
                    add_secondary,
                    col_shadow,
                    FontWeight::Medium,
                );

                // Dedicated '+' action button pill on right sharing input bar's ink
                UIFont::draw_rounded_rect_fill(
                    buf,
                    content_w,
                    content_h,
                    315.0 + tasks_offset_x,
                    add_bar_cy,
                    11.0,
                    11.0,
                    11.0,
                    if is_light {
                        [0, 0, 0, 16]
                    } else {
                        [255, 255, 255, 28]
                    },
                );
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "+",
                    315.0 + tasks_offset_x,
                    add_bar_cy,
                    16.0,
                    add_primary,
                    col_shadow,
                    FontWeight::Bold,
                );
            }
        }

        if agenda_offset_x < (content_w as f32) - 2.0 {
            let add_bar_cy = agenda_add_bar_cy;

            // Agenda View (Events from ~/.local/state/calendar_events.json, cy = 122.5 + idx * 44.0)
            if self.events.is_empty() {
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "No upcoming events",
                    180.0 + agenda_offset_x,
                    190.0,
                    14.5,
                    col_dimmed,
                    col_shadow,
                    FontWeight::Medium,
                );
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "Click ⚙ Secret below to connect Google Calendar",
                    180.0 + agenda_offset_x,
                    216.0,
                    11.5,
                    col_secondary,
                    col_shadow,
                    FontWeight::Regular,
                );
            } else {
                for (idx, item) in self.events.iter().enumerate().take(6) {
                    let cy = 122.5 + (idx as f32) * 44.0;

                    UIFont::draw_rounded_rect_fill(
                        buf,
                        content_w,
                        content_h,
                        180.0 + agenda_offset_x,
                        cy,
                        156.0,
                        18.0,
                        Tokens::RADIUS_INNER_TILE,
                        if is_light {
                            Tokens::INNER_TILE_BG_LIGHT
                        } else {
                            Tokens::INNER_TILE_BG_DARK
                        },
                    );

                    // Time (cx = 64.0)
                    let time_str = if !item.time.is_empty() {
                        item.time.as_str()
                    } else {
                        "ALL DAY"
                    };
                    self.font.draw_text_centered(
                        buf,
                        content_w,
                        content_h,
                        time_str,
                        64.0 + agenda_offset_x,
                        cy,
                        11.0,
                        col_secondary,
                        col_shadow,
                        FontWeight::Medium,
                    );

                    // Title (starts at x = 108.0)
                    let title = if item.title.chars().count() > 22 {
                        let truncated: String = item.title.chars().take(20).collect();
                        format!("{}...", truncated)
                    } else {
                        item.title.clone()
                    };
                    self.font.draw_text_centered_y(
                        buf,
                        content_w,
                        content_h,
                        &title,
                        108.0 + agenda_offset_x,
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
                        content_w,
                        content_h,
                        tag_str,
                        304.0 + agenda_offset_x,
                        cy,
                        10.0,
                        col_dimmed,
                        col_shadow,
                        FontWeight::Bold,
                    );
                }
            }

            // Bottom Agenda Bar (dynamic cy hug-content)
            if self.is_cal_editing {
                let input_str = if self.cal_input_buffer.is_empty() {
                    "Paste Google iCal secret link..."
                } else {
                    &self.cal_input_buffer
                };
                let input_col = if self.cal_input_buffer.is_empty() {
                    col_dimmed
                } else {
                    col_primary
                };

                let end_x = self.font.draw_text_centered_y(
                    buf,
                    content_w,
                    content_h,
                    input_str,
                    38.0 + agenda_offset_x,
                    add_bar_cy,
                    12.0,
                    input_col,
                    col_shadow,
                    FontWeight::Medium,
                );

                let elapsed_ms = self.cal_edit_start_time.elapsed().as_millis();
                if (elapsed_ms / 500).is_multiple_of(2) {
                    let cursor_x = if self.cal_input_buffer.is_empty() {
                        38.0 + agenda_offset_x
                    } else {
                        end_x + 2.0
                    };
                    QuicksandFont::draw_cursor(
                        buf,
                        content_w,
                        content_h,
                        cursor_x,
                        add_bar_cy - 9.0,
                        18.0,
                        2.0,
                        col_primary,
                    );
                }

                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "↵ Save",
                    315.0 + agenda_offset_x,
                    add_bar_cy,
                    11.5,
                    col_primary,
                    col_shadow,
                    FontWeight::Bold,
                );
            } else {
                let btn_bg = if is_light {
                    [220, 226, 238, 120]
                } else {
                    Tokens::INNER_TILE_BG_DARK
                };

                // 1. Sync button pill (cx = 64.0) with Lucide Sync icon
                UIFont::draw_rounded_rect_fill(
                    buf,
                    content_w,
                    content_h,
                    64.0 + agenda_offset_x,
                    add_bar_cy,
                    32.0,
                    12.0,
                    8.0,
                    btn_bg,
                );
                UIFont::draw_sync_icon(
                    buf,
                    content_w,
                    content_h,
                    46.0 + agenda_offset_x,
                    add_bar_cy,
                    col_primary,
                );
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    "Sync",
                    72.0 + agenda_offset_x,
                    add_bar_cy,
                    11.5,
                    col_primary,
                    col_shadow,
                    FontWeight::Bold,
                );

                // 2. Events status count (cx = 175.0)
                let cal_status = format!("{} Events", self.events.len());
                self.font.draw_text_centered(
                    buf,
                    content_w,
                    content_h,
                    &cal_status,
                    175.0 + agenda_offset_x,
                    add_bar_cy,
                    12.0,
                    col_secondary,
                    col_shadow,
                    FontWeight::SemiBold,
                );

                // 3. Settings button pill with Lucide Settings gear icon (cx = 308.0)
                UIFont::draw_rounded_rect_fill(
                    buf,
                    content_w,
                    content_h,
                    308.0 + agenda_offset_x,
                    add_bar_cy,
                    15.0,
                    12.0,
                    8.0,
                    btn_bg,
                );
                UIFont::draw_settings_icon(
                    buf,
                    content_w,
                    content_h,
                    308.0 + agenda_offset_x,
                    add_bar_cy,
                    col_primary,
                );
            }
        }
    }

    pub fn draw(&mut self) -> bool {
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

        let _card_h = self.get_card_height();

        // Generate dynamic Quicksand font overlay (reusing overlay_buffer without frame reallocations)
        let required_len = (self.width * self.height * 4) as usize;
        if self.overlay_buffer.len() != required_len {
            self.overlay_buffer.resize(required_len, 0);
        } else {
            self.overlay_buffer.fill(0);
        }
        let mut overlay_buf = std::mem::take(&mut self.overlay_buffer);

        let t = self.size_spring.value.clamp(0.0, 1.0);
        let cx = (self.width as f32) * 0.5;

        // 1. Render expanded content into overlay_buf if t > 0.05
        if t > 0.05 {
            let content_w = CARD_W as usize;
            let content_h = CARD_H as usize;
            let mut content_buf = vec![0u8; content_w * content_h * 4];
            self.render_ui_overlay(&mut content_buf);

            let alpha_factor = ((t - 0.1) / 0.9).clamp(0.0, 1.0);
            let slide_y = ((1.0 - t) * -22.0) as i32;
            let dst_x = (cx - CARD_W * 0.5) as usize;
            let stride_src = content_w * 4;
            let stride_dst = self.width as usize * 4;

            for row in 0..content_h {
                let target_y = (row as i32 + slide_y).clamp(0, (self.height as i32) - 1) as usize;
                let src_start = row * stride_src;
                let dst_start = target_y * stride_dst + dst_x * 4;
                if dst_start + stride_src <= overlay_buf.len()
                    && src_start + stride_src <= content_buf.len()
                {
                    for px_i in 0..content_w {
                        let si = src_start + px_i * 4;
                        let di = dst_start + px_i * 4;
                        let a = (content_buf[si + 3] as f32 * alpha_factor) as u8;
                        if a > 0 {
                            overlay_buf[di] = content_buf[si];
                            overlay_buf[di + 1] = content_buf[si + 1];
                            overlay_buf[di + 2] = content_buf[si + 2];
                            overlay_buf[di + 3] = a;
                        }
                    }
                }
            }
        }

        // 2. Render compact tab overview if t < 0.35
        if t < 0.35 {
            let comp_alpha = ((0.35 - t) / 0.35).clamp(0.0, 1.0);
            let is_light = self.effective_is_light();
            let ink_col = if is_light {
                [10, 14, 22, (255.0 * comp_alpha) as u8]
            } else {
                [255, 255, 255, (255.0 * comp_alpha) as u8]
            };
            let shadow_col = [0, 0, 0, 0];

            // Center of compact pill: cx, y = 18.0
            UIFont::draw_check_icon(
                &mut overlay_buf,
                self.width,
                self.height,
                cx - 36.0,
                18.0,
                ink_col,
            );
            self.font.draw_text_centered_y(
                &mut overlay_buf,
                self.width,
                self.height,
                "Tasks",
                cx - 22.0,
                18.0,
                13.5,
                ink_col,
                shadow_col,
                FontWeight::SemiBold,
            );

            // Count pending tasks
            let pending = self.todos.tasks.iter().filter(|tk| !tk.done).count();
            let badge_str = format!("{}", pending);
            let badge_w = self.font.measure_text(&badge_str, 12.0, FontWeight::Bold);
            let badge_cx = cx + 32.0;
            let badge_bg = if is_light {
                [0, 0, 0, (28.0 * comp_alpha) as u8]
            } else {
                [255, 255, 255, (36.0 * comp_alpha) as u8]
            };
            UIFont::draw_rounded_rect_fill(
                &mut overlay_buf,
                self.width,
                self.height,
                badge_cx,
                18.0,
                (badge_w + 10.0) * 0.5,
                9.0,
                5.0,
                badge_bg,
            );
            let badge_text_x = badge_cx - badge_w * 0.5;
            let badge_baseline = self
                .font
                .vertical_center_baseline(18.0, 12.0, FontWeight::Bold);
            self.font.draw_text_with_shadow(
                &mut overlay_buf,
                self.width,
                self.height,
                &badge_str,
                badge_text_x,
                badge_baseline,
                12.0,
                ink_col,
                shadow_col,
                FontWeight::Bold,
            );
        }

        let is_light_f32 = if self.effective_is_light() { 1.0 } else { 0.0 };

        let rendered = if let Some(renderer) = &mut self.renderer {
            renderer.update_overlay(self.width, self.height, &overlay_buf);

            let (px, py) = (self.cursor_pos[0], self.cursor_pos[1]);

            let card_left = cx - self.current_w * 0.5;
            let x_norm = card_left / (self.width as f32);
            let y_norm = 0.0;
            let w_norm = self.current_w / (self.width as f32);
            let h_norm = self.current_h / (self.height as f32);

            let todo_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle {
                    radius: Tokens::RADIUS_OUTER,
                })
                .bounds(x_norm, y_norm, w_norm, h_norm)
                .curvature(self.fillet_radius)
                .bevel(22.0)
                .refraction(20.0)
                .dispersion(0.14)
                .blur(self.blur_radius)
                .tint(0.0, progress_packed, 1.0, is_light_f32)
                .roughness(-1.0)
                .specular(1.10)
                .fresnel(0.60)
                .style_mode(self.style_mode)
                .element_type(26.0)
                .deformation(if self.is_docked { 2.0 } else { 0.0 })
                .pointer_pos(px, py);

            let surfaces = vec![todo_card];
            let _ = renderer.render(&surfaces);
            true
        } else {
            false
        };
        self.overlay_buffer = overlay_buf;
        rendered
    }
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
        (mean as f32, spread as f32, mean > 0.32)
    } else {
        (0.0, 0.0, false)
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_subcrop_stats(
    crop: &[u8],
    crop_w: u32,
    crop_h: u32,
    lx: u32,
    ly: u32,
    lw: u32,
    lh: u32,
) -> (f32, f32, bool) {
    let lx = lx + CARD_PAD as u32;
    let ly = ly + CARD_PAD as u32;
    let mut sub = Vec::with_capacity((lw * lh * 4) as usize);
    for row in 0..lh {
        let sy = (ly + row).min(crop_h.saturating_sub(1));
        let sx = lx.min(crop_w.saturating_sub(1));
        let actual_w = lw.min(crop_w.saturating_sub(sx));
        let src_idx = ((sy * crop_w + sx) * 4) as usize;
        let end_idx = src_idx + (actual_w * 4) as usize;
        if end_idx <= crop.len() {
            sub.extend_from_slice(&crop[src_idx..end_idx]);
        }
    }
    buffer_lum_stats(&sub)
}

#[allow(dead_code)]
fn sample_subcrop_is_light(
    crop: &[u8],
    crop_w: u32,
    crop_h: u32,
    lx: u32,
    ly: u32,
    lw: u32,
    lh: u32,
) -> bool {
    sample_subcrop_stats(crop, crop_w, crop_h, lx, ly, lw, lh).2
}

#[allow(dead_code)]
fn is_buffer_light(buf: &[u8]) -> bool {
    buffer_lum_stats(buf).2
}

#[allow(dead_code)]
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
