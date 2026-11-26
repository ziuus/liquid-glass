//! 4x4 Supersampled Lucide-style 24x24 Vector Icon Rasterizer.
//!
//! Renders modern, crisp, smooth-curved UI icons directly into RGBA8 buffers
//! without external runtime dependencies.

#[derive(Clone, Copy)]
pub enum IconCmd {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Circle(f32, f32, f32),
    RoundRect(f32, f32, f32, f32, f32),
    FillCircle(f32, f32, f32),
}

struct Seg {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

struct FlattenedIcon {
    segs: Vec<Seg>,
    circles: Vec<(f32, f32, f32)>,
    rrects: Vec<(f32, f32, f32, f32, f32)>,
    fill_circles: Vec<(f32, f32, f32)>,
}

fn flatten_cmds(cmds: &[IconCmd], scale: f32, ox: f32, oy: f32) -> FlattenedIcon {
    let mut segs = Vec::new();
    let mut circles = Vec::new();
    let mut rrects = Vec::new();
    let mut fill_circles = Vec::new();
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;

    for &cmd in cmds {
        match cmd {
            IconCmd::Move(x, y) => {
                cx = ox + (x - 12.0) * scale;
                cy = oy + (y - 12.0) * scale;
            }
            IconCmd::Line(x, y) => {
                let nx = ox + (x - 12.0) * scale;
                let ny = oy + (y - 12.0) * scale;
                segs.push(Seg {
                    x0: cx,
                    y0: cy,
                    x1: nx,
                    y1: ny,
                });
                cx = nx;
                cy = ny;
            }
            IconCmd::Cubic(c1x, c1y, c2x, c2y, x, y) => {
                let p0x = cx;
                let p0y = cy;
                let p1x = ox + (c1x - 12.0) * scale;
                let p1y = oy + (c1y - 12.0) * scale;
                let p2x = ox + (c2x - 12.0) * scale;
                let p2y = oy + (c2y - 12.0) * scale;
                let p3x = ox + (x - 12.0) * scale;
                let p3y = oy + (y - 12.0) * scale;
                let steps = 10;
                let mut prev_x = p0x;
                let mut prev_y = p0y;
                for s in 1..=steps {
                    let t = s as f32 / steps as f32;
                    let u = 1.0 - t;
                    let uu = u * u;
                    let tt = t * t;
                    let nx = uu * u * p0x + 3.0 * uu * t * p1x + 3.0 * u * tt * p2x + tt * t * p3x;
                    let ny = uu * u * p0y + 3.0 * uu * t * p1y + 3.0 * u * tt * p2y + tt * t * p3y;
                    segs.push(Seg {
                        x0: prev_x,
                        y0: prev_y,
                        x1: nx,
                        y1: ny,
                    });
                    prev_x = nx;
                    prev_y = ny;
                }
                cx = p3x;
                cy = p3y;
            }
            IconCmd::Circle(x, y, r) => {
                circles.push((ox + (x - 12.0) * scale, oy + (y - 12.0) * scale, r * scale));
            }
            IconCmd::RoundRect(x, y, w, h, rx) => {
                let rcx = ox + (x + w * 0.5 - 12.0) * scale;
                let rcy = oy + (y + h * 0.5 - 12.0) * scale;
                rrects.push((rcx, rcy, w * 0.5 * scale, h * 0.5 * scale, rx * scale));
            }
            IconCmd::FillCircle(x, y, r) => {
                fill_circles.push((ox + (x - 12.0) * scale, oy + (y - 12.0) * scale, r * scale));
            }
        }
    }
    FlattenedIcon {
        segs,
        circles,
        rrects,
        fill_circles,
    }
}

#[inline]
fn dist_seg(px: f32, py: f32, s: &Seg) -> f32 {
    let vx = s.x1 - s.x0;
    let vy = s.y1 - s.y0;
    let l2 = vx * vx + vy * vy;
    if l2 < 1e-6 {
        return ((px - s.x0).powi(2) + (py - s.y0).powi(2)).sqrt();
    }
    let t = (((px - s.x0) * vx + (py - s.y0) * vy) / l2).clamp(0.0, 1.0);
    let qx = s.x0 + t * vx;
    let qy = s.y0 + t * vy;
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

#[inline]
fn dist_rrect_stroke(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let dx = (px - cx).abs() - (hw - r);
    let dy = (py - cy).abs() - (hh - r);
    let d = if dx > 0.0 && dy > 0.0 {
        (dx * dx + dy * dy).sqrt() - r
    } else {
        dx.max(dy) - r
    };
    d.abs()
}

/// Draw a 24x24 vector icon with 4x4 supersampled anti-aliasing.
#[allow(clippy::too_many_arguments)]
pub fn draw_vector_icon(
    buf: &mut [u8],
    width: u32,
    height: u32,
    cx: f32,
    cy: f32,
    size_px: f32,
    stroke_px: f32,
    color: [u8; 4],
    cmds: &[IconCmd],
) {
    if color[3] == 0 {
        return;
    }
    let scale = size_px / 24.0;
    let half_sw = stroke_px * 0.5;
    let flat = flatten_cmds(cmds, scale, cx, cy);
    let segs = &flat.segs;
    let circles = &flat.circles;
    let rrects = &flat.rrects;
    let fill_circles = &flat.fill_circles;

    let pad = size_px * 0.5 + stroke_px + 2.0;
    let x_min = (cx - pad).max(0.0) as i32;
    let x_max = (cx + pad).min(width as f32 - 1.0) as i32;
    let y_min = (cy - pad).max(0.0) as i32;
    let y_max = (cy + pad).min(height as f32 - 1.0) as i32;

    const SUB: [f32; 4] = [0.125, 0.375, 0.625, 0.875];
    let inv16 = 1.0 / 16.0;

    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let mut hits = 0.0f32;
            for &sy in &SUB {
                let y = py as f32 + sy;
                for &sx in &SUB {
                    let x = px as f32 + sx;
                    let mut min_d = f32::MAX;
                    for s in segs {
                        let d = dist_seg(x, y, s) - half_sw;
                        if d < min_d {
                            min_d = d;
                        }
                    }
                    for &(ccx, ccy, cr) in circles {
                        let d =
                            (((x - ccx).powi(2) + (y - ccy).powi(2)).sqrt() - cr).abs() - half_sw;
                        if d < min_d {
                            min_d = d;
                        }
                    }
                    for &(rcx, rcy, hw, hh, r) in rrects {
                        let d = dist_rrect_stroke(x, y, rcx, rcy, hw, hh, r) - half_sw;
                        if d < min_d {
                            min_d = d;
                        }
                    }
                    for &(fcx, fcy, fr) in fill_circles {
                        let d = ((x - fcx).powi(2) + (y - fcy).powi(2)).sqrt() - fr;
                        if d < min_d {
                            min_d = d;
                        }
                    }
                    if min_d <= 0.0 {
                        hits += 1.0;
                    }
                }
            }
            if hits > 0.0 {
                let cov = hits * inv16;
                let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                if alpha > 0 {
                    let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                    if idx + 3 < buf.len() {
                        let inv_a = 255 - alpha;
                        buf[idx] =
                            ((color[0] as u32 * alpha + buf[idx] as u32 * inv_a) / 255) as u8;
                        buf[idx + 1] =
                            ((color[1] as u32 * alpha + buf[idx + 1] as u32 * inv_a) / 255) as u8;
                        buf[idx + 2] =
                            ((color[2] as u32 * alpha + buf[idx + 2] as u32 * inv_a) / 255) as u8;
                        buf[idx + 3] = (buf[idx + 3] as u32 + alpha).min(255) as u8;
                    }
                }
            }
        }
    }
}

pub const ICON_TERMINAL: &[IconCmd] = &[
    IconCmd::RoundRect(3.0, 4.0, 18.0, 16.0, 3.0),
    IconCmd::Move(7.0, 9.5),
    IconCmd::Line(10.5, 12.5),
    IconCmd::Line(7.0, 15.5),
    IconCmd::Move(13.0, 15.5),
    IconCmd::Line(17.0, 15.5),
];

pub const ICON_BROWSER: &[IconCmd] = &[
    IconCmd::Circle(12.0, 12.0, 9.0),
    IconCmd::Move(3.3, 9.5),
    IconCmd::Line(20.7, 9.5),
    IconCmd::Move(3.3, 14.5),
    IconCmd::Line(20.7, 14.5),
    IconCmd::Move(12.0, 3.0),
    IconCmd::Cubic(15.5, 6.5, 15.5, 17.5, 12.0, 21.0),
    IconCmd::Move(12.0, 3.0),
    IconCmd::Cubic(8.5, 6.5, 8.5, 17.5, 12.0, 21.0),
];

pub const ICON_EDITOR: &[IconCmd] = &[
    IconCmd::RoundRect(3.0, 4.0, 18.0, 16.0, 3.0),
    IconCmd::Move(9.0, 9.0),
    IconCmd::Line(6.0, 12.0),
    IconCmd::Line(9.0, 15.0),
    IconCmd::Move(15.0, 9.0),
    IconCmd::Line(18.0, 12.0),
    IconCmd::Line(15.0, 15.0),
    IconCmd::Move(13.2, 8.2),
    IconCmd::Line(10.8, 15.8),
];

pub const ICON_FOLDER: &[IconCmd] = &[
    IconCmd::Move(4.0, 20.0),
    IconCmd::Line(20.0, 20.0),
    IconCmd::Cubic(21.1, 20.0, 22.0, 19.1, 22.0, 18.0),
    IconCmd::Line(22.0, 8.5),
    IconCmd::Cubic(22.0, 7.4, 21.1, 6.5, 20.0, 6.5),
    IconCmd::Line(12.1, 6.5),
    IconCmd::Cubic(11.4, 6.5, 10.7, 6.1, 10.3, 5.5),
    IconCmd::Line(9.4, 4.3),
    IconCmd::Cubic(9.0, 3.8, 8.4, 3.5, 7.8, 3.5),
    IconCmd::Line(4.0, 3.5),
    IconCmd::Cubic(2.9, 3.5, 2.0, 4.4, 2.0, 5.5),
    IconCmd::Line(2.0, 18.0),
    IconCmd::Cubic(2.0, 19.1, 2.9, 20.0, 4.0, 20.0),
    IconCmd::Move(2.5, 10.0),
    IconCmd::Line(21.5, 10.0),
];

pub const ICON_MUSIC: &[IconCmd] = &[
    IconCmd::Move(9.0, 17.5),
    IconCmd::Line(9.0, 5.5),
    IconCmd::Line(20.0, 3.5),
    IconCmd::Line(20.0, 15.5),
    IconCmd::Move(9.0, 9.0),
    IconCmd::Line(20.0, 7.0),
    IconCmd::FillCircle(6.5, 17.5, 2.6),
    IconCmd::FillCircle(17.5, 15.5, 2.6),
];

pub const ICON_BELL: &[IconCmd] = &[
    IconCmd::Move(18.0, 8.5),
    IconCmd::Cubic(18.0, 5.2, 15.3, 3.0, 12.0, 3.0),
    IconCmd::Cubic(8.7, 3.0, 6.0, 5.2, 6.0, 8.5),
    IconCmd::Cubic(6.0, 14.5, 3.5, 16.5, 3.5, 16.5),
    IconCmd::Line(20.5, 16.5),
    IconCmd::Cubic(20.5, 16.5, 18.0, 14.5, 18.0, 8.5),
    IconCmd::Move(10.3, 19.5),
    IconCmd::Cubic(10.7, 20.6, 13.3, 20.6, 13.7, 19.5),
];

pub const ICON_CPU: &[IconCmd] = &[
    IconCmd::RoundRect(5.0, 5.0, 14.0, 14.0, 2.5),
    IconCmd::RoundRect(9.0, 9.0, 6.0, 6.0, 1.0),
    IconCmd::Move(9.0, 2.5),
    IconCmd::Line(9.0, 5.0),
    IconCmd::Move(15.0, 2.5),
    IconCmd::Line(15.0, 5.0),
    IconCmd::Move(9.0, 19.0),
    IconCmd::Line(9.0, 21.5),
    IconCmd::Move(15.0, 19.0),
    IconCmd::Line(15.0, 21.5),
    IconCmd::Move(2.5, 9.0),
    IconCmd::Line(5.0, 9.0),
    IconCmd::Move(2.5, 15.0),
    IconCmd::Line(5.0, 15.0),
    IconCmd::Move(19.0, 9.0),
    IconCmd::Line(21.5, 9.0),
    IconCmd::Move(19.0, 15.0),
    IconCmd::Line(21.5, 15.0),
];

pub const ICON_RAM: &[IconCmd] = &[
    IconCmd::RoundRect(3.0, 6.0, 18.0, 11.0, 2.0),
    IconCmd::RoundRect(6.5, 9.0, 3.5, 5.0, 0.7),
    IconCmd::RoundRect(14.0, 9.0, 3.5, 5.0, 0.7),
    IconCmd::Move(6.0, 17.0),
    IconCmd::Line(6.0, 20.0),
    IconCmd::Move(10.0, 17.0),
    IconCmd::Line(10.0, 20.0),
    IconCmd::Move(14.0, 17.0),
    IconCmd::Line(14.0, 20.0),
    IconCmd::Move(18.0, 17.0),
    IconCmd::Line(18.0, 20.0),
];

pub const ICON_LOCK: &[IconCmd] = &[
    IconCmd::RoundRect(4.5, 10.5, 15.0, 10.5, 2.5),
    IconCmd::Move(8.0, 10.5),
    IconCmd::Line(8.0, 7.0),
    IconCmd::Cubic(8.0, 4.5, 16.0, 4.5, 16.0, 7.0),
    IconCmd::Line(16.0, 10.5),
    IconCmd::FillCircle(12.0, 15.7, 1.3),
];

pub const ICON_MOON: &[IconCmd] = &[
    IconCmd::Move(20.0, 14.5),
    IconCmd::Cubic(18.4, 15.2, 16.6, 15.4, 14.8, 14.9),
    IconCmd::Cubic(11.1, 13.9, 8.9, 10.1, 9.9, 6.4),
    IconCmd::Cubic(10.2, 5.2, 10.8, 4.1, 11.7, 3.3),
    IconCmd::Cubic(7.0, 3.8, 3.5, 7.8, 3.5, 12.6),
    IconCmd::Cubic(3.5, 17.8, 7.7, 21.5, 12.9, 21.5),
    IconCmd::Cubic(16.4, 21.5, 19.3, 19.1, 20.0, 14.5),
];

pub const ICON_POWER: &[IconCmd] = &[
    IconCmd::Move(12.0, 3.0),
    IconCmd::Line(12.0, 11.5),
    IconCmd::Move(17.6, 6.4),
    IconCmd::Cubic(20.6, 9.4, 20.6, 14.6, 17.6, 17.6),
    IconCmd::Cubic(14.5, 20.7, 9.5, 20.7, 6.4, 17.6),
    IconCmd::Cubic(3.4, 14.6, 3.4, 9.4, 6.4, 6.4),
];

pub const ICON_CLOCK: &[IconCmd] = &[
    IconCmd::Circle(12.0, 12.0, 9.0),
    IconCmd::Move(12.0, 7.0),
    IconCmd::Line(12.0, 12.0),
    IconCmd::Line(15.8, 14.2),
];
