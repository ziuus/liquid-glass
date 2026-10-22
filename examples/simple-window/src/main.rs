use winit::{
    event::*,
    event_loop::EventLoop,
    window::WindowBuilder,
};
use glass_core::GlassRenderer;
use glass_ui::{GlassWidget, GlassCard, GlassButton, GlassToggle, GlassSlider};
use std::sync::Arc;
use std::time::Instant;

async fn run() {
    env_logger::init();
    
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new()
        .with_title("Liquid Glass UI Components")
        .with_inner_size(winit::dpi::LogicalSize::new(900.0, 640.0))
        .with_transparent(true)
        .build(&event_loop)
        .unwrap());

    let size = window.inner_size();
    let mut renderer = GlassRenderer::new(window.clone(), size.width, size.height).await.unwrap();

    let mut cursor_pos = [0.5, 0.5];
    let mut cursor_velocity = [0.0, 0.0];

    // Build the UI using glass-ui components
    let mut widget = GlassWidget::new();

    // 1. Background Island
    widget.add_card(GlassCard::island(0.08, 0.08, 0.84, 0.84).blur(32.0).specular(1.0));

    // 2. Interactive Glass Buttons
    widget.add_button(GlassButton::new(0.14, 0.22, 0.20, 0.10, "Play"));
    widget.add_button(GlassButton::new(0.38, 0.22, 0.20, 0.10, "Shuffle"));
    widget.add_button(GlassButton::new(0.62, 0.22, 0.20, 0.10, "Favorite"));

    // 3. Animated Glass Toggle Switch
    widget.add_toggle(GlassToggle::new(0.14, 0.42, 0.16, 0.08, true));
    widget.add_toggle(GlassToggle::new(0.38, 0.42, 0.16, 0.08, false));

    // 4. Interactive Scrubber Slider
    widget.add_slider(GlassSlider::new(0.14, 0.62, 0.68, 0.04, 0.45));

    let mut last_frame = Instant::now();

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

                    widget.on_pointer_move(nx, ny);
                }
                WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                    if *state == ElementState::Pressed {
                        widget.on_pointer_down(cursor_pos[0], cursor_pos[1]);
                    } else {
                        if let Some(clicked_idx) = widget.on_pointer_up(cursor_pos[0], cursor_pos[1]) {
                            println!("Button [{}] clicked!", clicked_idx);
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let dt = last_frame.elapsed().as_secs_f32().min(0.05);
                    last_frame = Instant::now();

                    // Advance physics & animations
                    widget.update(dt);

                    let mut surfaces = widget.collect_surfaces();
                    for s in surfaces.iter_mut() {
                        s.pointer_pos = cursor_pos;
                        s.pointer_velocity = cursor_velocity;
                    }
                    cursor_velocity[0] *= 0.85;
                    cursor_velocity[1] *= 0.85;

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
            }
            _ => {}
        }
    }).unwrap();
}

fn main() {
    pollster::block_on(run());
}
