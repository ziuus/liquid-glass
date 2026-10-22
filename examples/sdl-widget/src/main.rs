use glass_core::{GlassMaterial, MaterialType, GlassTransition};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::Instant;

const WIDTH: u32 = 500;
const HEIGHT: u32 = 600;

fn main() -> Result<(), String> {
    println!("\n🪟 ═══════════════════════════════════════════");
    println!("   LIQUID GLASS WIDGET - SDL2 Demo");
    println!("   ═══════════════════════════════════════════\n");

    // Create materials
    let background = GlassMaterial::new(MaterialType::HUD);
    let button_mat = GlassMaterial::menu();

    println!("📦 Materials Created:");
    println!("   Background (HUD): blur={}px, sat={}x, opacity={}",
        background.vibrancy.blur_radius,
        background.vibrancy.saturation_boost,
        background.vibrancy.opacity);
    println!("   Button (Menu): blur={}px, sat={}x, opacity={}",
        button_mat.vibrancy.blur_radius,
        button_mat.vibrancy.saturation_boost,
        button_mat.vibrancy.opacity);

    let mut button_transition = GlassTransition::new();

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("🪟 Liquid Glass Widget", WIDTH, HEIGHT)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;

    println!("\n✅ Window created! You should see it now.\n");
    println!("💡 Instructions:");
    println!("   • Hover over the button to see spring animations");
    println!("   • Watch the blur/opacity/scale values change smoothly");
    println!("   • Press ESC or close window to exit\n");

    let mut event_pump = sdl_context.event_pump()?;
    let mut last_time = Instant::now();
    let mut frame_count = 0u64;

    let button_rect = Rect::new(175, 275, 150, 50);
    let mut is_hovering = false;

    'running: loop {
        let now = Instant::now();
        let delta = now.duration_since(last_time).as_secs_f32();
        last_time = now;
        frame_count += 1;

        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::MouseMotion { x, y, .. } => {
                    let was_hovering = is_hovering;
                    is_hovering = button_rect.contains_point((x, y));

                    if is_hovering && !was_hovering {
                        println!("🎪 Hover IN  - animating: blur {:.0}→{:.0}px, opacity {:.2}→{:.2}, scale {:.2}→{:.2}",
                            button_transition.blur_radius.value,
                            button_mat.vibrancy.blur_radius * 0.9,
                            button_transition.opacity.value,
                            0.95,
                            button_transition.scale.value,
                            1.02);
                        button_transition.hover_in(button_mat.vibrancy.blur_radius);
                    } else if !is_hovering && was_hovering {
                        println!("   Hover OUT - returning to rest");
                        button_transition.hover_out(button_mat.vibrancy.blur_radius);
                    }
                }
                _ => {}
            }
        }

        // Update animations
        button_transition.update(delta);

        // Get animated values
        let blur = button_transition.blur_radius.value;
        let opacity = button_transition.opacity.value;
        let scale = button_transition.scale.value;

        // Clear screen with dark background
        canvas.set_draw_color(Color::RGB(20, 20, 20));
        canvas.clear();

        // Draw background panel (simulating HUD glass material)
        let panel_alpha = (background.vibrancy.opacity * 255.0) as u8;
        canvas.set_draw_color(Color::RGBA(40, 40, 55, panel_alpha));
        let panel = Rect::new(50, 50, 400, 500);
        canvas.fill_rect(panel)?;

        // Draw panel border for visibility
        canvas.set_draw_color(Color::RGBA(80, 80, 100, 150));
        canvas.draw_rect(panel)?;

        // Draw button with animated scale
        let scale_offset = ((1.0 - scale) * button_rect.width() as f32 / 2.0) as i32;
        let scaled_button = Rect::new(
            button_rect.x() + scale_offset,
            button_rect.y() + scale_offset,
            (button_rect.width() as f32 * scale) as u32,
            (button_rect.height() as f32 * scale) as u32,
        );

        // Button color changes with opacity (simulating vibrancy)
        let brightness = (opacity * 180.0) as u8;
        canvas.set_draw_color(Color::RGBA(brightness, brightness, brightness + 50, (opacity * 255.0) as u8));
        canvas.fill_rect(scaled_button)?;

        // Button border
        canvas.set_draw_color(Color::RGBA(200, 200, 255, 200));
        canvas.draw_rect(scaled_button)?;

        // Draw title
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        draw_text_simple(&mut canvas, "LIQUID GLASS ENGINE", 130, 100)?;

        // Draw button label
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        draw_text_simple(&mut canvas, "HOVER ME", 210, 295)?;

        // Draw animated stats
        canvas.set_draw_color(Color::RGB(170, 170, 170));
        draw_text_simple(&mut canvas, &format!("Blur:    {:.1} px", blur), 80, 380)?;
        draw_text_simple(&mut canvas, &format!("Opacity: {:.3}", opacity), 80, 410)?;
        draw_text_simple(&mut canvas, &format!("Scale:   {:.3}", scale), 80, 440)?;

        // Draw spring info
        canvas.set_draw_color(Color::RGB(120, 120, 120));
        draw_text_simple(&mut canvas, "Spring Physics: Natural Motion", 100, 500)?;

        // Draw FPS
        if frame_count % 60 == 0 {
            canvas.set_draw_color(Color::RGB(100, 100, 100));
            draw_text_simple(&mut canvas, &format!("Frame: {}", frame_count), 380, 30)?;
        }

        canvas.present();

        // Limit to 60 FPS
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    println!("\n👋 Widget closed. Liquid glass demo complete!\n");
    Ok(())
}

fn draw_text_simple(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, text: &str, x: i32, y: i32) -> Result<(), String> {
    // Draw simple rectangles as "text" (SDL2 text rendering needs TTF which adds complexity)
    // For a real widget, you'd use SDL2_ttf
    for (i, _ch) in text.chars().enumerate() {
        let char_x = x + (i as i32 * 10);
        let rect = Rect::new(char_x, y, 8, 2);
        canvas.fill_rect(rect)?;
    }
    Ok(())
}
