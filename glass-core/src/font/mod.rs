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
}
