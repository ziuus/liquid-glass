use winit::{
    event::*,
    event_loop::EventLoop,
    window::WindowBuilder,
};
use glass_core::{GlassRenderer, GlassSurface, Material, geometry::Shape};
use std::sync::Arc;
use std::time::Instant;

async fn run() {
    env_logger::init();
    
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new()
        .with_title("Liquid Glass Benchmark")
        .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0))
        .build(&event_loop)
        .unwrap());

    let size = window.inner_size();
    let mut renderer = GlassRenderer::new(window.clone(), size.width, size.height).await.unwrap();

    let counts = [1, 5, 10, 25, 50, 100];
    let mut current_count_idx = 0;
    
    let generate_surfaces = |count: usize| {
        let mut surfaces = Vec::with_capacity(count);
        for i in 0..count {
            let x = (i % 10) as f32 * 0.1;
            let y = (i / 10) as f32 * 0.1;
            surfaces.push(
                GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::RoundedRectangle { radius: 10.0 })
                    .bounds(x, y, 0.08, 0.08) // 8% of screen size each
            );
        }
        surfaces
    };

    let mut surfaces = generate_surfaces(counts[current_count_idx]);
    
    let mut frames = 0;
    let mut last_log = Instant::now();
    let start_time = Instant::now();
    
    println!("Starting benchmark. Surfaces: {}", counts[current_count_idx]);

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
                WindowEvent::KeyboardInput {
                    event: winit::event::KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space),
                        ..
                    },
                    ..
                } => {
                    current_count_idx = (current_count_idx + 1) % counts.len();
                    surfaces = generate_surfaces(counts[current_count_idx]);
                    println!("Switched to {} surfaces", counts[current_count_idx]);
                }
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(physical_size.width, physical_size.height);
                }
                WindowEvent::RedrawRequested => {
                    frames += 1;
                    let now = Instant::now();
                    if now.duration_since(last_log).as_secs() >= 1 {
                        let elapsed = now.duration_since(last_log).as_secs_f64();
                        let fps = frames as f64 / elapsed;
                        let frame_time = 1000.0 / fps;
                        println!("Surfaces: {:3} | FPS: {:6.1} | Frame Time: {:5.2} ms", counts[current_count_idx], fps, frame_time);
                        frames = 0;
                        last_log = now;
                    }
                    
                    match renderer.render(&surfaces) {
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
                
                // Auto advance benchmark every 5 seconds for automated testing
                if start_time.elapsed().as_secs() > ((current_count_idx + 1) * 5) as u64 && current_count_idx < counts.len() - 1 {
                    current_count_idx += 1;
                    surfaces = generate_surfaces(counts[current_count_idx]);
                    println!("Auto-advancing to {} surfaces", counts[current_count_idx]);
                } else if start_time.elapsed().as_secs() > (counts.len() * 5 + 2) as u64 {
                    println!("Benchmark complete. Exiting.");
                    elwt.exit();
                }
            }
            _ => {}
        }
    }).unwrap();
}

fn main() {
    pollster::block_on(run());
}
