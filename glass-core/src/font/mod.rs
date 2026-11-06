//! High-craft Inter font rendering engine for Liquid Glass UI overlays.
//!
//! Provides clean, professional, neo-grotesque typography matching Apple SF Pro.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const FONT_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
const FONT_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Bold,
    SemiBold,
    Medium,
    Regular,
}

pub struct UIFont {
    bold: FontRef<'static>,
    semibold: FontRef<'static>,
    medium: FontRef<'static>,
    regular: FontRef<'static>,
}

/// Backward compatibility alias for QuicksandFont.
pub type QuicksandFont = UIFont;

impl Default for UIFont {
    fn default() -> Self {
        Self::new()
    }
}

impl UIFont {
    pub fn new() -> Self {
        Self {
            bold: FontRef::try_from_slice(FONT_BOLD).expect("Valid Inter-Bold font"),
            semibold: FontRef::try_from_slice(FONT_SEMIBOLD).expect("Valid Inter-SemiBold font"),
            medium: FontRef::try_from_slice(FONT_MEDIUM).expect("Valid Inter-Medium font"),
            regular: FontRef::try_from_slice(FONT_REGULAR).expect("Valid Inter-Regular font"),
        }
    }

    fn get_font(&self, weight: FontWeight) -> &FontRef<'static> {
        match weight {
            FontWeight::Bold => &self.bold,
            FontWeight::SemiBold => &self.semibold,
            FontWeight::Medium => &self.medium,
            FontWeight::Regular => &self.regular,
        }
    }

    pub fn measure_text(&self, text: &str, size: f32, weight: FontWeight) -> f32 {
        let font = self.get_font(weight);
        let scale = PxScale::from(size);
        let scaled = font.as_scaled(scale);
        let mut width = 0.0;
        for ch in text.chars() {
            let glyph = scaled.scaled_glyph(ch);
            width += scaled.h_advance(glyph.id);
        }
        width
    }

    /// Calculate the baseline y coordinate that centers text vertically around `cy`.
    pub fn vertical_center_baseline(&self, cy: f32, size: f32, weight: FontWeight) -> f32 {
        let font = self.get_font(weight);
        let scale = PxScale::from(size);
        let scaled = font.as_scaled(scale);
        cy + (scaled.ascent() + scaled.descent()) * 0.5
    }

    /// Render crisp anti-aliased text vertically centered at `cy` with subtle ambient shadow.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_centered_y(
        &self,
        buf: &mut [u8],
        width: u32,
        height: u32,
        text: &str,
        x: f32,
        cy: f32,
        size: f32,
        color: [u8; 4],
        shadow_color: [u8; 4],
        weight: FontWeight,
    ) -> f32 {
        let baseline = self.vertical_center_baseline(cy, size, weight);
        self.draw_text_with_shadow(
            buf,
            width,
            height,
            text,
            x,
            baseline,
            size,
            color,
            shadow_color,
            weight,
        )
    }

    /// Render crisp anti-aliased text horizontally and vertically centered at `(cx, cy)` with subtle ambient shadow.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_centered(
        &self,
        buf: &mut [u8],
        width: u32,
        height: u32,
        text: &str,
        cx: f32,
        cy: f32,
        size: f32,
        color: [u8; 4],
        shadow_color: [u8; 4],
        weight: FontWeight,
    ) -> f32 {
        let text_w = self.measure_text(text, size, weight);
        let x = cx - text_w * 0.5;
        self.draw_text_centered_y(
            buf,
            width,
            height,
            text,
            x,
            cy,
            size,
            color,
            shadow_color,
            weight,
        )
    }

    /// Render crisp anti-aliased text with subtle ambient drop shadow for maximum legibility on any background.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_with_shadow(
        &self,
        buf: &mut [u8],
        width: u32,
        height: u32,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [u8; 4],
        shadow_color: [u8; 4],
        weight: FontWeight,
    ) -> f32 {
        if shadow_color[3] > 0 {
            // Elegant macOS downward drop shadow (y + 1.0)
            self.draw_text(
                buf,
                width,
                height,
                text,
                x,
                y + 1.0,
                size,
                shadow_color,
                weight,
            );
        }
        self.draw_text(buf, width, height, text, x, y, size, color, weight)
    }

    /// Render crisp anti-aliased text directly into an RGBA8 buffer.
    ///
    /// Coordinates `(x, y)` represent the baseline-anchored top-left position.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &self,
        buf: &mut [u8],
        width: u32,
        height: u32,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [u8; 4],
        weight: FontWeight,
    ) -> f32 {
        let font = self.get_font(weight);
        let scale = PxScale::from(size);
        let scaled = font.as_scaled(scale);
        let mut pen_x = x;

        for ch in text.chars() {
            let glyph = scaled.scaled_glyph(ch);
            let adv = scaled.h_advance(glyph.id);
            let mut positioned = glyph;
            positioned.position = point(pen_x, y);

            if let Some(outlined) = font.outline_glyph(positioned) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                        let alpha = ((coverage * color[3] as f32).round() as u32).min(255);
                        if alpha > 0 {
                            let inv_a = 255 - alpha;
                            let r =
                                ((color[0] as u32 * alpha + buf[idx] as u32 * inv_a) / 255) as u8;
                            let g = ((color[1] as u32 * alpha + buf[idx + 1] as u32 * inv_a) / 255)
                                as u8;
                            let b = ((color[2] as u32 * alpha + buf[idx + 2] as u32 * inv_a) / 255)
                                as u8;
                            let out_a = (buf[idx + 3] as u32 + alpha).min(255) as u8;
                            buf[idx] = r;
                            buf[idx + 1] = g;
                            buf[idx + 2] = b;
                            buf[idx + 3] = out_a;
                        }
                    }
                });
            }
            pen_x += adv;
        }
        pen_x
    }

    /// Draw a horizontal strike-through line
    #[allow(clippy::too_many_arguments)]
    pub fn draw_strikethrough(
        buf: &mut [u8],
        width: u32,
        height: u32,
        x_start: f32,
        x_end: f32,
        y: f32,
        thickness: f32,
        color: [u8; 4],
    ) {
        let y_min = (y - thickness * 0.5).max(0.0) as i32;
        let y_max = (y + thickness * 0.5).min(height as f32 - 1.0) as i32;
        let x_min = x_start.max(0.0) as i32;
        let x_max = x_end.min(width as f32 - 1.0) as i32;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                let alpha = color[3] as u32;
                if alpha > 0 {
                    let inv_a = 255 - alpha;
                    buf[idx] = ((color[0] as u32 * alpha + buf[idx] as u32 * inv_a) / 255) as u8;
                    buf[idx + 1] =
                        ((color[1] as u32 * alpha + buf[idx + 1] as u32 * inv_a) / 255) as u8;
                    buf[idx + 2] =
                        ((color[2] as u32 * alpha + buf[idx + 2] as u32 * inv_a) / 255) as u8;
                    buf[idx + 3] = (buf[idx + 3] as u32 + alpha).min(255) as u8;
                }
            }
        }
    }

    /// Draw a vertical blinking cursor bar
    #[allow(clippy::too_many_arguments)]
    pub fn draw_cursor(
        buf: &mut [u8],
        width: u32,
        height: u32,
        x: f32,
        y_top: f32,
        h: f32,
        thickness: f32,
        color: [u8; 4],
    ) {
        let x_min = (x - thickness * 0.5).max(0.0) as i32;
        let x_max = (x + thickness * 0.5).min(width as f32 - 1.0) as i32;
        let y_min = y_top.max(0.0) as i32;
        let y_max = (y_top + h).min(height as f32 - 1.0) as i32;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                let alpha = color[3] as u32;
                if alpha > 0 {
                    let inv_a = 255 - alpha;
                    buf[idx] = ((color[0] as u32 * alpha + buf[idx] as u32 * inv_a) / 255) as u8;
                    buf[idx + 1] =
                        ((color[1] as u32 * alpha + buf[idx + 1] as u32 * inv_a) / 255) as u8;
                    buf[idx + 2] =
                        ((color[2] as u32 * alpha + buf[idx + 2] as u32 * inv_a) / 255) as u8;
                    buf[idx + 3] = (buf[idx + 3] as u32 + alpha).min(255) as u8;
                }
            }
        }
    }

    /// Draw an anti-aliased rounded rectangle fill
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rounded_rect_fill(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        half_w: f32,
        half_h: f32,
        radius: f32,
        color: [u8; 4],
    ) {
        let x_min = ((cx - half_w - 1.0).max(0.0)) as i32;
        let x_max = ((cx + half_w + 1.0).min(width as f32 - 1.0)) as i32;
        let y_min = ((cy - half_h - 1.0).max(0.0)) as i32;
        let y_max = ((cy + half_h + 1.0).min(height as f32 - 1.0)) as i32;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let dx = (px as f32 - cx).abs() - (half_w - radius);
                let dy = (py as f32 - cy).abs() - (half_h - radius);
                let d = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt() - radius
                } else {
                    dx.max(dy) - radius
                };
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw an anti-aliased rounded rectangle border stroke
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rounded_rect_stroke(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        half_w: f32,
        half_h: f32,
        radius: f32,
        stroke_width: f32,
        color: [u8; 4],
    ) {
        let half_sw = stroke_width * 0.5;
        let x_min = ((cx - half_w - stroke_width - 1.0).max(0.0)) as i32;
        let x_max = ((cx + half_w + stroke_width + 1.0).min(width as f32 - 1.0)) as i32;
        let y_min = ((cy - half_h - stroke_width - 1.0).max(0.0)) as i32;
        let y_max = ((cy + half_h + stroke_width + 1.0).min(height as f32 - 1.0)) as i32;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let dx = (px as f32 - cx).abs() - (half_w - radius);
                let dy = (py as f32 - cy).abs() - (half_h - radius);
                let d_sdf = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt() - radius
                } else {
                    dx.max(dy) - radius
                };
                let dist_to_boundary = d_sdf.abs() - half_sw;
                if dist_to_boundary < 1.0 {
                    let cov = (1.0 - dist_to_boundary).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a crisp anti-aliased checkmark icon
    #[allow(clippy::too_many_arguments)]
    pub fn draw_checkmark(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        scale: f32,
        stroke: f32,
        color: [u8; 4],
    ) {
        let p0 = (cx - 4.2 * scale, cy + 0.2 * scale);
        let p1 = (cx - 1.2 * scale, cy + 3.4 * scale);
        let p2 = (cx + 4.8 * scale, cy - 3.4 * scale);

        let dist_seg = |px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32| -> f32 {
            let bax = bx - ax;
            let bay = by - ay;
            let pax = px - ax;
            let pay = py - ay;
            let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
            let qx = pax - bax * h;
            let qy = pay - bay * h;
            (qx * qx + qy * qy).sqrt()
        };

        let x_min = ((cx - 7.0 * scale).max(0.0)) as i32;
        let x_max = ((cx + 7.0 * scale).min(width as f32 - 1.0)) as i32;
        let y_min = ((cy - 6.0 * scale).max(0.0)) as i32;
        let y_max = ((cy + 6.0 * scale).min(height as f32 - 1.0)) as i32;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let d0 = dist_seg(px as f32, py as f32, p0.0, p0.1, p1.0, p1.1);
                let d1 = dist_seg(px as f32, py as f32, p1.0, p1.1, p2.0, p2.1);
                let d = d0.min(d1) - stroke * 0.5;
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a minimal calendar icon
    #[allow(clippy::too_many_arguments)]
    pub fn draw_calendar_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Outline box: 11x10, r=2, stroke 1.3
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 5.5, 5.0, 2.0, 1.3, color);
        // Top pins
        Self::draw_rounded_rect_fill(buf, width, height, cx - 2.5, cy - 5.5, 0.7, 1.2, 0.5, color);
        Self::draw_rounded_rect_fill(buf, width, height, cx + 2.5, cy - 5.5, 0.7, 1.2, 0.5, color);
    }

    /// Draw a minimal battery capsule with level gauge
    #[allow(clippy::too_many_arguments)]
    pub fn draw_battery_capsule(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        level: f32,
        color: [u8; 4],
    ) {
        // Outer capsule: 24x10, r=4, stroke 1.3
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 12.0, 5.0, 4.0, 1.3, color);
        // Positive terminal knob on right
        Self::draw_rounded_rect_fill(buf, width, height, cx + 13.0, cy, 1.0, 2.5, 1.0, color);
        // Filled level bar inside
        let inner_max_w = 20.0;
        let fill_w = ((level / 100.0).clamp(0.05, 1.0) * inner_max_w).max(2.0);
        let fill_cx = cx - 10.0 + fill_w * 0.5;
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            fill_cx,
            cy,
            fill_w * 0.5,
            3.2,
            2.0,
            color,
        );
    }

    /// Draw a precision anti-aliased vector play triangle (right-pointing)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_play_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        let x0 = cx - r * 0.60;
        let x1 = cx + r * 0.90;
        let y_top = cy - r * 0.85;
        let y_bot = cy + r * 0.85;

        let x_min = (x0 - 1.0).max(0.0) as i32;
        let x_max = (x1 + 1.0).min(width as f32 - 1.0) as i32;
        let y_min = (y_top - 1.0).max(0.0) as i32;
        let y_max = (y_bot + 1.0).min(height as f32 - 1.0) as i32;

        let h_span = x1 - x0;
        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let fx = px as f32;
                let fy = py as f32;
                if fx < x0 || fx > x1 {
                    continue;
                }
                let t = (fx - x0) / h_span;
                let max_dy = (1.0 - t) * (r * 0.85);
                let dist_y = (fy - cy).abs() - max_dy;
                let dist_x = (x0 - fx).max(fx - x1);
                let d = dist_y.max(dist_x);
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a precision anti-aliased vector pause icon (two vertical rounded batons)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_pause_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        let bar_hw = (r * 0.22).max(1.2);
        let bar_hh = r * 0.75;
        let sep = r * 0.45;
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            cx - sep,
            cy,
            bar_hw,
            bar_hh,
            bar_hw,
            color,
        );
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            cx + sep,
            cy,
            bar_hw,
            bar_hh,
            bar_hw,
            color,
        );
    }

    /// Draw a precision anti-aliased vector previous icon (vertical bar + left-pointing triangle)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_prev_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        // Left vertical bar
        let bar_hw = (r * 0.16).max(1.0);
        let bar_hh = r * 0.70;
        let bar_cx = cx - r * 0.65;
        Self::draw_rounded_rect_fill(
            buf, width, height, bar_cx, cy, bar_hw, bar_hh, bar_hw, color,
        );

        // Left-pointing triangle
        let x0 = cx - r * 0.45;
        let x1 = cx + r * 0.75;
        let y_top = cy - r * 0.70;
        let y_bot = cy + r * 0.70;

        let x_min = (x0 - 1.0).max(0.0) as i32;
        let x_max = (x1 + 1.0).min(width as f32 - 1.0) as i32;
        let y_min = (y_top - 1.0).max(0.0) as i32;
        let y_max = (y_bot + 1.0).min(height as f32 - 1.0) as i32;

        let h_span = x1 - x0;
        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let fx = px as f32;
                let fy = py as f32;
                if fx < x0 || fx > x1 {
                    continue;
                }
                let t = (x1 - fx) / h_span;
                let max_dy = (1.0 - t) * (r * 0.70);
                let dist_y = (fy - cy).abs() - max_dy;
                let dist_x = (x0 - fx).max(fx - x1);
                let d = dist_y.max(dist_x);
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a precision anti-aliased vector next icon (right-pointing triangle + vertical bar)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_next_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        // Right vertical bar
        let bar_hw = (r * 0.16).max(1.0);
        let bar_hh = r * 0.70;
        let bar_cx = cx + r * 0.65;
        Self::draw_rounded_rect_fill(
            buf, width, height, bar_cx, cy, bar_hw, bar_hh, bar_hw, color,
        );

        // Right-pointing triangle
        let x0 = cx - r * 0.75;
        let x1 = cx + r * 0.45;
        let y_top = cy - r * 0.70;
        let y_bot = cy + r * 0.70;

        let x_min = (x0 - 1.0).max(0.0) as i32;
        let x_max = (x1 + 1.0).min(width as f32 - 1.0) as i32;
        let y_min = (y_top - 1.0).max(0.0) as i32;
        let y_max = (y_bot + 1.0).min(height as f32 - 1.0) as i32;

        let h_span = x1 - x0;
        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let fx = px as f32;
                let fy = py as f32;
                if fx < x0 || fx > x1 {
                    continue;
                }
                let t = (fx - x0) / h_span;
                let max_dy = (1.0 - t) * (r * 0.70);
                let dist_y = (fy - cy).abs() - max_dy;
                let dist_x = (x0 - fx).max(fx - x1);
                let d = dist_y.max(dist_x);
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a precision anti-aliased vector notification bell icon
    #[allow(clippy::too_many_arguments)]
    pub fn draw_bell_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        // Bell dome: rounded rectangle upper body
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            cx,
            cy - r * 0.15,
            r * 0.50,
            r * 0.55,
            r * 0.45,
            color,
        );
        // Bell rim flare at bottom
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            cx,
            cy + r * 0.40,
            r * 0.72,
            r * 0.16,
            r * 0.16,
            color,
        );
        // Bell clapper at bottom center
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            cx,
            cy + r * 0.68,
            r * 0.22,
            r * 0.22,
            r * 0.22,
            color,
        );
    }

    /// Draw a precision anti-aliased line segment
    #[allow(clippy::too_many_arguments)]
    pub fn draw_segment(
        buf: &mut [u8],
        width: u32,
        height: u32,
        p0: (f32, f32),
        p1: (f32, f32),
        stroke: f32,
        color: [u8; 4],
    ) {
        let half_s = stroke * 0.5;
        let x_min = (p0.0.min(p1.0) - stroke - 1.0).max(0.0) as i32;
        let x_max = (p0.0.max(p1.0) + stroke + 1.0).min(width as f32 - 1.0) as i32;
        let y_min = (p0.1.min(p1.1) - stroke - 1.0).max(0.0) as i32;
        let y_max = (p0.1.max(p1.1) + stroke + 1.0).min(height as f32 - 1.0) as i32;

        let bax = p1.0 - p0.0;
        let bay = p1.1 - p0.1;
        let l2 = (bax * bax + bay * bay).max(0.0001);

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let pax = px as f32 - p0.0;
                let pay = py as f32 - p0.1;
                let h = ((pax * bax + pay * bay) / l2).clamp(0.0, 1.0);
                let qx = pax - bax * h;
                let qy = pay - bay * h;
                let dist = (qx * qx + qy * qy).sqrt();
                let d = dist - half_s;
                if d < 1.0 {
                    let cov = (1.0 - d).clamp(0.0, 1.0);
                    let alpha = ((cov * color[3] as f32).round() as u32).min(255);
                    if alpha > 0 {
                        let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
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

    /// Draw a precision terminal prompt icon (>_)
    pub fn draw_terminal_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Chevron '>'
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 6.0, cy - 4.5),
            (cx - 1.5, cy),
            1.5,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 1.5, cy),
            (cx - 6.0, cy + 4.5),
            1.5,
            color,
        );
        // Underline cursor '_'
        Self::draw_segment(
            buf,
            width,
            height,
            (cx + 1.0, cy + 4.5),
            (cx + 6.5, cy + 4.5),
            1.5,
            color,
        );
    }

    /// Draw a precision browser globe icon with meridian arcs (reads clearly as a 3D globe)
    pub fn draw_browser_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Outer spherical circle
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 7.0, 7.0, 7.0, 1.4, color);
        // Horizontal equator line
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 6.8, cy),
            (cx + 6.8, cy),
            1.2,
            color,
        );
        // Vertical meridian curved ellipse
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 3.4, 6.8, 3.4, 1.2, color);
    }

    /// Draw a precision close/dismiss icon (X)
    pub fn draw_close_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        r: f32,
        color: [u8; 4],
    ) {
        let d = r * 0.707;
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - d, cy - d),
            (cx + d, cy + d),
            1.5,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - d, cy + d),
            (cx + d, cy - d),
            1.5,
            color,
        );
    }

    /// Draw a precision editor code brackets icon (< / >)
    pub fn draw_editor_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Left bracket '<'
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 3.0, cy - 5.0),
            (cx - 7.0, cy),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 7.0, cy),
            (cx - 3.0, cy + 5.0),
            1.4,
            color,
        );
        // Right bracket '>'
        Self::draw_segment(
            buf,
            width,
            height,
            (cx + 3.0, cy - 5.0),
            (cx + 7.0, cy),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx + 7.0, cy),
            (cx + 3.0, cy + 5.0),
            1.4,
            color,
        );
        // Center slash '/'
        Self::draw_segment(
            buf,
            width,
            height,
            (cx + 1.2, cy - 6.0),
            (cx - 1.2, cy + 6.0),
            1.3,
            color,
        );
    }

    /// Draw a precision Lucide-grade folder icon (clean tab + body outline)
    pub fn draw_folder_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Tab: top left
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 7.0, cy - 4.5),
            (cx - 3.0, cy - 4.5),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 3.0, cy - 4.5),
            (cx - 1.5, cy - 2.5),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 1.5, cy - 2.5),
            (cx + 7.0, cy - 2.5),
            1.4,
            color,
        );
        // Folder body outline
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy + 1.5, 7.0, 4.5, 1.5, 1.3, color);
    }

    /// Draw a precision Lucide-grade music note icon
    pub fn draw_music_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Left note head
        Self::draw_rounded_rect_fill(buf, width, height, cx - 4.0, cy + 3.0, 2.4, 1.8, 1.8, color);
        // Right note head
        Self::draw_rounded_rect_fill(buf, width, height, cx + 4.0, cy + 1.0, 2.4, 1.8, 1.8, color);
        // Left stem
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 2.0, cy + 3.0),
            (cx - 2.0, cy - 5.5),
            1.4,
            color,
        );
        // Right stem
        Self::draw_segment(
            buf,
            width,
            height,
            (cx + 6.0, cy + 1.0),
            (cx + 6.0, cy - 7.5),
            1.4,
            color,
        );
        // Connecting top cross-beam
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 2.0, cy - 5.5),
            (cx + 6.0, cy - 7.5),
            2.0,
            color,
        );
    }

    #[inline]
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Draw a precision Lucide-grade Lock icon (curved shackle + body)
    pub fn draw_lock_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Body (half_w = 6.0, half_h = 4.5, radius = 2.0)
        Self::draw_rounded_rect_fill(buf, width, height, cx, cy + 2.5, 6.0, 4.5, 2.0, color);
        // Shackle (arched stroke, half_w = 3.6, half_h = 4.0, radius = 3.6)
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy - 3.5, 3.6, 4.0, 3.6, 1.4, color);
        // Keyhole dot (contrast negative dot inside body)
        let inv_color = [20, 25, 35, 255];
        Self::draw_rounded_rect_fill(buf, width, height, cx, cy + 2.0, 1.2, 1.6, 1.0, inv_color);
    }

    /// Draw a precision Lucide-grade Moon icon (crescent moon for Suspend)
    pub fn draw_moon_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        let r = 7.0f32;
        let r_in = 6.2f32;
        let off_x = 3.2f32;
        let off_y = -1.8f32;

        let min_x = (cx - r - 1.0).max(0.0) as u32;
        let max_x = (cx + r + 1.0).min(width as f32 - 1.0) as u32;
        let min_y = (cy - r - 1.0).max(0.0) as u32;
        let max_y = (cy + r + 1.0).min(height as f32 - 1.0) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let d_outer = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r;
                let d_inner =
                    ((px - (cx + off_x)).powi(2) + (py - (cy + off_y)).powi(2)).sqrt() - r_in;

                let d_crescent = d_outer.max(-d_inner);
                let alpha =
                    (1.0 - Self::smoothstep(-0.4, 0.4, d_crescent)) * (color[3] as f32 / 255.0);
                if alpha > 0.01 {
                    let idx = ((y * width + x) * 4) as usize;
                    if idx + 3 < buf.len() {
                        let inv_a = 1.0 - alpha;
                        buf[idx] = (color[0] as f32 * alpha + buf[idx] as f32 * inv_a) as u8;
                        buf[idx + 1] =
                            (color[1] as f32 * alpha + buf[idx + 1] as f32 * inv_a) as u8;
                        buf[idx + 2] =
                            (color[2] as f32 * alpha + buf[idx + 2] as f32 * inv_a) as u8;
                        buf[idx + 3] = (alpha * 255.0).max(buf[idx + 3] as f32) as u8;
                    }
                }
            }
        }
    }

    /// Draw a precision Lucide-grade Restart icon (circular arrow)
    pub fn draw_restart_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        let r = 6.0f32;
        let min_x = (cx - r - 2.0).max(0.0) as u32;
        let max_x = (cx + r + 2.0).min(width as f32 - 1.0) as u32;
        let min_y = (cy - r - 2.0).max(0.0) as u32;
        let max_y = (cy + r + 2.0).min(height as f32 - 1.0) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let d_ring = (dist - r).abs() - 0.75;

                let angle = (py - cy).atan2(px - cx); // [-pi, pi]
                                                      // 270 degree arc, open top-right (from -0.5 to 0.5 rad gap)
                let in_gap = (-0.4..0.6).contains(&angle);
                if !in_gap {
                    let alpha =
                        (1.0 - Self::smoothstep(-0.4, 0.4, d_ring)) * (color[3] as f32 / 255.0);
                    if alpha > 0.01 {
                        let idx = ((y * width + x) * 4) as usize;
                        if idx + 3 < buf.len() {
                            let inv_a = 1.0 - alpha;
                            buf[idx] = (color[0] as f32 * alpha + buf[idx] as f32 * inv_a) as u8;
                            buf[idx + 1] =
                                (color[1] as f32 * alpha + buf[idx + 1] as f32 * inv_a) as u8;
                            buf[idx + 2] =
                                (color[2] as f32 * alpha + buf[idx + 2] as f32 * inv_a) as u8;
                            buf[idx + 3] = (alpha * 255.0).max(buf[idx + 3] as f32) as u8;
                        }
                    }
                }
            }
        }

        // Arrow head at arc termination (top right: cx + 4.5, cy - 3.5)
        let ax = cx + 4.8;
        let ay = cy - 3.5;
        Self::draw_segment(
            buf,
            width,
            height,
            (ax - 3.5, ay - 1.0),
            (ax + 0.5, ay),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (ax - 0.5, ay + 3.8),
            (ax + 0.5, ay),
            1.4,
            color,
        );
    }

    /// Draw a precision Lucide-grade Power Off icon (circle with top vertical bar)
    pub fn draw_power_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        let r = 6.2f32;
        let min_x = (cx - r - 2.0).max(0.0) as u32;
        let max_x = (cx + r + 2.0).min(width as f32 - 1.0) as u32;
        let min_y = (cy - r - 2.0).max(0.0) as u32;
        let max_y = (cy + r + 2.0).min(height as f32 - 1.0) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let d_ring = (dist - r).abs() - 0.75;

                let angle = (py - cy).atan2(px - cx); // [-pi, pi]
                                                      // Open at top (-1.9 to -1.2 rad)
                let in_top_gap = angle > -1.95 && angle < -1.19;
                if !in_top_gap {
                    let alpha =
                        (1.0 - Self::smoothstep(-0.4, 0.4, d_ring)) * (color[3] as f32 / 255.0);
                    if alpha > 0.01 {
                        let idx = ((y * width + x) * 4) as usize;
                        if idx + 3 < buf.len() {
                            let inv_a = 1.0 - alpha;
                            buf[idx] = (color[0] as f32 * alpha + buf[idx] as f32 * inv_a) as u8;
                            buf[idx + 1] =
                                (color[1] as f32 * alpha + buf[idx + 1] as f32 * inv_a) as u8;
                            buf[idx + 2] =
                                (color[2] as f32 * alpha + buf[idx + 2] as f32 * inv_a) as u8;
                            buf[idx + 3] = (alpha * 255.0).max(buf[idx + 3] as f32) as u8;
                        }
                    }
                }
            }
        }

        // Center vertical power pin
        Self::draw_segment(
            buf,
            width,
            height,
            (cx, cy - 7.5),
            (cx, cy - 1.0),
            1.5,
            color,
        );
    }

    /// Draw a precision Lucide-grade Battery icon with level bar
    #[allow(clippy::too_many_arguments)]
    pub fn draw_battery_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        pct: u32,
        charging: bool,
        color: [u8; 4],
    ) {
        // Outer body (half_w = 7.0, half_h = 4.2, r = 1.6)
        Self::draw_rounded_rect_stroke(buf, width, height, cx - 1.0, cy, 7.0, 4.2, 1.6, 1.2, color);
        // Positive terminal cap (+ right nipple)
        Self::draw_rounded_rect_fill(buf, width, height, cx + 7.2, cy, 1.0, 2.0, 0.8, color);

        // Fill indicator
        let fill_pct = (pct as f32 / 100.0).clamp(0.08, 1.0);
        let max_w = 11.5f32;
        let cur_w = max_w * fill_pct;
        let fill_cx = (cx - 1.0) - (max_w - cur_w) * 0.5;
        Self::draw_rounded_rect_fill(
            buf,
            width,
            height,
            fill_cx,
            cy,
            cur_w * 0.5,
            2.8,
            1.0,
            color,
        );

        // Bolt if charging
        if charging {
            let bolt_col = [255, 220, 60, 255];
            Self::draw_segment(
                buf,
                width,
                height,
                (cx - 2.0, cy - 4.5),
                (cx, cy),
                1.4,
                bolt_col,
            );
            Self::draw_segment(
                buf,
                width,
                height,
                (cx - 0.5, cy),
                (cx + 1.5, cy + 4.5),
                1.4,
                bolt_col,
            );
        }
    }

    /// Draw a precision Lucide-grade CPU microchip icon
    pub fn draw_cpu_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Outer chip package (half = 5.5 x 5.5, r = 1.5)
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 5.5, 5.5, 1.5, 1.2, color);
        // Core die
        Self::draw_rounded_rect_fill(buf, width, height, cx, cy, 2.4, 2.4, 0.8, color);

        // Pins (2 on each of the 4 sides)
        let pin_len = 2.0f32;
        for &offset in &[-2.5f32, 2.5f32] {
            // Top pins
            Self::draw_segment(
                buf,
                width,
                height,
                (cx + offset, cy - 5.5),
                (cx + offset, cy - 5.5 - pin_len),
                1.1,
                color,
            );
            // Bottom pins
            Self::draw_segment(
                buf,
                width,
                height,
                (cx + offset, cy + 5.5),
                (cx + offset, cy + 5.5 + pin_len),
                1.1,
                color,
            );
            // Left pins
            Self::draw_segment(
                buf,
                width,
                height,
                (cx - 5.5, cy + offset),
                (cx - 5.5 - pin_len, cy + offset),
                1.1,
                color,
            );
            // Right pins
            Self::draw_segment(
                buf,
                width,
                height,
                (cx + 5.5, cy + offset),
                (cx + 5.5 + pin_len, cy + offset),
                1.1,
                color,
            );
        }
    }

    /// Draw a precision Lucide-grade RAM memory stick icon
    pub fn draw_ram_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Outer stick module: half_w = 6.5, half_h = 4.0, r = 1.0, stroke = 1.2
        Self::draw_rounded_rect_stroke(buf, width, height, cx, cy, 6.5, 4.0, 1.0, 1.2, color);
        // 3 mini memory chip blocks inside
        for &ox in &[-3.5f32, 0.0f32, 3.5f32] {
            Self::draw_rounded_rect_fill(
                buf,
                width,
                height,
                cx + ox,
                cy - 0.5,
                1.1,
                1.8,
                0.5,
                color,
            );
        }
        // Bottom contact pins
        for &ox in &[-4.0f32, -2.0f32, 2.0f32, 4.0f32] {
            Self::draw_segment(
                buf,
                width,
                height,
                (cx + ox, cy + 4.0),
                (cx + ox, cy + 5.5),
                1.0,
                color,
            );
        }
    }

    /// Draw a precision Lucide-grade Clock / Uptime icon (circle + hands)
    pub fn draw_clock_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        let r = 6.2f32;
        let min_x = (cx - r - 2.0).max(0.0) as u32;
        let max_x = (cx + r + 2.0).min(width as f32 - 1.0) as u32;
        let min_y = (cy - r - 2.0).max(0.0) as u32;
        let max_y = (cy + r + 2.0).min(height as f32 - 1.0) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let d_ring = (dist - r).abs() - 0.65;
                let alpha = (1.0 - Self::smoothstep(-0.4, 0.4, d_ring)) * (color[3] as f32 / 255.0);
                if alpha > 0.01 {
                    let idx = ((y * width + x) * 4) as usize;
                    if idx + 3 < buf.len() {
                        let inv_a = 1.0 - alpha;
                        buf[idx] = (color[0] as f32 * alpha + buf[idx] as f32 * inv_a) as u8;
                        buf[idx + 1] =
                            (color[1] as f32 * alpha + buf[idx + 1] as f32 * inv_a) as u8;
                        buf[idx + 2] =
                            (color[2] as f32 * alpha + buf[idx + 2] as f32 * inv_a) as u8;
                        buf[idx + 3] = (alpha * 255.0).max(buf[idx + 3] as f32) as u8;
                    }
                }
            }
        }

        // Center dot
        Self::draw_rounded_rect_fill(buf, width, height, cx, cy, 0.9, 0.9, 0.9, color);
        // Hour hand: center to 12 o'clock
        Self::draw_segment(buf, width, height, (cx, cy), (cx, cy - 3.4), 1.3, color);
        // Minute hand: center to 3 o'clock
        Self::draw_segment(buf, width, height, (cx, cy), (cx + 3.2, cy), 1.3, color);
    }

    /// Draw a precision Lucide-grade Settings / Cog gear icon
    pub fn draw_settings_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        // Center ring: r = 3.0, stroke = 1.3
        let r = 3.0f32;
        let min_x = (cx - 7.5).max(0.0) as u32;
        let max_x = (cx + 7.5).min(width as f32 - 1.0) as u32;
        let min_y = (cy - 7.5).max(0.0) as u32;
        let max_y = (cy + 7.5).min(height as f32 - 1.0) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let d_ring = (dist - r).abs() - 0.65;
                let alpha = (1.0 - Self::smoothstep(-0.4, 0.4, d_ring)) * (color[3] as f32 / 255.0);
                if alpha > 0.01 {
                    let idx = ((y * width + x) * 4) as usize;
                    if idx + 3 < buf.len() {
                        let inv_a = 1.0 - alpha;
                        buf[idx] = (color[0] as f32 * alpha + buf[idx] as f32 * inv_a) as u8;
                        buf[idx + 1] =
                            (color[1] as f32 * alpha + buf[idx + 1] as f32 * inv_a) as u8;
                        buf[idx + 2] =
                            (color[2] as f32 * alpha + buf[idx + 2] as f32 * inv_a) as u8;
                        buf[idx + 3] = (alpha * 255.0).max(buf[idx + 3] as f32) as u8;
                    }
                }
            }
        }

        // 6 radial teeth
        for i in 0..6 {
            let angle = (i as f32) * (std::f32::consts::PI / 3.0);
            let (sin_a, cos_a) = angle.sin_cos();
            let p1 = (cx + cos_a * 4.0, cy + sin_a * 4.0);
            let p2 = (cx + cos_a * 6.8, cy + sin_a * 6.8);
            Self::draw_segment(buf, width, height, p1, p2, 1.6, color);
        }
    }

    /// Draw a precision Lucide-grade Sync / Refresh icon
    pub fn draw_sync_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        Self::draw_restart_icon(buf, width, height, cx, cy, color);
    }

    /// Draw a precision Lucide-grade Checkmark icon
    pub fn draw_check_icon(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        color: [u8; 4],
    ) {
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 3.8, cy),
            (cx - 1.0, cy + 3.2),
            1.4,
            color,
        );
        Self::draw_segment(
            buf,
            width,
            height,
            (cx - 1.0, cy + 3.2),
            (cx + 4.2, cy - 3.5),
            1.4,
            color,
        );
    }

    /// Draws a photorealistic spinning vinyl disc with concentric groove rings,
    /// rotating anisotropic specular sheen, colored inner core, and spindle hole.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_spinning_vinyl(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        radius: f32,
        angle_rad: f32,
        is_light: bool,
    ) {
        let x_min = (cx - radius - 1.5).max(0.0) as i32;
        let x_max = (cx + radius + 1.5).min(width as f32 - 1.0) as i32;
        let y_min = (cy - radius - 1.5).max(0.0) as i32;
        let y_max = (cy + radius + 1.5).min(height as f32 - 1.0) as i32;

        let base_body: [u8; 3] = if is_light { [24, 28, 38] } else { [16, 20, 28] };
        let core_color: [u8; 3] = if is_light {
            [235, 240, 252]
        } else {
            [245, 248, 255]
        };
        let hole_color: [u8; 3] = [10, 12, 16];

        let core_r = radius * 0.38;
        let hole_r = radius * 0.12;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let dx = px as f32 - cx;
                let dy = py as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > radius + 1.0 {
                    continue;
                }

                let outer_aa = (radius - dist).clamp(0.0, 1.0);
                if outer_aa <= 0.0 {
                    continue;
                }

                let pixel_angle = dy.atan2(dx);
                // Rotating anisotropic light sheen: produces two rotating specular lobes (bow-tie sheen)
                let rel_angle = pixel_angle - angle_rad;
                let sheen = rel_angle.sin().abs().powi(6) * 0.32;

                // Micro-groove pattern (hairline concentric tracks)
                let groove = ((dist * 1.8).sin() * 0.5 + 0.5) * 0.08;

                let final_rgb = if dist <= hole_r {
                    // Center spindle hole
                    let hole_aa = (hole_r - dist).clamp(0.0, 1.0);
                    let mix_r = (core_color[0] as f32 * (1.0 - hole_aa)
                        + hole_color[0] as f32 * hole_aa) as u8;
                    let mix_g = (core_color[1] as f32 * (1.0 - hole_aa)
                        + hole_color[1] as f32 * hole_aa) as u8;
                    let mix_b = (core_color[2] as f32 * (1.0 - hole_aa)
                        + hole_color[2] as f32 * hole_aa) as u8;
                    [mix_r, mix_g, mix_b]
                } else if dist <= core_r {
                    // Center label / core with subtle rotating marker notch
                    let notch_angle = (rel_angle.cos() * 0.5 + 0.5).powi(12) * 0.15;
                    let r = (core_color[0] as f32 * (1.0 - notch_angle)).clamp(0.0, 255.0) as u8;
                    let g = (core_color[1] as f32 * (1.0 - notch_angle)).clamp(0.0, 255.0) as u8;
                    let b = (core_color[2] as f32 * (1.0 - notch_angle)).clamp(0.0, 255.0) as u8;
                    [r, g, b]
                } else {
                    // Vinyl grooves + rotating specular reflection
                    let light_boost = (sheen + groove).clamp(0.0, 0.45);
                    let r = (base_body[0] as f32 + 255.0 * light_boost).clamp(0.0, 255.0) as u8;
                    let g = (base_body[1] as f32 + 255.0 * light_boost).clamp(0.0, 255.0) as u8;
                    let b = (base_body[2] as f32 + 255.0 * light_boost).clamp(0.0, 255.0) as u8;
                    [r, g, b]
                };

                // Alpha blend onto destination buffer
                let alpha = (outer_aa * 255.0) as u8;
                let idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                let src_a = alpha as f32 / 255.0;
                let dst_a = buf[idx + 3] as f32 / 255.0;
                let out_a = src_a + dst_a * (1.0 - src_a);

                if out_a > 0.0 {
                    buf[idx] = ((final_rgb[0] as f32 * src_a
                        + buf[idx] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[idx + 1] = ((final_rgb[1] as f32 * src_a
                        + buf[idx + 1] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[idx + 2] = ((final_rgb[2] as f32 * src_a
                        + buf[idx + 2] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[idx + 3] = (out_a * 255.0).min(255.0) as u8;
                }
            }
        }
    }

    /// Draws a dynamic 6-bar reactive audio frequency spectrum visualizer with smooth rounded pills
    #[allow(clippy::too_many_arguments)]
    pub fn draw_audio_visualizer(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        t_sec: f32,
        is_playing: bool,
        col: [u8; 4],
    ) {
        let bar_count = 6;
        let bar_w = 2.4f32;
        let spacing = 6.0f32;
        let total_w = (bar_count as f32 - 1.0) * spacing;
        let start_x = cx - total_w * 0.5;

        for i in 0..bar_count {
            let bx = start_x + (i as f32) * spacing;
            let half_h = if is_playing {
                let fi = i as f32;
                let wave1 = (t_sec * (8.0 + fi * 2.5)).sin();
                let wave2 = (t_sec * (13.0 - fi * 1.8)).cos();
                let wave3 = (t_sec * (5.5 + fi * 3.8)).sin();
                let combined = (wave1 * 0.40 + wave2 * 0.35 + wave3 * 0.25).abs();
                (combined * 10.0 + 3.0).clamp(2.5, 14.0)
            } else {
                2.2f32
            };

            Self::draw_rounded_rect_fill(
                buf,
                width,
                height,
                bx,
                cy,
                bar_w * 0.5,
                half_h,
                bar_w * 0.5,
                col,
            );
        }
    }

    /// Draws a rounded RGBA thumbnail (e.g. 46x46 album art) with smooth corner clipping
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rounded_thumbnail(
        buf: &mut [u8],
        width: u32,
        height: u32,
        cx: f32,
        cy: f32,
        thumb_rgba: &[u8],
        thumb_w: u32,
        thumb_h: u32,
        corner_radius: f32,
    ) {
        let half_w = (thumb_w as f32) * 0.5;
        let half_h = (thumb_h as f32) * 0.5;
        let x_min = (cx - half_w).max(0.0) as i32;
        let x_max = (cx + half_w).min(width as f32 - 1.0) as i32;
        let y_min = (cy - half_h).max(0.0) as i32;
        let y_max = (cy + half_h).min(height as f32 - 1.0) as i32;

        let r = corner_radius.min(half_w).min(half_h);

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let lx = px as f32 - cx;
                let ly = py as f32 - cy;

                let qx = lx.abs() - half_w + r;
                let qy = ly.abs() - half_h + r;
                let sdf =
                    (qx.max(qy)).min(0.0) + (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() - r;

                if sdf > 0.5 {
                    continue;
                }
                let aa = (0.5 - sdf).clamp(0.0, 1.0);

                let tx = (((lx + half_w) / (half_w * 2.0)) * (thumb_w as f32))
                    .clamp(0.0, thumb_w as f32 - 1.0) as u32;
                let ty = (((ly + half_h) / (half_h * 2.0)) * (thumb_h as f32))
                    .clamp(0.0, thumb_h as f32 - 1.0) as u32;
                let t_idx = ((ty * thumb_w + tx) * 4) as usize;

                if t_idx + 3 >= thumb_rgba.len() {
                    continue;
                }

                let src_a = (thumb_rgba[t_idx + 3] as f32 / 255.0) * aa;
                let d_idx = ((py as usize) * (width as usize) + (px as usize)) * 4;
                let dst_a = buf[d_idx + 3] as f32 / 255.0;
                let out_a = src_a + dst_a * (1.0 - src_a);

                if out_a > 0.0 {
                    buf[d_idx] = ((thumb_rgba[t_idx] as f32 * src_a
                        + buf[d_idx] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[d_idx + 1] = ((thumb_rgba[t_idx + 1] as f32 * src_a
                        + buf[d_idx + 1] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[d_idx + 2] = ((thumb_rgba[t_idx + 2] as f32 * src_a
                        + buf[d_idx + 2] as f32 * dst_a * (1.0 - src_a))
                        / out_a) as u8;
                    buf[d_idx + 3] = (out_a * 255.0).min(255.0) as u8;
                }
            }
        }
    }
}
