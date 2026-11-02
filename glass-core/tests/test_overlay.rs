use glass_core::font::{FontWeight, QuicksandFont};

#[test]
fn test_render_clock_and_todo_overlays() {
    let font = QuicksandFont::new();
    let brain_dir = "/home/zius/.gemini/antigravity-cli/brain/47486015-2027-4a74-9bb3-457d7cd0d76b";

    for is_light in [false, true] {
        let (col_primary, col_secondary, col_dimmed, col_accent, col_shadow) = if is_light {
            // Light wallpaper: crisp obsidian charcoal with subtle white specular highlight
            (
                [24, 28, 38, 245],
                [60, 68, 84, 225],
                [135, 145, 165, 190],
                [16, 20, 30, 255],
                [255, 255, 255, 160],
            )
        } else {
            // Dark/vibrant wallpaper: radiant pure white with ambient dark drop shadow
            (
                [255, 255, 255, 255],
                [235, 240, 250, 230],
                [190, 198, 212, 175],
                [255, 255, 255, 255],
                [10, 14, 22, 140],
            )
        };

        // 1. CLOCK OVERLAY (320 x 218)
        let (cw, ch) = (320u32, 218u32);
        let mut clock_buf = vec![0u8; (cw * ch * 4) as usize];

        // Day & Battery (cy = 53.0)
        font.draw_text_centered(
            &mut clock_buf,
            cw,
            ch,
            "MON",
            96.0,
            53.0,
            13.5,
            col_secondary,
            col_shadow,
            FontWeight::Bold,
        );
        font.draw_text_centered(
            &mut clock_buf,
            cw,
            ch,
            "60%",
            249.0,
            53.0,
            13.5,
            col_secondary,
            col_shadow,
            FontWeight::Bold,
        );

        // Hero Clock (cy = 105.0)
        let time_str = "01:46";
        let time_baseline = font.vertical_center_baseline(105.0, 52.0, FontWeight::Bold);
        let time_w = font.measure_text(time_str, 52.0, FontWeight::Bold);
        let time_x = 142.0 - time_w * 0.5;
        let end_x = font.draw_text_with_shadow(
            &mut clock_buf,
            cw,
            ch,
            time_str,
            time_x,
            time_baseline,
            52.0,
            col_primary,
            col_shadow,
            FontWeight::Bold,
        );
        font.draw_text_with_shadow(
            &mut clock_buf,
            cw,
            ch,
            "PM",
            end_x + 6.0,
            time_baseline - 4.0,
            14.0,
            col_secondary,
            col_shadow,
            FontWeight::Bold,
        );

        // Date (cy = 163.0)
        font.draw_text_centered(
            &mut clock_buf,
            cw,
            ch,
            "Nov 02",
            96.0,
            163.0,
            13.5,
            col_secondary,
            col_shadow,
            FontWeight::Bold,
        );

        let suffix = if is_light { "light" } else { "dark" };
        image::save_buffer(
            format!("{}/clock_overlay_{}.png", brain_dir, suffix),
            &clock_buf,
            cw,
            ch,
            image::ExtendedColorType::Rgba8,
        )
        .expect("Saved clock overlay");

        // 2. TODO OVERLAY (360 x 455)
        let (tw, th) = (360u32, 455u32);
        let mut todo_buf = vec![0u8; (tw * th * 4) as usize];

        // Tabs (cy = 32.5)
        font.draw_text_centered(
            &mut todo_buf,
            tw,
            th,
            "Tasks (6)",
            104.0,
            32.5,
            14.0,
            col_accent,
            col_shadow,
            FontWeight::Bold,
        );
        font.draw_text_centered(
            &mut todo_buf,
            tw,
            th,
            "Agenda",
            256.0,
            32.5,
            14.0,
            col_dimmed,
            col_shadow,
            FontWeight::SemiBold,
        );

        // Filters (cy = 79.5)
        font.draw_text_centered(
            &mut todo_buf,
            tw,
            th,
            "All",
            84.0,
            79.5,
            12.5,
            col_accent,
            col_shadow,
            FontWeight::SemiBold,
        );
        font.draw_text_centered(
            &mut todo_buf,
            tw,
            th,
            "Active",
            156.0,
            79.5,
            12.5,
            col_dimmed,
            col_shadow,
            FontWeight::SemiBold,
        );
        font.draw_text_centered(
            &mut todo_buf,
            tw,
            th,
            "Done",
            228.0,
            79.5,
            12.5,
            col_dimmed,
            col_shadow,
            FontWeight::SemiBold,
        );

        // Tasks (cy = 122.5 + row_idx * 44.0)
        let sample_tasks = [
            ("LEETCODE", false),
            ("LEARN", false),
            ("CERT", false),
            ("CAREER", false),
            ("HACKERRANK", false),
            ("AI & ML", true),
        ];

        for (row_idx, (title, done)) in sample_tasks.iter().enumerate() {
            let cy = 122.5 + (row_idx as f32) * 44.0;
            let (text_col, weight) = if *done {
                (col_dimmed, FontWeight::Regular)
            } else {
                (col_primary, FontWeight::SemiBold)
            };

            let end_x = font.draw_text_centered_y(
                &mut todo_buf,
                tw,
                th,
                title,
                64.0,
                cy,
                14.0,
                text_col,
                col_shadow,
                weight,
            );

            if *done {
                QuicksandFont::draw_strikethrough(
                    &mut todo_buf,
                    tw,
                    th,
                    63.0,
                    end_x + 2.0,
                    cy,
                    1.4,
                    col_dimmed,
                );
            }

            font.draw_text_centered(
                &mut todo_buf,
                tw,
                th,
                "DAILY",
                304.0,
                cy,
                10.0,
                col_secondary,
                col_shadow,
                FontWeight::Bold,
            );
        }

        // Add task (cy = 412.5)
        font.draw_text_centered_y(
            &mut todo_buf,
            tw,
            th,
            "+  Add task",
            42.0,
            412.5,
            14.0,
            col_secondary,
            col_shadow,
            FontWeight::SemiBold,
        );

        image::save_buffer(
            format!("{}/todo_overlay_{}.png", brain_dir, suffix),
            &todo_buf,
            tw,
            th,
            image::ExtendedColorType::Rgba8,
        )
        .expect("Saved todo overlay");
    }
}
