// Simple animation demo - no GUI required
use glass_core::{GlassMaterial, MaterialType, GlassTransition, Spring, SpringParams};
use std::thread;
use std::time::Duration;

fn main() {
    println!("\n🪟 ═══════════════════════════════════════════════════");
    println!("   LIQUID GLASS ENGINE - Animation Demo");
    println!("   ═══════════════════════════════════════════════════\n");

    // Show materials
    demo_materials();

    // Show spring animation
    demo_spring();

    // Show coordinated transition
    demo_transition();

    // Show presets
    demo_presets();

    println!("\n✨ ═══════════════════════════════════════════════════");
    println!("   Demo Complete! Your liquid glass engine is ready.");
    println!("   ═══════════════════════════════════════════════════\n");
}

fn demo_materials() {
    println!("📦 APPLE-STYLE MATERIALS");
    println!("   ─────────────────────\n");

    let materials = [
        ("WindowBackground", GlassMaterial::new(MaterialType::WindowBackground)),
        ("Sidebar", GlassMaterial::new(MaterialType::Sidebar)),
        ("Menu", GlassMaterial::menu()),
        ("Popover", GlassMaterial::new(MaterialType::Popover)),
        ("HUD", GlassMaterial::new(MaterialType::HUD)),
    ];

    for (name, mat) in &materials {
        println!("   {:18} │ blur:{:3.0}px │ sat:{:.1}x │ opacity:{:.2}",
            name,
            mat.vibrancy.blur_radius,
            mat.vibrancy.saturation_boost,
            mat.vibrancy.opacity);
    }
    println!();
}

fn demo_spring() {
    println!("🎪 SPRING PHYSICS ANIMATION");
    println!("   ───────────────────────\n");

    let mut opacity = Spring::with_params(1.0, SpringParams::default_ui());
    println!("   Target: 1.0 → 0.5\n");
    opacity.set_target(0.5);

    for frame in (0..60).step_by(10) {
        for _ in 0..10 {
            opacity.update(1.0 / 60.0);
        }
        print_progress_bar(frame, 60);
        println!(" Frame {:2}: opacity={:.3} velocity={:.3}",
            frame, opacity.value, opacity.velocity);
        thread::sleep(Duration::from_millis(100));
    }
    println!("\n   ✅ Settled at {:.3}\n", opacity.value);
}

fn demo_transition() {
    println!("🎨 COORDINATED GLASS TRANSITION");
    println!("   ────────────────────────────\n");

    let mut transition = GlassTransition::new();
    println!("   Simulating hover effect (blur + opacity + scale)...\n");
    transition.hover_in(60.0);

    for frame in (0..60).step_by(10) {
        for _ in 0..10 {
            transition.update(1.0 / 60.0);
        }

        print_progress_bar(frame, 60);
        println!(" blur:{:4.1}px │ opacity:{:.2} │ scale:{:.3}",
            transition.blur_radius.value,
            transition.opacity.value,
            transition.scale.value);
        thread::sleep(Duration::from_millis(100));
    }

    println!("\n   ✅ Final: blur={:.1}px, opacity={:.2}, scale={:.3}\n",
        transition.blur_radius.value,
        transition.opacity.value,
        transition.scale.value);
}

fn demo_presets() {
    println!("🌈 SPRING PRESETS");
    println!("   ──────────────\n");

    let presets = [
        ("Bouncy", SpringParams::bouncy(), "🎾"),
        ("Smooth", SpringParams::smooth(), "🧈"),
        ("Snappy", SpringParams::snappy(), "⚡"),
        ("Gentle", SpringParams::gentle(), "🌸"),
    ];

    for (name, params, emoji) in &presets {
        let mut spring = Spring::with_params(0.0, *params);
        spring.set_target(100.0);

        let mut frames = 0;
        while !spring.is_at_rest() && frames < 300 {
            spring.update(1.0 / 60.0);
            frames += 1;
        }

        let bar_len = (frames as f32 / 300.0 * 20.0) as usize;
        let bar: String = "█".repeat(bar_len) + &"░".repeat(20 - bar_len);

        println!("   {} {:8} │ {} │ {} frames ({:.1}s)",
            emoji, name, bar, frames, frames as f32 / 60.0);
    }
    println!();
}

fn print_progress_bar(current: usize, total: usize) {
    let percent = (current as f32 / total as f32 * 100.0) as usize;
    let filled = percent / 5;
    let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);
    print!("   [{}]", bar);
}
