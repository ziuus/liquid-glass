// I will keep capture module for Screencopy specific struct and delegation.
use smithay_client_toolkit::{
    registry::{ProvidesRegistryState, RegistryState, GlobalProxy},
    shm::{Shm, ShmHandler, slot::{SlotPool, Buffer}},
};
use wayland_client::{
    protocol::{wl_output::WlOutput, wl_shm::WlShm, wl_buffer::WlBuffer},
    Connection, QueueHandle, Dispatch, delegate_dispatch,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    zwlr_screencopy_frame_v1::{ZwlrScreencopyFrameV1, Event as FrameEvent, ZwlrScreencopyFrameV1 as Frame},
};
use std::os::fd::AsRawFd;

pub struct ScreencopyState {
    pub manager: Option<ZwlrScreencopyManagerV1>,
    pub frame: Option<Frame>,
    pub pool: SlotPool,
    pub buffer: Option<smithay_client_toolkit::shm::slot::Buffer>,
    pub data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
    pub done: bool,
    pub is_capturing: bool,
}

impl ScreencopyState {
    pub fn new(registry: &RegistryState, qh: &QueueHandle<super::AppData>, shm: &Shm) -> Self {
        let manager = registry.bind_one::<ZwlrScreencopyManagerV1, _, _>(qh, 1..=3, ()).ok();
        let pool = SlotPool::new(1024 * 1024 * 4, shm).expect("Failed to create Shm SlotPool");
        Self {
            manager,
            frame: None,
            pool,
            buffer: None,
            data: None,
            width: 0,
            height: 0,
            stride: 0,
            format: 0,
            done: false,
            is_capturing: false,
        }
    }

    pub fn capture(&mut self, output: &WlOutput, x: i32, y: i32, width: i32, height: i32, qh: &QueueHandle<super::AppData>) {
        if self.is_capturing {
            return;
        }
        if let Some(manager) = &self.manager {
            self.done = false;
            self.data = None;
            self.is_capturing = true;
            self.frame = Some(manager.capture_output_region(0, output, x, y, width, height, qh, ()));
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for super::AppData {
    fn event(
        _state: &mut Self,
        _manager: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for super::AppData {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: FrameEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            FrameEvent::Buffer { format, width, height, stride } => {
                let actual_format = format.into_result().unwrap_or(wayland_client::protocol::wl_shm::Format::Argb8888);
                state.screencopy.width = width;
                state.screencopy.height = height;
                state.screencopy.stride = stride;
                state.screencopy.format = actual_format as u32;
                
                if let Ok((buffer, _canvas)) = state.screencopy.pool.create_buffer(
                    width as i32,
                    height as i32,
                    stride as i32,
                    actual_format
                ) {
                    frame.copy(buffer.wl_buffer());
                    state.screencopy.buffer = Some(buffer);
                }
            }
            FrameEvent::Flags { flags } => {}
            FrameEvent::Ready { .. } => {
                state.screencopy.done = true;
                state.screencopy.is_capturing = false;
                if let Some(buffer) = state.screencopy.buffer.take() {
                    if let Some(canvas) = state.screencopy.pool.canvas(&buffer) {
                        // Assuming XRGB8888 or ARGB8888 format from Wayland
                        // Convert to RGBA8 for wgpu
                        let width = state.screencopy.width as usize;
                        let height = state.screencopy.height as usize;
                        let stride = state.screencopy.stride as usize;
                        
                        let mut rgba = Vec::with_capacity(width * height * 4);
                        for y in 0..height {
                            let row = &canvas[(y * stride)..(y * stride + width * 4)];
                            for chunk in row.chunks_exact(4) {
                                // BGRA -> RGBA (typical for wl_shm ARGB8888/XRGB8888 in little endian)
                                rgba.push(chunk[2]);
                                rgba.push(chunk[1]);
                                rgba.push(chunk[0]);
                                rgba.push(255);
                            }
                        }
                        state.screencopy.data = Some(rgba);
                    }
                }
            }
            FrameEvent::Failed => {
                state.screencopy.done = true;
                state.screencopy.is_capturing = false;
            }
            FrameEvent::Damage { .. } => {}
            _ => {}
        }
    }
}
