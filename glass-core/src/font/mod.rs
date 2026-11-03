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
}
