use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use glass_core::{GlassRenderer, GlassSurface, Material, geometry::Shape};
use std::sync::Arc;

async fn run() {
    env_logger::init();
    
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new()
        .with_title("Liquid Glass Example")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .with_transparent(true)
        .build(&event_loop)
        .unwrap());

    let size = window.inner_size();
    let mut renderer = GlassRenderer::new(window.clone(), size.width, size.height).await.unwrap();

    let mut cursor_pos = [0.5, 0.5];
    let mut cursor_velocity = [0.0, 0.0];

    let surfaces = vec![
        GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::RoundedRectangle { radius: 28.0 })
            // Normalized bounds: x, y, w, h
            .bounds(0.2, 0.2, 0.6, 0.6),
        GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::RoundedRectangle { radius: 15.0 })
            .bounds(0.4, 0.4, 0.4, 0.4)
    ];

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event: winit::event::KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape),
                        ..
                    },
                    ..
                } => elwt.exit(),
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(physical_size.width, physical_size.height);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let size = window.inner_size();
                    let nx = position.x as f32 / size.width as f32;
                    let ny = position.y as f32 / size.height as f32;
                    cursor_velocity = [nx - cursor_pos[0], ny - cursor_pos[1]];
                    cursor_pos = [nx, ny];
                }
                WindowEvent::RedrawRequested => {
                    // Update surfaces with cursor info
                    let mut current_surfaces = surfaces.clone();
                    for s in current_surfaces.iter_mut() {
                        s.pointer_pos = cursor_pos;
                        s.pointer_velocity = cursor_velocity;
                    }
                    cursor_velocity[0] *= 0.9;
                    cursor_velocity[1] *= 0.9;
                    
                    match renderer.render(&current_surfaces) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => renderer.resize(window.inner_size().width, window.inner_size().height),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}

fn main() {
    pollster::block_on(run());
}
