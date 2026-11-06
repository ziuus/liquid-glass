use glass_core::font::{FontWeight, QuicksandFont};

#[test]
fn test_render_clock_and_todo_overlays() {
    let font = QuicksandFont::new();
    let target_dir = {
        let brain = std::path::Path::new(
            "/home/zius/.gemini/antigravity-cli/brain/64634fbc-40f0-4ff7-b05d-73575fd4890c",
        );
        if brain.exists() {
            brain.to_path_buf()
        } else {
            std::env::temp_dir()
        }
    };

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
        let _ = image::save_buffer(
            format!("{}/clock_overlay_{}.png", target_dir.display(), suffix),
            &clock_buf,
            cw,
            ch,
            image::ExtendedColorType::Rgba8,
        );

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

        let _ = image::save_buffer(
            format!("{}/todo_overlay_{}.png", target_dir.display(), suffix),
            &todo_buf,
            tw,
            th,
            image::ExtendedColorType::Rgba8,
        );
    }
}

#[test]
fn test_render_slide_carousel_and_lucide_overlays() {
    use glass_core::font::UIFont;
    use glass_core::Tokens;

    let font = UIFont::new();
    let brain_dir = std::path::Path::new(
        "/home/zius/.gemini/antigravity-cli/brain/64634fbc-40f0-4ff7-b05d-73575fd4890c",
    );
    let target_dir = if brain_dir.exists() {
        brain_dir.to_path_buf()
    } else {
        std::env::temp_dir()
    };

    // ─────────────────────────────────────────────────────────────
    // 1. TODO SLIDING CAROUSEL (Tasks -> Mid -> Agenda)
    // ─────────────────────────────────────────────────────────────
    let (tw, th) = (360u32, 455u32);
    for (t_name, t_val) in [("tasks", 0.0f32), ("mid", 0.5f32), ("agenda", 1.0f32)] {
        let mut buf = vec![0u8; (tw * th * 4) as usize];
        let card_h = 320.0f32;

        // Card Substrate
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            tw,
            th,
            180.0,
            card_h * 0.5,
            178.0,
            card_h * 0.5 - 2.0,
            Tokens::RADIUS_OUTER,
            [16, 20, 28, 230],
        );
        UIFont::draw_rounded_rect_stroke(
            &mut buf,
            tw,
            th,
            180.0,
            card_h * 0.5,
            178.0,
            card_h * 0.5 - 2.0,
            Tokens::RADIUS_OUTER,
            1.0,
            Tokens::RIM_OUTER_DARK,
        );

        // Header Tabs
        let tab_x = 104.0 + t_val * 152.0;
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            tw,
            th,
            tab_x,
            32.5,
            48.0,
            14.0,
            10.0,
            [255, 255, 255, 30],
        );
        font.draw_text_centered(
            &mut buf,
            tw,
            th,
            "Tasks",
            104.0,
            32.5,
            15.5,
            if t_val < 0.5 {
                Tokens::TEXT_PRIMARY
            } else {
                Tokens::TEXT_SECONDARY
            },
            [0, 0, 0, 0],
            FontWeight::Bold,
        );
        font.draw_text_centered(
            &mut buf,
            tw,
            th,
            "Agenda",
            256.0,
            32.5,
            15.5,
            if t_val > 0.5 {
                Tokens::TEXT_PRIMARY
            } else {
                Tokens::TEXT_SECONDARY
            },
            [0, 0, 0, 0],
            FontWeight::Bold,
        );

        // Settings gear button
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            tw,
            th,
            330.0,
            32.5,
            13.0,
            13.0,
            8.0,
            Tokens::INNER_TILE_BG_DARK,
        );
        UIFont::draw_settings_icon(&mut buf, tw, th, 330.0, 32.5, Tokens::TEXT_SECONDARY);

        // Sliding offsets
        let tasks_offset_x = -t_val * (tw as f32);
        let agenda_offset_x = (1.0 - t_val) * (tw as f32);

        // Render Tasks view
        if tasks_offset_x > -(tw as f32) + 2.0 {
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "All",
                84.0 + tasks_offset_x,
                79.5,
                14.0,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "Active",
                156.0 + tasks_offset_x,
                79.5,
                14.0,
                Tokens::TEXT_SECONDARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "Done",
                228.0 + tasks_offset_x,
                79.5,
                14.0,
                Tokens::TEXT_SECONDARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );

            let demo_tasks = [
                ("Design system review", false),
                ("Slide animation physics", true),
                ("Obsidian glass tuning", false),
            ];
            for (idx, (txt, done)) in demo_tasks.iter().enumerate() {
                let cy = 122.5 + (idx as f32) * 44.0;
                let chk_cx = 44.0 + tasks_offset_x;
                if *done {
                    UIFont::draw_rounded_rect_fill(
                        &mut buf,
                        tw,
                        th,
                        chk_cx,
                        cy,
                        9.0,
                        9.0,
                        5.0,
                        [255, 255, 255, 245],
                    );
                    UIFont::draw_checkmark(
                        &mut buf,
                        tw,
                        th,
                        chk_cx,
                        cy,
                        1.1,
                        1.6,
                        [10, 14, 22, 255],
                    );
                } else {
                    UIFont::draw_rounded_rect_stroke(
                        &mut buf,
                        tw,
                        th,
                        chk_cx,
                        cy,
                        9.0,
                        9.0,
                        5.0,
                        1.3,
                        [255, 255, 255, 230],
                    );
                }
                font.draw_text_centered_y(
                    &mut buf,
                    tw,
                    th,
                    txt,
                    68.0 + tasks_offset_x,
                    cy,
                    14.5,
                    Tokens::TEXT_PRIMARY,
                    [0, 0, 0, 0],
                    FontWeight::Bold,
                );
            }

            // Bottom Add Task Bar
            let add_cy = 254.5f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                tw,
                th,
                180.0 + tasks_offset_x,
                add_cy,
                156.0,
                17.0,
                17.0,
                Tokens::INNER_TILE_BG_DARK,
            );
            font.draw_text_centered_y(
                &mut buf,
                tw,
                th,
                "Add task",
                42.0 + tasks_offset_x,
                add_cy,
                14.5,
                Tokens::TEXT_SECONDARY,
                [0, 0, 0, 0],
                FontWeight::Medium,
            );
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                tw,
                th,
                315.0 + tasks_offset_x,
                add_cy,
                11.0,
                11.0,
                11.0,
                Tokens::INNER_TILE_BG_DARK,
            );
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "+",
                315.0 + tasks_offset_x,
                add_cy,
                16.0,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
        }

        // Render Agenda view
        if agenda_offset_x < (tw as f32) - 2.0 {
            let demo_events = [
                ("10:00", "Engineering Sync", "TODAY"),
                ("14:30", "Product Roadmap", "TODAY"),
                ("17:00", "Design Review", "EVENT"),
            ];
            for (idx, (tm, title, tag)) in demo_events.iter().enumerate() {
                let cy = 122.5 + (idx as f32) * 44.0;
                font.draw_text_centered(
                    &mut buf,
                    tw,
                    th,
                    tm,
                    64.0 + agenda_offset_x,
                    cy,
                    11.0,
                    Tokens::TEXT_SECONDARY,
                    [0, 0, 0, 0],
                    FontWeight::Medium,
                );
                font.draw_text_centered_y(
                    &mut buf,
                    tw,
                    th,
                    title,
                    108.0 + agenda_offset_x,
                    cy,
                    13.5,
                    Tokens::TEXT_PRIMARY,
                    [0, 0, 0, 0],
                    FontWeight::SemiBold,
                );
                font.draw_text_centered(
                    &mut buf,
                    tw,
                    th,
                    tag,
                    304.0 + agenda_offset_x,
                    cy,
                    10.0,
                    Tokens::TEXT_TERTIARY,
                    [0, 0, 0, 0],
                    FontWeight::Bold,
                );
            }

            // Bottom Sync Bar
            let add_cy = 254.5f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                tw,
                th,
                64.0 + agenda_offset_x,
                add_cy,
                32.0,
                12.0,
                8.0,
                Tokens::INNER_TILE_BG_DARK,
            );
            UIFont::draw_sync_icon(
                &mut buf,
                tw,
                th,
                46.0 + agenda_offset_x,
                add_cy,
                Tokens::TEXT_PRIMARY,
            );
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "Sync",
                72.0 + agenda_offset_x,
                add_cy,
                11.5,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            font.draw_text_centered(
                &mut buf,
                tw,
                th,
                "3 Events",
                175.0 + agenda_offset_x,
                add_cy,
                12.0,
                Tokens::TEXT_SECONDARY,
                [0, 0, 0, 0],
                FontWeight::SemiBold,
            );
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                tw,
                th,
                308.0 + agenda_offset_x,
                add_cy,
                15.0,
                12.0,
                8.0,
                Tokens::INNER_TILE_BG_DARK,
            );
            UIFont::draw_settings_icon(
                &mut buf,
                tw,
                th,
                308.0 + agenda_offset_x,
                add_cy,
                Tokens::TEXT_PRIMARY,
            );
        }

        let _ = image::save_buffer(
            format!("{}/todo_slide_{}.png", target_dir.display(), t_name),
            &buf,
            tw,
            th,
            image::ExtendedColorType::Rgba8,
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 2. ISLAND EXPANDED MODES (460 x 120 Obsidian Liquid Glass)
    // ─────────────────────────────────────────────────────────────
    let (iw, ih) = (460u32, 120u32);
    let island_cx = 230.0f32;
    let _island_top = 0.0f32;
    let island_cy = 60.0f32;

    // A. Music Expanded
    {
        let mut buf = vec![0u8; (iw * ih * 4) as usize];
        UIFont::draw_rounded_rect_stroke(
            &mut buf,
            iw,
            ih,
            island_cx,
            island_cy,
            228.0,
            58.0,
            Tokens::RADIUS_OUTER,
            1.0,
            Tokens::RIM_OUTER_DARK,
        );
        UIFont::draw_segment(
            &mut buf,
            iw,
            ih,
            (island_cx - 200.0, 1.2),
            (island_cx + 200.0, 1.2),
            1.0,
            Tokens::INNER_TOP_HIGHLIGHT_DARK,
        );

        font.draw_text(
            &mut buf,
            iw,
            ih,
            "NOW PLAYING",
            32.0,
            22.0,
            11.0,
            Tokens::TEXT_TERTIARY,
            FontWeight::Bold,
        );
        // Vinyl (clean monochrome)
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            50.0,
            68.0,
            23.0,
            23.0,
            23.0,
            [235, 240, 252, 255],
        );
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            50.0,
            68.0,
            9.0,
            9.0,
            9.0,
            [16, 20, 28, 255],
        );
        font.draw_text(
            &mut buf,
            iw,
            ih,
            "Starboy",
            88.0,
            58.0,
            Tokens::TYPE_BODY,
            Tokens::TEXT_PRIMARY,
            FontWeight::Bold,
        );
        font.draw_text(
            &mut buf,
            iw,
            ih,
            "The Weeknd",
            88.0,
            78.0,
            Tokens::TYPE_BODY_SM,
            Tokens::TEXT_SECONDARY,
            FontWeight::Medium,
        );

        // Transport
        let ctrl_y = 68.0f32;
        let play_cx = iw as f32 - 64.0;
        UIFont::draw_prev_icon(
            &mut buf,
            iw,
            ih,
            play_cx - 44.0,
            ctrl_y,
            8.5,
            Tokens::TEXT_PRIMARY,
        );
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            play_cx,
            ctrl_y,
            18.0,
            18.0,
            18.0,
            [245, 248, 255, 255],
        );
        UIFont::draw_pause_icon(&mut buf, iw, ih, play_cx, ctrl_y, 7.5, [10, 14, 20, 255]);
        UIFont::draw_next_icon(
            &mut buf,
            iw,
            ih,
            play_cx + 44.0,
            ctrl_y,
            8.5,
            Tokens::TEXT_PRIMARY,
        );

        // Progress bar
        let bar_y = 104.0f32;
        let bar_w = iw as f32 - 64.0;
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            island_cx,
            bar_y,
            bar_w * 0.5,
            2.0,
            2.0,
            Tokens::INNER_TILE_BG_DARK,
        );
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            island_cx - bar_w * 0.5 + bar_w * 0.42 * 0.5,
            bar_y,
            bar_w * 0.42 * 0.5,
            2.0,
            2.0,
            Tokens::TEXT_PRIMARY,
        );
        UIFont::draw_rounded_rect_fill(
            &mut buf,
            iw,
            ih,
            island_cx - bar_w * 0.5 + bar_w * 0.42,
            bar_y,
            4.0,
            4.0,
            4.0,
            [255, 255, 255, 255],
        );

        let _ = image::save_buffer(
            format!("{}/island_slide_music.png", target_dir.display()),
            &buf,
            iw,
            ih,
            image::ExtendedColorType::Rgba8,
        );
    }

    // B. System Overview
    {
        let mut buf = vec![0u8; (iw * ih * 4) as usize];
        UIFont::draw_rounded_rect_stroke(
            &mut buf,
            iw,
            ih,
            island_cx,
            island_cy,
            228.0,
            58.0,
            Tokens::RADIUS_OUTER,
            1.0,
            Tokens::RIM_OUTER_DARK,
        );
        font.draw_text(
            &mut buf,
            iw,
            ih,
            "SYSTEM OVERVIEW",
            32.0,
            24.0,
            Tokens::TYPE_CAPTION,
            Tokens::TEXT_TERTIARY,
            FontWeight::Bold,
        );

        let items = [
            ("CPU", "24%"),
            ("RAM", "7.2G"),
            ("BATTERY", "85%"),
            ("UPTIME", "4h 12m"),
        ];
        let item_w = (iw as f32 - 52.0) / 4.0;
        let start_x = 26.0 + item_w * 0.5;
        for (idx, (lbl, val)) in items.iter().enumerate() {
            let cx = start_x + (idx as f32) * item_w;
            let cy = 65.0f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                iw,
                ih,
                cx,
                cy,
                (item_w - 10.0) * 0.5,
                28.0,
                Tokens::RADIUS_INNER_TILE,
                Tokens::INNER_TILE_BG_DARK,
            );
            UIFont::draw_segment(
                &mut buf,
                iw,
                ih,
                (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                1.0,
                Tokens::INNER_TOP_HIGHLIGHT_DARK,
            );

            match idx {
                0 => UIFont::draw_cpu_icon(&mut buf, iw, ih, cx, cy - 12.0, Tokens::TEXT_PRIMARY),
                1 => UIFont::draw_ram_icon(&mut buf, iw, ih, cx, cy - 12.0, Tokens::TEXT_PRIMARY),
                2 => UIFont::draw_battery_icon(
                    &mut buf,
                    iw,
                    ih,
                    cx,
                    cy - 12.0,
                    85u32,
                    true,
                    Tokens::TEXT_PRIMARY,
                ),
                _ => UIFont::draw_clock_icon(&mut buf, iw, ih, cx, cy - 12.0, Tokens::TEXT_PRIMARY),
            }
            font.draw_text_centered(
                &mut buf,
                iw,
                ih,
                val,
                cx,
                cy + 3.0,
                Tokens::TYPE_BODY,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::Bold,
            );
            font.draw_text_centered(
                &mut buf,
                iw,
                ih,
                lbl,
                cx,
                cy + 17.0,
                9.0,
                Tokens::TEXT_TERTIARY,
                [0, 0, 0, 0],
                FontWeight::SemiBold,
            );
        }
        let _ = image::save_buffer(
            format!("{}/island_slide_system.png", target_dir.display()),
            &buf,
            iw,
            ih,
            image::ExtendedColorType::Rgba8,
        );
    }

    // C. Quick Access
    {
        let mut buf = vec![0u8; (iw * ih * 4) as usize];
        UIFont::draw_rounded_rect_stroke(
            &mut buf,
            iw,
            ih,
            island_cx,
            island_cy,
            228.0,
            58.0,
            Tokens::RADIUS_OUTER,
            1.0,
            Tokens::RIM_OUTER_DARK,
        );
        font.draw_text(
            &mut buf,
            iw,
            ih,
            "QUICK ACCESS",
            32.0,
            24.0,
            11.0,
            Tokens::TEXT_TERTIARY,
            FontWeight::Bold,
        );

        let apps = ["Terminal", "Browser", "Editor", "Files", "Music"];
        let item_w = (iw as f32 - 50.0) / (apps.len() as f32);
        let start_x = 25.0 + item_w * 0.5;
        for (idx, name) in apps.iter().enumerate() {
            let cx = start_x + (idx as f32) * item_w;
            let cy = 65.0f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                iw,
                ih,
                cx,
                cy,
                (item_w - 10.0) * 0.5,
                28.0,
                Tokens::RADIUS_INNER_TILE,
                Tokens::INNER_TILE_BG_DARK,
            );
            UIFont::draw_segment(
                &mut buf,
                iw,
                ih,
                (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                1.0,
                Tokens::INNER_TOP_HIGHLIGHT_DARK,
            );

            match idx {
                0 => {
                    UIFont::draw_terminal_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY)
                }
                1 => {
                    UIFont::draw_browser_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY)
                }
                2 => UIFont::draw_editor_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
                3 => UIFont::draw_folder_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
                _ => UIFont::draw_music_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
            }
            font.draw_text_centered(
                &mut buf,
                iw,
                ih,
                name,
                cx,
                cy + 13.0,
                Tokens::TYPE_CAPTION,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::Medium,
            );
        }
        let _ = image::save_buffer(
            format!("{}/island_slide_apps.png", target_dir.display()),
            &buf,
            iw,
            ih,
            image::ExtendedColorType::Rgba8,
        );
    }

    // D. Power Menu (with Hold-to-confirm)
    {
        let mut buf = vec![0u8; (iw * ih * 4) as usize];
        UIFont::draw_rounded_rect_stroke(
            &mut buf,
            iw,
            ih,
            island_cx,
            island_cy,
            228.0,
            58.0,
            Tokens::RADIUS_OUTER,
            1.0,
            Tokens::RIM_OUTER_DARK,
        );
        font.draw_text(
            &mut buf,
            iw,
            ih,
            "SESSION & POWER",
            32.0,
            24.0,
            Tokens::TYPE_CAPTION,
            Tokens::TEXT_TERTIARY,
            FontWeight::Bold,
        );

        let actions = ["Lock", "Suspend", "Restart", "Power Off"];
        let item_w = (iw as f32 - 52.0) / 4.0;
        let start_x = 26.0 + item_w * 0.5;
        for (idx, name) in actions.iter().enumerate() {
            let cx = start_x + (idx as f32) * item_w;
            let cy = 65.0f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                iw,
                ih,
                cx,
                cy,
                (item_w - 10.0) * 0.5,
                28.0,
                Tokens::RADIUS_INNER_TILE,
                Tokens::INNER_TILE_BG_DARK,
            );
            UIFont::draw_segment(
                &mut buf,
                iw,
                ih,
                (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                1.0,
                Tokens::INNER_TOP_HIGHLIGHT_DARK,
            );

            match idx {
                0 => UIFont::draw_lock_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
                1 => UIFont::draw_moon_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
                2 => {
                    UIFont::draw_restart_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY)
                }
                _ => UIFont::draw_power_icon(&mut buf, iw, ih, cx, cy - 8.0, Tokens::TEXT_PRIMARY),
            }
            font.draw_text_centered(
                &mut buf,
                iw,
                ih,
                name,
                cx,
                cy + 12.0,
                Tokens::TYPE_CAPTION,
                Tokens::TEXT_PRIMARY,
                [0, 0, 0, 0],
                FontWeight::SemiBold,
            );
        }
        let _ = image::save_buffer(
            format!("{}/island_slide_power.png", target_dir.display()),
            &buf,
            iw,
            ih,
            image::ExtendedColorType::Rgba8,
        );
    }
}

#[test]
fn test_render_light_and_dark_island_modes() {
    use glass_core::font::UIFont;
    use glass_core::Tokens;

    let font = UIFont::new();
    let brain_dir = std::path::Path::new(
        "/home/zius/.gemini/antigravity-cli/brain/64634fbc-40f0-4ff7-b05d-73575fd4890c",
    );
    let target_dir = if brain_dir.exists() {
        brain_dir.to_path_buf()
    } else {
        std::env::temp_dir()
    };

    let (iw, ih) = (460u32, 120u32);
    let island_cx = 230.0f32;
    let island_cy = 60.0f32;

    for is_light in [false, true] {
        let mode_str = if is_light { "light" } else { "dark" };
        let col_ink = if is_light {
            Tokens::TEXT_PRIMARY_LIGHT
        } else {
            Tokens::TEXT_PRIMARY
        };
        let col_dim = if is_light {
            Tokens::TEXT_SECONDARY_LIGHT
        } else {
            Tokens::TEXT_SECONDARY
        };
        let col_shadow = if is_light {
            Tokens::TEXT_SHADOW_LIGHT
        } else {
            Tokens::TEXT_SHADOW
        };

        // 1. COMPACT PILL (230 x 38)
        {
            let (pw, ph) = (230u32, 38u32);
            let mut buf = vec![0u8; (pw * ph * 4) as usize];
            let pcx = (pw as f32) * 0.5;
            let pcy = (ph as f32) * 0.5;

            // Outer Rim
            UIFont::draw_rounded_rect_stroke(
                &mut buf,
                pw,
                ph,
                pcx,
                pcy,
                pcx - 1.0,
                pcy - 1.0,
                pcy,
                1.0,
                if is_light {
                    Tokens::RIM_OUTER_LIGHT
                } else {
                    Tokens::RIM_OUTER_DARK
                },
            );

            // Left music icon disc
            let disc_cx = 22.0f32;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                pw,
                ph,
                disc_cx,
                pcy,
                11.0,
                11.0,
                11.0,
                if is_light {
                    Tokens::INNER_TILE_BG_LIGHT
                } else {
                    Tokens::INNER_TILE_BG_DARK
                },
            );
            UIFont::draw_music_icon(&mut buf, pw, ph, disc_cx, pcy, col_dim);

            // Title
            font.draw_text_centered(
                &mut buf,
                pw,
                ph,
                "No Media Playing",
                pcx,
                pcy,
                Tokens::TYPE_BODY_SM,
                col_ink,
                col_shadow,
                FontWeight::SemiBold,
            );

            // Right minimalist pause bars
            let right_x = pw as f32 - 22.0;
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                pw,
                ph,
                right_x - 3.0,
                pcy,
                1.2,
                4.5,
                1.0,
                col_dim,
            );
            UIFont::draw_rounded_rect_fill(
                &mut buf,
                pw,
                ph,
                right_x + 3.0,
                pcy,
                1.2,
                4.5,
                1.0,
                col_dim,
            );

            let _ = image::save_buffer(
                format!("{}/island_{}_compact.png", target_dir.display(), mode_str),
                &buf,
                pw,
                ph,
                image::ExtendedColorType::Rgba8,
            );
        }

        // 2. POWER MENU (460 x 120)
        {
            let mut buf = vec![0u8; (iw * ih * 4) as usize];
            UIFont::draw_rounded_rect_stroke(
                &mut buf,
                iw,
                ih,
                island_cx,
                island_cy,
                228.0,
                58.0,
                Tokens::RADIUS_OUTER,
                1.0,
                if is_light {
                    Tokens::RIM_OUTER_LIGHT
                } else {
                    Tokens::RIM_OUTER_DARK
                },
            );

            // Header
            font.draw_text(
                &mut buf,
                iw,
                ih,
                "SESSION & POWER",
                32.0,
                24.0,
                Tokens::TYPE_CAPTION,
                col_dim,
                FontWeight::Bold,
            );

            // Pager dots at top right
            for i in 0..4 {
                let dx = iw as f32 - 65.0 + (i as f32) * 11.0;
                let (half_r, col) = if i == 3 {
                    (
                        3.2f32,
                        if is_light {
                            Tokens::TEXT_PRIMARY_LIGHT
                        } else {
                            Tokens::TEXT_PRIMARY
                        },
                    )
                } else {
                    (
                        2.0f32,
                        if is_light {
                            [140, 150, 165, 180]
                        } else {
                            [255, 255, 255, 90]
                        },
                    )
                };
                UIFont::draw_rounded_rect_fill(
                    &mut buf, iw, ih, dx, 22.0, half_r, half_r, half_r, col,
                );
            }

            let actions = [
                ("Lock", Tokens::ACCENT),
                ("Suspend", Tokens::ACCENT),
                ("Restart", Tokens::STATUS_AMBER),
                ("Power Off", Tokens::STATUS_RED),
            ];
            let item_w = (iw as f32 - 52.0) / 4.0;
            let start_x = 26.0 + item_w * 0.5;
            for (idx, (name, _col)) in actions.iter().enumerate() {
                let cx = start_x + (idx as f32) * item_w;
                let cy = 65.0f32;
                UIFont::draw_rounded_rect_fill(
                    &mut buf,
                    iw,
                    ih,
                    cx,
                    cy,
                    (item_w - 10.0) * 0.5,
                    28.0,
                    Tokens::RADIUS_INNER_TILE,
                    if is_light {
                        Tokens::INNER_TILE_BG_LIGHT
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    },
                );
                UIFont::draw_segment(
                    &mut buf,
                    iw,
                    ih,
                    (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                    (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                    1.0,
                    if is_light {
                        Tokens::INNER_TOP_HIGHLIGHT_LIGHT
                    } else {
                        Tokens::INNER_TOP_HIGHLIGHT_DARK
                    },
                );

                match idx {
                    0 => UIFont::draw_lock_icon(&mut buf, iw, ih, cx, cy - 8.0, col_ink),
                    1 => UIFont::draw_moon_icon(&mut buf, iw, ih, cx, cy - 8.0, col_ink),
                    2 => UIFont::draw_restart_icon(&mut buf, iw, ih, cx, cy - 8.0, col_ink),
                    _ => UIFont::draw_power_icon(&mut buf, iw, ih, cx, cy - 8.0, col_ink),
                }
                font.draw_text_centered(
                    &mut buf,
                    iw,
                    ih,
                    name,
                    cx,
                    cy + 12.0,
                    Tokens::TYPE_CAPTION,
                    col_ink,
                    col_shadow,
                    FontWeight::SemiBold,
                );
            }
            let _ = image::save_buffer(
                format!("{}/island_{}_power.png", target_dir.display(), mode_str),
                &buf,
                iw,
                ih,
                image::ExtendedColorType::Rgba8,
            );
        }

        // 3. QUICK ACCESS (460 x 120)
        {
            let mut buf = vec![0u8; (iw * ih * 4) as usize];
            UIFont::draw_rounded_rect_stroke(
                &mut buf,
                iw,
                ih,
                island_cx,
                island_cy,
                228.0,
                58.0,
                Tokens::RADIUS_OUTER,
                1.0,
                if is_light {
                    Tokens::RIM_OUTER_LIGHT
                } else {
                    Tokens::RIM_OUTER_DARK
                },
            );

            font.draw_text(
                &mut buf,
                iw,
                ih,
                "QUICK ACCESS",
                32.0,
                24.0,
                11.0,
                col_dim,
                FontWeight::Bold,
            );

            let apps = ["Terminal", "Browser", "Editor", "Files", "Music"];
            let item_w = (iw as f32 - 50.0) / (apps.len() as f32);
            let start_x = 25.0 + item_w * 0.5;
            for (idx, name) in apps.iter().enumerate() {
                let cx = start_x + (idx as f32) * item_w;
                let cy = 65.0f32;
                UIFont::draw_rounded_rect_fill(
                    &mut buf,
                    iw,
                    ih,
                    cx,
                    cy,
                    (item_w - 10.0) * 0.5,
                    28.0,
                    Tokens::RADIUS_INNER_TILE,
                    if is_light {
                        Tokens::INNER_TILE_BG_LIGHT
                    } else {
                        Tokens::INNER_TILE_BG_DARK
                    },
                );
                UIFont::draw_segment(
                    &mut buf,
                    iw,
                    ih,
                    (cx - (item_w - 10.0) * 0.5 + 6.0, cy - 27.0),
                    (cx + (item_w - 10.0) * 0.5 - 6.0, cy - 27.0),
                    1.0,
                    if is_light {
                        Tokens::INNER_TOP_HIGHLIGHT_LIGHT
                    } else {
                        Tokens::INNER_TOP_HIGHLIGHT_DARK
                    },
                );

                match idx {
                    0 => UIFont::draw_terminal_icon(&mut buf, iw, ih, cx, cy - 7.0, col_ink),
                    1 => UIFont::draw_browser_icon(&mut buf, iw, ih, cx, cy - 7.0, col_ink),
                    2 => UIFont::draw_editor_icon(&mut buf, iw, ih, cx, cy - 7.0, col_ink),
                    3 => UIFont::draw_folder_icon(&mut buf, iw, ih, cx, cy - 7.0, col_ink),
                    _ => UIFont::draw_music_icon(&mut buf, iw, ih, cx, cy - 7.0, col_ink),
                }

                font.draw_text_centered(
                    &mut buf,
                    iw,
                    ih,
                    name,
                    cx,
                    cy + 13.0,
                    Tokens::TYPE_CAPTION,
                    col_ink,
                    col_shadow,
                    FontWeight::Medium,
                );
            }
            let _ = image::save_buffer(
                format!(
                    "{}/island_{}_quickaccess.png",
                    target_dir.display(),
                    mode_str
                ),
                &buf,
                iw,
                ih,
                image::ExtendedColorType::Rgba8,
            );
        }
    }
}
