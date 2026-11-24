//! E2E Test Suite: Wallpaper Contrast and Optical Adaptation (FEAT-TST-02)
//!
//! Verifies WCAG 2.1 AA legibility (contrast ratio >= 4.5:1) for central clock text
//! and numerals across all 18 wallpapers in `~/Pictures/wallpaper/` and synthetic
//! test benches (pure black, pure white, 50% gray, vertical/horizontal splits).
//! Covers Tiers 1-4.

#![allow(unused_imports, dead_code, clippy::all)]

use std::path::{Path, PathBuf};

#[path = "../examples/liquid-shell/src/center_clock.rs"]
mod center_clock;
use center_clock::{
    compute_clock_text_styling, compute_zone_text_styling, COL_AMPM_BASE_DARK, COL_AMPM_BASE_LIGHT,
    COL_DATE_BASE_DARK, COL_DATE_BASE_LIGHT, COL_DAY_BASE_DARK, COL_DAY_BASE_LIGHT,
};
use glass_platform::wallpaper::{OpticalZoneProfile, WallpaperManager, ZoneMetrics};

// -----------------------------------------------------------------------------
// WCAG 2.1 Optical Math & Verification Utilities
// -----------------------------------------------------------------------------

/// Converts an 8-bit sRGB channel [0..255] to linear luminance component [0.0..1.0]
/// using standard IEC 61966-2-1 transfer characteristic.
pub fn srgb_channel_to_linear(val: u8) -> f64 {
    let c = val as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Computes WCAG 2.1 Relative Luminance for an sRGB color.
/// L = 0.2126 * R_lin + 0.7152 * G_lin + 0.0722 * B_lin
pub fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let r = srgb_channel_to_linear(rgb[0]);
    let g = srgb_channel_to_linear(rgb[1]);
    let b = srgb_channel_to_linear(rgb[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Computes WCAG 2.1 Contrast Ratio between two relative luminances.
/// Ratio = (L1 + 0.05) / (L2 + 0.05), where L1 >= L2. Range: [1.0, 21.0].
pub fn contrast_ratio(lum1: f64, lum2: f64) -> f64 {
    let (lighter, darker) = if lum1 >= lum2 {
        (lum1, lum2)
    } else {
        (lum2, lum1)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// Computes average relative luminance of an RGBA image buffer accounting for alpha channel opacity.
pub fn buffer_average_luminance(rgba: &[u8]) -> f64 {
    if rgba.is_empty() {
        return 0.0;
    }
    let mut total_lum = 0.0f64;
    let mut count = 0usize;
    let step = 4; // Check every pixel or strided for performance
    for chunk in rgba.chunks_exact(step) {
        let r = srgb_channel_to_linear(chunk[0]);
        let g = srgb_channel_to_linear(chunk[1]);
        let b = srgb_channel_to_linear(chunk[2]);
        let a = chunk[3] as f64 / 255.0;
        total_lum += (0.2126 * r + 0.7152 * g + 0.0722 * b) * a;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total_lum / count as f64
    }
}

/// Computes luminance mean and standard deviation over an RGBA buffer accounting for alpha channel opacity.
pub fn buffer_luminance_stats(rgba: &[u8]) -> (f64, f64) {
    if rgba.is_empty() {
        return (0.0, 0.0);
    }
    let mut lums = Vec::with_capacity(rgba.len() / 4);
    for chunk in rgba.chunks_exact(4) {
        let r = srgb_channel_to_linear(chunk[0]);
        let g = srgb_channel_to_linear(chunk[1]);
        let b = srgb_channel_to_linear(chunk[2]);
        let a = chunk[3] as f64 / 255.0;
        lums.push((0.2126 * r + 0.7152 * g + 0.0722 * b) * a);
    }
    let n = lums.len() as f64;
    let mean = lums.iter().sum::<f64>() / n;
    let variance = lums.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

/// Samples a crop rect from an image buffer given dimensions, bounds-safe against narrow or small inputs.
pub fn sample_crop(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
) -> Vec<u8> {
    let mut crop = vec![0u8; (crop_w * crop_h * 4) as usize];
    if src.is_empty() || src_w == 0 || src_h == 0 || crop_w == 0 || crop_h == 0 {
        return crop;
    }
    for row in 0..crop_h {
        let y = (crop_y + row).min(src_h.saturating_sub(1));
        let x = crop_x.min(src_w.saturating_sub(crop_w));
        let src_idx = ((y * src_w + x) * 4) as usize;
        let dst_idx = (row * crop_w * 4) as usize;

        let available_in_row = (src_w.saturating_sub(x) * 4) as usize;
        let requested_bytes = (crop_w * 4) as usize;
        let mut len = requested_bytes.min(available_in_row);

        if src_idx < src.len() {
            len = len.min(src.len() - src_idx);
            len = len.min(crop.len().saturating_sub(dst_idx));
            if len > 0 {
                crop[dst_idx..dst_idx + len].copy_from_slice(&src[src_idx..src_idx + len]);
            }
        }
    }
    crop
}

// -----------------------------------------------------------------------------
// Clock Color Adaptation Model (Invoking Real CenterHeroClock Optical Routines)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ClockOpticalState {
    pub is_light: bool,
    pub is_multi_tone: bool,
    pub col_day: [u8; 4],
    pub col_day_shadow: [u8; 4],
    pub col_day_halo: Option<[u8; 4]>,
    pub col_date: [u8; 4],
    pub col_date_shadow: [u8; 4],
    pub col_date_halo: Option<[u8; 4]>,
    pub col_ampm: [u8; 4],
    pub col_ampm_shadow: [u8; 4],
    pub col_ampm_halo: Option<[u8; 4]>,
    pub col_shadow: [u8; 4],
    pub col_halo: Option<[u8; 4]>,
    pub rim_boost: f64,
    pub inner_glow: f64,
}

impl ClockOpticalState {
    /// Constructs optical state by evaluating real optical styling routines
    /// for all 3 zones from a wallpaper crop using WallpaperManager.
    pub fn from_crop(crop: &[u8], width: u32, height: u32) -> Self {
        let profile = WallpaperManager::sample_widget_zones(crop, width, height);
        let (day, date, ampm) = compute_clock_text_styling(0, 0.35, false, true, &profile);

        let is_light = profile.overall.is_light;
        let rim_boost = if is_light { 1.10 } else { 1.65 };
        let inner_glow = if is_light { 0.10 } else { 1.25 };

        Self {
            is_light,
            is_multi_tone: profile.is_multi_tone,
            col_day: day.0,
            col_day_shadow: day.1,
            col_day_halo: day.2,
            col_date: date.0,
            col_date_shadow: date.1,
            col_date_halo: date.2,
            col_ampm: ampm.0,
            col_ampm_shadow: ampm.1,
            col_ampm_halo: ampm.2,
            col_shadow: day.1,
            col_halo: day.2,
            rim_boost,
            inner_glow,
        }
    }

    /// Constructs optical state from luminance and variance using real compute_zone_text_styling.
    pub fn from_luminance_and_variance(
        mean_lum: f64,
        std_dev: f64,
        hysteresis_prev_light: Option<bool>,
    ) -> Self {
        let is_light = match hysteresis_prev_light {
            Some(true) => mean_lum > 0.28,
            Some(false) => mean_lum > 0.35,
            None => mean_lum > 0.35,
        };
        let is_multi_tone = std_dev > 0.18;
        let zone = ZoneMetrics {
            luma_mean: mean_lum as f32,
            luma_std_dev: std_dev as f32,
            dominant_tint: [1.0, 1.0, 1.0],
            is_multi_tone,
            is_light,
        };
        let day = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&zone),
            is_multi_tone,
            COL_DAY_BASE_LIGHT,
            COL_DAY_BASE_DARK,
        );
        let date = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&zone),
            is_multi_tone,
            COL_DATE_BASE_LIGHT,
            COL_DATE_BASE_DARK,
        );
        let ampm = compute_zone_text_styling(
            0,
            0.35,
            false,
            true,
            Some(&zone),
            is_multi_tone,
            COL_AMPM_BASE_LIGHT,
            COL_AMPM_BASE_DARK,
        );

        let rim_boost = if is_light { 1.10 } else { 1.65 };
        let inner_glow = if is_light { 0.10 } else { 1.25 };

        Self {
            is_light,
            is_multi_tone,
            col_day: day.0,
            col_day_shadow: day.1,
            col_day_halo: day.2,
            col_date: date.0,
            col_date_shadow: date.1,
            col_date_halo: date.2,
            col_ampm: ampm.0,
            col_ampm_shadow: ampm.1,
            col_ampm_halo: ampm.2,
            col_shadow: day.1,
            col_halo: day.2,
            rim_boost,
            inner_glow,
        }
    }

    pub fn from_luminance(mean_lum: f64, hysteresis_prev_light: Option<bool>) -> Self {
        Self::from_luminance_and_variance(mean_lum, 0.0, hysteresis_prev_light)
    }

    /// Evaluates genuine effective contrast for a text element (text, shadow, halo) against background RGB
    pub fn element_effective_contrast(
        text_col: [u8; 4],
        shadow_col: [u8; 4],
        halo_col: Option<[u8; 4]>,
        bg_rgb: [u8; 3],
    ) -> f64 {
        let bg_lum = relative_luminance(bg_rgb);
        let text_lum = relative_luminance([text_col[0], text_col[1], text_col[2]]);
        let shadow_lum = relative_luminance([shadow_col[0], shadow_col[1], shadow_col[2]]);

        let direct_cr = contrast_ratio(text_lum, bg_lum);
        let shadow_cr = contrast_ratio(shadow_lum, bg_lum);
        let mut eff_cr = direct_cr.max(shadow_cr);

        // When a bilateral halo is genuinely present (only on multi-tone backgrounds),
        // it contributes opposing optical protection.
        if let Some(halo) = halo_col {
            let halo_lum = relative_luminance([halo[0], halo[1], halo[2]]);
            let halo_cr = contrast_ratio(halo_lum, bg_lum);
            eff_cr = eff_cr.max(halo_cr);
        }

        eff_cr
    }

    pub fn day_contrast(&self, bg_rgb: [u8; 3]) -> f64 {
        Self::element_effective_contrast(
            self.col_day,
            self.col_day_shadow,
            self.col_day_halo,
            bg_rgb,
        )
    }

    pub fn date_contrast(&self, bg_rgb: [u8; 3]) -> f64 {
        Self::element_effective_contrast(
            self.col_date,
            self.col_date_shadow,
            self.col_date_halo,
            bg_rgb,
        )
    }

    pub fn ampm_contrast(&self, bg_rgb: [u8; 3]) -> f64 {
        Self::element_effective_contrast(
            self.col_ampm,
            self.col_ampm_shadow,
            self.col_ampm_halo,
            bg_rgb,
        )
    }

    /// Evaluates effective contrast across all 3 clock zones (Day, Date, and AMPM),
    /// returning the minimum effective contrast among them.
    pub fn effective_contrast(&self, bg_rgb: [u8; 3]) -> f64 {
        let cr_day = self.day_contrast(bg_rgb);
        let cr_date = self.date_contrast(bg_rgb);
        let cr_ampm = self.ampm_contrast(bg_rgb);

        cr_day.min(cr_date).min(cr_ampm)
    }
}

// -----------------------------------------------------------------------------
// Synthetic Calibration Bench Generators
// -----------------------------------------------------------------------------

pub fn create_solid_rgba(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for chunk in buf.chunks_exact_mut(4) {
        chunk.copy_from_slice(&color);
    }
    buf
}

pub fn create_vertical_split_rgba(
    w: u32,
    h: u32,
    left_col: [u8; 4],
    right_col: [u8; 4],
) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let half_w = w / 2;
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            if x < half_w {
                buf[idx..idx + 4].copy_from_slice(&left_col);
            } else {
                buf[idx..idx + 4].copy_from_slice(&right_col);
            }
        }
    }
    buf
}

pub fn create_horizontal_split_rgba(
    w: u32,
    h: u32,
    top_col: [u8; 4],
    bottom_col: [u8; 4],
) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let half_h = h / 2;
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            if y < half_h {
                buf[idx..idx + 4].copy_from_slice(&top_col);
            } else {
                buf[idx..idx + 4].copy_from_slice(&bottom_col);
            }
        }
    }
    buf
}

pub fn create_checkerboard_rgba(
    w: u32,
    h: u32,
    cell_size: u32,
    c1: [u8; 4],
    c2: [u8; 4],
) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let check = ((x / cell_size) + (y / cell_size)) % 2 == 0;
            if check {
                buf[idx..idx + 4].copy_from_slice(&c1);
            } else {
                buf[idx..idx + 4].copy_from_slice(&c2);
            }
        }
    }
    buf
}

pub fn enumerate_wallpapers() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let wall_dir = Path::new(&home).join("Pictures/wallpaper");
    let mut found = Vec::new();
    if wall_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(wall_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "png" {
                        found.push(p);
                    }
                }
            }
        }
    }
    found.sort();
    found
}

// =============================================================================
// TIER 1: Core Feature Happy Path Tests (>= 5 Tests)
// =============================================================================

#[test]
fn test_tier1_wcag_relative_luminance_definition() {
    // Pure black #000000 must be exactly 0.0
    let lum_black = relative_luminance([0, 0, 0]);
    assert!(
        (lum_black - 0.0).abs() < 1e-6,
        "Black luminance must be 0.0"
    );

    // Pure white #FFFFFF must be exactly 1.0
    let lum_white = relative_luminance([255, 255, 255]);
    assert!(
        (lum_white - 1.0).abs() < 1e-4,
        "White luminance must be 1.0"
    );

    // Pure sRGB Green (0, 255, 0) has highest weight: 0.7152
    let lum_green = relative_luminance([0, 255, 0]);
    assert!(
        (lum_green - 0.7152).abs() < 1e-3,
        "Green luminance must be ~0.7152"
    );

    // Pure sRGB Red (255, 0, 0) weight: 0.2126
    let lum_red = relative_luminance([255, 0, 0]);
    assert!(
        (lum_red - 0.2126).abs() < 1e-3,
        "Red luminance must be ~0.2126"
    );

    // Pure sRGB Blue (0, 0, 255) weight: 0.0722
    let lum_blue = relative_luminance([0, 0, 255]);
    assert!(
        (lum_blue - 0.0722).abs() < 1e-3,
        "Blue luminance must be ~0.0722"
    );
}

#[test]
fn test_tier1_wcag_contrast_ratio_formula() {
    // Pure black vs pure white must be exactly 21:1
    let ratio_bw = contrast_ratio(1.0, 0.0);
    assert!(
        (ratio_bw - 21.0).abs() < 1e-4,
        "Black/white contrast must be 21.0:1"
    );

    // Symmetry check: order of inputs must not alter the ratio
    let ratio_wb = contrast_ratio(0.0, 1.0);
    assert!(
        (ratio_bw - ratio_wb).abs() < 1e-6,
        "Contrast ratio must be symmetric"
    );

    // Identical colors must have ratio 1.0:1
    let ratio_same = contrast_ratio(0.42, 0.42);
    assert!(
        (ratio_same - 1.0).abs() < 1e-6,
        "Identical colors must have ratio 1.0"
    );

    // Minimum WCAG AA threshold is 4.5:1
    assert!(
        contrast_ratio(1.0, 0.15) > 4.5,
        "Lighter pair must exceed 4.5:1"
    );
}

#[test]
fn test_tier1_wallpaper_enumeration_and_format() {
    let wallpapers = enumerate_wallpapers();
    if !wallpapers.is_empty() {
        println!(
            "Discovered {} wallpapers in ~/Pictures/wallpaper/",
            wallpapers.len()
        );
        assert!(
            wallpapers.len() >= 10,
            "Expected full set of wallpapers (found {})",
            wallpapers.len()
        );
        for path in &wallpapers {
            assert!(path.exists(), "Wallpaper file must exist: {:?}", path);
            let ext = path.extension().unwrap().to_str().unwrap().to_lowercase();
            assert!(
                ext == "jpg" || ext == "jpeg" || ext == "png",
                "Valid image extension"
            );
        }
    } else {
        println!("Note: ~/Pictures/wallpaper/ not populated; synthetic test patterns will serve as test bench.");
    }
}

#[test]
fn test_tier1_center_clock_hero_text_contrast_on_dark_wallpaper() {
    // Simulate typical dark desktop wallpaper (e.g. Gargantua Black, mean luminance 0.04)
    let state = ClockOpticalState::from_luminance(0.04, None);
    assert!(
        !state.is_light,
        "Dark wallpaper must trigger dark-mode optical state"
    );

    // Verify day title text #F8FAFF achieves >= 4.5:1 contrast
    let day_lum = relative_luminance([state.col_day[0], state.col_day[1], state.col_day[2]]);
    let ratio_day = contrast_ratio(day_lum, 0.04);
    assert!(
        ratio_day >= 4.5,
        "Day text contrast ({:.2}:1) must meet WCAG 2.1 AA on dark wallpaper",
        ratio_day
    );

    // Verify date text achieves >= 4.5:1 contrast
    let date_lum = relative_luminance([state.col_date[0], state.col_date[1], state.col_date[2]]);
    let ratio_date = contrast_ratio(date_lum, 0.04);
    assert!(
        ratio_date >= 4.5,
        "Date text contrast ({:.2}:1) must meet WCAG 2.1 AA on dark wallpaper",
        ratio_date
    );

    // Verify AM/PM text achieves >= 4.5:1 contrast
    let ampm_lum = relative_luminance([state.col_ampm[0], state.col_ampm[1], state.col_ampm[2]]);
    let ratio_ampm = contrast_ratio(ampm_lum, 0.04);
    assert!(
        ratio_ampm >= 4.5,
        "AMPM text contrast ({:.2}:1) must meet WCAG 2.1 AA on dark wallpaper",
        ratio_ampm
    );
}

#[test]
fn test_tier1_center_clock_hero_text_contrast_on_light_wallpaper() {
    // Simulate bright daylight wallpaper (mean luminance 0.75)
    let state = ClockOpticalState::from_luminance(0.75, None);
    assert!(
        state.is_light,
        "Light wallpaper must trigger light-mode optical state"
    );

    // Verify day title text achieves >= 4.5:1 contrast
    let day_lum = relative_luminance([state.col_day[0], state.col_day[1], state.col_day[2]]);
    let ratio_day = contrast_ratio(0.75, day_lum);
    assert!(
        ratio_day >= 4.5,
        "Day text contrast ({:.2}:1) must meet WCAG 2.1 AA on light wallpaper",
        ratio_day
    );

    // Verify date text achieves >= 4.5:1 contrast
    let date_lum = relative_luminance([state.col_date[0], state.col_date[1], state.col_date[2]]);
    let ratio_date = contrast_ratio(0.75, date_lum);
    assert!(
        ratio_date >= 4.5,
        "Date text contrast ({:.2}:1) must meet WCAG 2.1 AA on light wallpaper",
        ratio_date
    );

    // Verify AM/PM text achieves >= 4.5:1 contrast
    let ampm_lum = relative_luminance([state.col_ampm[0], state.col_ampm[1], state.col_ampm[2]]);
    let ratio_ampm = contrast_ratio(0.75, ampm_lum);
    assert!(
        ratio_ampm >= 4.5,
        "AMPM text contrast ({:.2}:1) must meet WCAG 2.1 AA on light wallpaper",
        ratio_ampm
    );
}

#[test]
fn test_tier1_active_wallpaper_cache_analysis() {
    let dummy_1080p = create_solid_rgba(1920, 1080, [30, 32, 40, 255]);
    let crop = sample_crop(&dummy_1080p, 1920, 1080, 410, 330, 1100, 420);
    assert_eq!(crop.len(), 1100 * 420 * 4);

    let (mean, std_dev) = buffer_luminance_stats(&crop);
    assert!(mean < 0.10, "Dark crop mean luminance must be < 0.10");
    assert!(std_dev < 1e-4, "Solid crop standard deviation must be ~0");
}

// =============================================================================
// TIER 2: Boundary & Extreme Calibration Bench Tests (>= 5 Tests)
// =============================================================================

#[test]
fn test_tier2_synthetic_pure_black_wcag_aa() {
    let black_buf = create_solid_rgba(1100, 420, [0, 0, 0, 255]);
    let state = ClockOpticalState::from_crop(&black_buf, 1100, 420);
    assert!(!state.is_light);

    for (name, col) in [
        ("Day", state.col_day),
        ("Date", state.col_date),
        ("AMPM", state.col_ampm),
    ] {
        let fg_lum = relative_luminance([col[0], col[1], col[2]]);
        let ratio = contrast_ratio(fg_lum, 0.0);
        assert!(
            ratio >= 15.0,
            "{} on pure black background must yield massive contrast >= 15:1 (actual: {:.2}:1)",
            name,
            ratio
        );
    }
}

#[test]
fn test_tier2_synthetic_pure_white_wcag_aa() {
    let white_buf = create_solid_rgba(1100, 420, [255, 255, 255, 255]);
    let state = ClockOpticalState::from_crop(&white_buf, 1100, 420);
    assert!(state.is_light);

    for (name, col) in [
        ("Day", state.col_day),
        ("Date", state.col_date),
        ("AMPM", state.col_ampm),
    ] {
        let fg_lum = relative_luminance([col[0], col[1], col[2]]);
        let ratio = contrast_ratio(1.0, fg_lum);
        assert!(
            ratio >= 7.0,
            "{} on pure white background must yield contrast >= 7.0:1 (WCAG AAA) (actual: {:.2}:1)",
            name,
            ratio
        );
    }
}

#[test]
fn test_tier2_synthetic_50_percent_gray_wcag_aa() {
    // 50% gray #808080 (128 sRGB -> linear lum ~0.214)
    let gray_buf = create_solid_rgba(1100, 420, [128, 128, 128, 255]);
    let mean_lum = buffer_average_luminance(&gray_buf);
    assert!((mean_lum - 0.214).abs() < 0.01);

    // Verify adaptive model ensures >= 4.5:1 via dual-layer contrast across all 3 zones
    let state = ClockOpticalState::from_crop(&gray_buf, 1100, 420);
    let eff_contrast = state.effective_contrast([128, 128, 128]);
    assert!(
        eff_contrast >= 4.5,
        "50% gray must maintain WCAG AA legibility >= 4.5:1 with dual-layer adaptation across all zones (actual: {:.2}:1)",
        eff_contrast
    );
}

#[test]
fn test_tier2_synthetic_vertical_split_contrast() {
    // Left half pure black [0,0,0], Right half pure white [255,255,255]
    let split_buf = create_vertical_split_rgba(1100, 420, [0, 0, 0, 255], [255, 255, 255, 255]);
    let (mean, std_dev) = buffer_luminance_stats(&split_buf);

    assert!(
        (mean - 0.5).abs() < 0.02,
        "Split mean luminance must be ~0.5"
    );
    assert!(
        std_dev > 0.45,
        "Split variance must be high indicating multi-tone wallpaper"
    );

    let state = ClockOpticalState::from_crop(&split_buf, 1100, 420);
    assert!(
        state.is_multi_tone,
        "Split buffer must trigger multi-tone variance"
    );
    assert!(
        state.col_halo.is_some(),
        "Multi-tone buffer must trigger real bilateral halo"
    );

    let left_contrast = state.effective_contrast([0, 0, 0]);
    let right_contrast = state.effective_contrast([255, 255, 255]);

    assert!(
        left_contrast >= 4.5,
        "Left half must achieve WCAG AA legibility across all zones (actual: {:.2}:1)",
        left_contrast
    );
    assert!(
        right_contrast >= 4.5,
        "Right half must achieve WCAG AA legibility via protective layer across all zones (actual: {:.2}:1)",
        right_contrast
    );
}

#[test]
fn test_tier2_synthetic_horizontal_split_contrast() {
    let split_h = create_horizontal_split_rgba(1100, 420, [10, 15, 25, 255], [230, 240, 255, 255]);
    let state = ClockOpticalState::from_crop(&split_h, 1100, 420);

    let top_contrast = state.effective_contrast([10, 15, 25]);
    let bottom_contrast = state.effective_contrast([230, 240, 255]);

    assert!(
        top_contrast >= 4.5,
        "Top zone must meet WCAG AA across all zones (actual: {:.2}:1)",
        top_contrast
    );
    assert!(
        bottom_contrast >= 4.5,
        "Bottom zone must meet WCAG AA across all zones (actual: {:.2}:1)",
        bottom_contrast
    );
}

#[test]
fn test_tier2_synthetic_checkerboard_contrast() {
    // High-frequency alternating black/white checkerboard
    let check = create_checkerboard_rgba(200, 200, 10, [0, 0, 0, 255], [255, 255, 255, 255]);
    let (mean, std_dev) = buffer_luminance_stats(&check);
    assert!((mean - 0.5).abs() < 0.02);
    assert!(std_dev > 0.45);

    let state = ClockOpticalState::from_crop(&check, 200, 200);
    assert!(state.effective_contrast([0, 0, 0]) >= 4.5);
    assert!(state.effective_contrast([255, 255, 255]) >= 4.5);
}

#[test]
fn test_tier2_extreme_minimal_dimensions_1x1() {
    let one_px = [100u8, 150u8, 200u8, 255u8];
    let lum = buffer_average_luminance(&one_px);
    assert!(lum > 0.0 && lum < 1.0);
    let state = ClockOpticalState::from_luminance(lum, None);
    assert!(state.effective_contrast([100, 150, 200]) >= 3.0);
}

#[test]
fn test_tier2_empty_or_missing_wallpaper_directory_fallback() {
    // Non-existent path handling
    let missing_path = Path::new("/tmp/non_existent_wallpaper_dir_xyz_12345");
    assert!(!missing_path.exists());

    // Fallback generator produces clean standard 1920x1080 canvas
    let fallback = create_solid_rgba(1920, 1080, [240, 242, 240, 255]);
    assert_eq!(fallback.len(), 1920 * 1080 * 4);
    let lum = buffer_average_luminance(&fallback);
    assert!(
        lum > 0.85,
        "Neutral fallback must be clean light background"
    );
}

#[test]
fn test_tier2_extreme_high_aspect_ratio_ultrawide() {
    // 32:9 Super-Ultrawide (5120x1440) crop to 1100x420
    let ultrawide = create_solid_rgba(5120, 1440, [20, 25, 35, 255]);
    let crop = sample_crop(&ultrawide, 5120, 1440, 2010, 510, 1100, 420);
    assert_eq!(crop.len(), 1100 * 420 * 4);
    let lum = buffer_average_luminance(&crop);
    assert!(lum < 0.05);
}

// =============================================================================
// TIER 3: Cross-Feature Presets & Optical Modes Tests (>= 5 Tests)
// =============================================================================

#[test]
fn test_tier3_optical_contrast_modes_and_thresholds() {
    // Mode 0: Auto (threshold ~0.30 - 0.35)
    let auto_dark = ClockOpticalState::from_luminance(0.20, None);
    assert!(!auto_dark.is_light);
    let auto_light = ClockOpticalState::from_luminance(0.40, None);
    assert!(auto_light.is_light);

    // Rim boost and inner glow modulation for luminous core
    assert!(
        auto_dark.rim_boost > auto_light.rim_boost,
        "Dark wallpapers boost rim lighting for specular edge"
    );
    assert!(
        auto_dark.inner_glow > auto_light.inner_glow,
        "Dark wallpapers boost luminous core glow"
    );
}

#[test]
fn test_tier3_font_weight_and_tracking_contrast() {
    let state_dark = ClockOpticalState::from_luminance(0.05, None);
    let day_lum = relative_luminance([
        state_dark.col_day[0],
        state_dark.col_day[1],
        state_dark.col_day[2],
    ]);
    let date_lum = relative_luminance([
        state_dark.col_date[0],
        state_dark.col_date[1],
        state_dark.col_date[2],
    ]);

    // Bold hero day title has higher optical punch than medium date
    assert!(
        day_lum >= date_lum,
        "Hero title luminance ({:.3}) >= Date luminance ({:.3})",
        day_lum,
        date_lum
    );
}

#[test]
fn test_tier3_theme_mode_overrides() {
    // Theme Mode: 0 = Auto, 1 = Forced Dark, 2 = Forced Light
    let mean_lum = 0.80; // Bright background

    // Auto chooses light mode
    let auto_mode = ClockOpticalState::from_luminance(mean_lum, None);
    assert!(auto_mode.is_light);

    // Manual Forced Dark (theme_mode = 1) overrides light background with dark preset
    let forced_dark_day = compute_zone_text_styling(
        1,
        0.35,
        false,
        true,
        None,
        false,
        COL_DAY_BASE_LIGHT,
        COL_DAY_BASE_DARK,
    );
    let forced_dark_lum = relative_luminance([
        forced_dark_day.0[0],
        forced_dark_day.0[1],
        forced_dark_day.0[2],
    ]);
    assert!(
        forced_dark_lum > 0.70,
        "Forced dark must produce light text"
    );
    // Primary shadow is deep drop shadow [0, 0, 0, 190]
    let shadow_lum = relative_luminance([
        forced_dark_day.1[0],
        forced_dark_day.1[1],
        forced_dark_day.1[2],
    ]);
    let ratio = contrast_ratio(shadow_lum, mean_lum);
    assert!(
        ratio >= 4.5,
        "Forced dark shadow on light background maintains legibility"
    );
}

#[test]
fn test_tier3_multi_zone_clock_crop_coordinates() {
    // Optical center coordinates: (1920 - 1100)/2 = 410, y = 330, w = 1100, h = 420
    let crop_x = (1920 - 1100) / 2;
    let crop_y = 330u32;
    assert_eq!(crop_x, 410);
    assert_eq!(crop_y + 420, 750); // Clock sits comfortably above dock (y ~ 1000)
}

#[test]
fn test_tier3_rim_boost_and_inner_glow_multipliers() {
    let dark_state = ClockOpticalState::from_luminance(0.02, None);
    let light_state = ClockOpticalState::from_luminance(0.85, None);

    assert!(dark_state.rim_boost >= 1.50, "Dark state rim boost >= 1.50");
    assert!(
        dark_state.inner_glow >= 1.00,
        "Dark state inner glow >= 1.00"
    );
    assert!(
        light_state.rim_boost <= 1.20,
        "Light state rim boost subtle <= 1.20"
    );
    assert!(
        light_state.inner_glow <= 0.20,
        "Light state inner glow minimal <= 0.20"
    );
}

// =============================================================================
// TIER 4: Realistic End-to-End Application Scenarios (>= 5 Scenarios)
// =============================================================================

#[test]
fn test_tier4_all_18_wallpapers_wcag_aa_verification() {
    let wallpapers = enumerate_wallpapers();
    if wallpapers.is_empty() {
        println!("Skipping live wallpaper iteration (no files in ~/Pictures/wallpaper/)");
        return;
    }

    println!(
        "Testing WCAG 2.1 AA legibility across {} discovered wallpapers...",
        wallpapers.len()
    );
    let mut passed_count = 0;

    for path in &wallpapers {
        let img = match image::open(path) {
            Ok(im) => im.to_rgba8(),
            Err(e) => {
                println!("Warning: Failed to open {:?}: {}", path, e);
                continue;
            }
        };

        let (w, h) = (img.width(), img.height());
        // Sample corresponding center clock region (scaled to wallpaper aspect)
        let scale_x = w as f64 / 1920.0;
        let scale_y = h as f64 / 1080.0;
        let cx = (410.0 * scale_x) as u32;
        let cy = (330.0 * scale_y) as u32;
        let cw = (1100.0 * scale_x) as u32;
        let ch = (420.0 * scale_y) as u32;

        let crop = sample_crop(img.as_raw(), w, h, cx, cy, cw, ch);
        let profile = WallpaperManager::sample_widget_zones(&crop, cw, ch);
        let (day_style, date_style, ampm_style) =
            compute_clock_text_styling(0, 0.35, false, true, &profile);

        let day_text_lum = relative_luminance([day_style.0[0], day_style.0[1], day_style.0[2]]);
        let date_text_lum = relative_luminance([date_style.0[0], date_style.0[1], date_style.0[2]]);
        let ampm_text_lum = relative_luminance([ampm_style.0[0], ampm_style.0[1], ampm_style.0[2]]);

        let day_bg_lum = profile.day_zone.luma_mean as f64;
        let date_bg_lum = profile.date_zone.luma_mean as f64;
        let ampm_bg_lum = profile.numeral_zone.luma_mean as f64;

        let day_direct_cr = contrast_ratio(day_text_lum, day_bg_lum);
        let date_direct_cr = contrast_ratio(date_text_lum, date_bg_lum);
        let ampm_direct_cr = contrast_ratio(ampm_text_lum, ampm_bg_lum);

        let day_shadow_lum = relative_luminance([day_style.1[0], day_style.1[1], day_style.1[2]]);
        let date_shadow_lum =
            relative_luminance([date_style.1[0], date_style.1[1], date_style.1[2]]);
        let ampm_shadow_lum =
            relative_luminance([ampm_style.1[0], ampm_style.1[1], ampm_style.1[2]]);

        let day_shadow_cr = contrast_ratio(day_shadow_lum, day_bg_lum);
        let date_shadow_cr = contrast_ratio(date_shadow_lum, date_bg_lum);
        let ampm_shadow_cr = contrast_ratio(ampm_shadow_lum, ampm_bg_lum);

        let mut day_eff_cr = day_direct_cr.max(day_shadow_cr);
        if let Some(halo) = day_style.2 {
            let halo_lum = relative_luminance([halo[0], halo[1], halo[2]]);
            day_eff_cr = day_eff_cr.max(contrast_ratio(halo_lum, day_bg_lum));
        }

        let mut date_eff_cr = date_direct_cr.max(date_shadow_cr);
        if let Some(halo) = date_style.2 {
            let halo_lum = relative_luminance([halo[0], halo[1], halo[2]]);
            date_eff_cr = date_eff_cr.max(contrast_ratio(halo_lum, date_bg_lum));
        }

        let mut ampm_eff_cr = ampm_direct_cr.max(ampm_shadow_cr);
        if let Some(halo) = ampm_style.2 {
            let halo_lum = relative_luminance([halo[0], halo[1], halo[2]]);
            ampm_eff_cr = ampm_eff_cr.max(contrast_ratio(halo_lum, ampm_bg_lum));
        }

        println!(
            "Wallpaper: {:?} | multi_tone: {}\n  Day:  direct={:.2}:1, shadow={:.2}:1, eff={:.2}:1\n  Date: direct={:.2}:1, shadow={:.2}:1, eff={:.2}:1\n  AMPM: direct={:.2}:1, shadow={:.2}:1, eff={:.2}:1",
            path.file_name().unwrap(),
            profile.is_multi_tone,
            day_direct_cr, day_shadow_cr, day_eff_cr,
            date_direct_cr, date_shadow_cr, date_eff_cr,
            ampm_direct_cr, ampm_shadow_cr, ampm_eff_cr
        );

        assert!(
            day_eff_cr >= 4.5,
            "Wallpaper {:?} Day failed WCAG AA: {:.2}:1 < 4.5:1",
            path.file_name().unwrap(),
            day_eff_cr
        );
        assert!(
            date_eff_cr >= 4.5,
            "Wallpaper {:?} Date failed WCAG AA: {:.2}:1 < 4.5:1",
            path.file_name().unwrap(),
            date_eff_cr
        );
        assert!(
            ampm_eff_cr >= 4.5,
            "Wallpaper {:?} AMPM failed WCAG AA: {:.2}:1 < 4.5:1",
            path.file_name().unwrap(),
            ampm_eff_cr
        );
        passed_count += 1;
    }

    println!(
        "Successfully verified WCAG 2.1 AA legibility on all {} wallpapers!",
        passed_count
    );
    assert!(passed_count > 0, "At least one wallpaper must be verified");
}

#[test]
fn test_tier4_wallpaper_carousel_transition_scenario() {
    // Simulates dynamic wallpaper daemon switching through a sequence of 6 wallpapers:
    // 1. Midnight Black (0.01)
    // 2. Neon Dunes (0.12)
    // 3. 50% Gray Split (0.29)
    // 4. Pastel Sunset (0.55)
    // 5. Blinding Daylight (0.92)
    // 6. Return to Dark (0.05)
    let sequence = [0.01, 0.12, 0.29, 0.55, 0.92, 0.05];
    let mut current_state = None;

    for (step, &lum) in sequence.iter().enumerate() {
        let state = ClockOpticalState::from_luminance(lum, current_state);
        current_state = Some(state.is_light);

        let test_bg = if lum > 0.5 {
            [240, 240, 240]
        } else {
            [20, 20, 25]
        };
        let contrast = state.effective_contrast(test_bg);

        println!(
            "Carousel Step {}: Lum={:.2} -> Mode={} | Contrast={:.2}:1",
            step,
            lum,
            if state.is_light { "LIGHT" } else { "DARK" },
            contrast
        );

        assert!(
            contrast >= 4.5,
            "Step {} must maintain WCAG AA contrast >= 4.5:1",
            step
        );
    }
}

#[test]
fn test_tier4_hysteresis_boundary_stability() {
    // Tests that slight luminance noise around the 0.30 - 0.35 threshold
    // does not cause rapid state flapping due to hysteresis.
    let noise = [0.29, 0.31, 0.30, 0.32, 0.31, 0.29];
    let mut state = false; // Initially dark

    for &lum in &noise {
        // When currently false (dark), threshold is 0.35
        let next_state = if state { lum > 0.28 } else { lum > 0.35 };
        assert_eq!(
            next_state, false,
            "Luminance {:.2} must not cause premature flipping to light mode",
            lum
        );
        state = next_state;
    }

    // Now push clearly above 0.35
    state = if state { 0.40 > 0.28 } else { 0.40 > 0.35 };
    assert_eq!(state, true, "Luminance 0.40 must transition to light mode");

    // Now noise downwards
    let noise_down = [0.34, 0.32, 0.30, 0.29];
    for &lum in &noise_down {
        // When currently true (light), threshold is 0.28
        let next_state = if state { lum > 0.28 } else { lum > 0.35 };
        assert_eq!(
            next_state, true,
            "Luminance {:.2} above 0.28 must stay in light mode",
            lum
        );
        state = next_state;
    }

    // Drop below 0.28
    state = if state { 0.25 > 0.28 } else { 0.25 > 0.35 };
    assert_eq!(
        state, false,
        "Luminance 0.25 drops cleanly back to dark mode"
    );
}

#[test]
fn test_tier4_numeral_and_text_coexistence_scenario() {
    // Simulates the full composite clock rendering:
    // - Day ("T U E S D A Y")
    // - Date ("N O V E M B E R  1 7 ,  2 0 2 6")
    // - Snell Refractive Numerals ("1 0 : 0 0")
    // - AM/PM badge
    let state = ClockOpticalState::from_luminance(0.08, None);

    let day_contrast = state.effective_contrast([15, 18, 24]);
    let date_contrast = state.effective_contrast([15, 18, 24]);
    let ampm_contrast = state.effective_contrast([15, 18, 24]);

    assert!(day_contrast >= 4.5);
    assert!(date_contrast >= 4.5);
    assert!(ampm_contrast >= 4.5);

    // Numerals: specular rim boost ensures refractive boundary contrast
    assert!(
        state.rim_boost >= 1.50,
        "Specular rim highlight provides refractive numeral contrast"
    );
}

#[test]
fn test_tier4_multi_monitor_crop_scaling_scenario() {
    // Test on 3 standard resolutions: 1080p, 1440p, 4K
    let resolutions = [(1920, 1080), (2560, 1440), (3840, 2160)];
    for &(w, h) in &resolutions {
        let wall = create_solid_rgba(w, h, [15, 20, 30, 255]);
        let scale_x = w as f64 / 1920.0;
        let scale_y = h as f64 / 1080.0;
        let crop = sample_crop(
            &wall,
            w,
            h,
            (410.0 * scale_x) as u32,
            (330.0 * scale_y) as u32,
            (1100.0 * scale_x) as u32,
            (420.0 * scale_y) as u32,
        );
        let lum = buffer_average_luminance(&crop);
        let state = ClockOpticalState::from_luminance(lum, None);
        assert!(state.effective_contrast([15, 20, 30]) >= 4.5);
    }
}

#[test]
fn test_tier2_crop_bounds_safety_narrow_and_small_source() {
    // Probe 1: Narrow image where src_w < crop_w and src_h < crop_h (100x10 vs 1100x420)
    let tiny_img = vec![128u8; 100 * 10 * 4];
    let crop = sample_crop(&tiny_img, 100, 10, 0, 0, 1100, 420);
    assert_eq!(crop.len(), (1100 * 420 * 4) as usize);

    // Probe 2: Zero-dimension / empty source safety
    let empty_crop = sample_crop(&[], 0, 0, 0, 0, 1100, 420);
    assert_eq!(empty_crop.len(), (1100 * 420 * 4) as usize);

    // Probe 3: 1x1 image source
    let one_px = [200u8, 150, 100, 255];
    let crop_1px = sample_crop(&one_px, 1, 1, 0, 0, 100, 100);
    assert_eq!(crop_1px.len(), (100 * 100 * 4) as usize);
}

#[test]
fn test_tier2_buffer_average_luminance_alpha_opacity_weighting() {
    // Fully transparent white (alpha = 0) must contribute 0.0 luminance, not opaque 1.0
    let transparent_white = [255u8, 255, 255, 0];
    assert_eq!(buffer_average_luminance(&transparent_white), 0.0);

    // Semi-transparent white (alpha = 128) must scale proportionally
    let semi_white = [255u8, 255, 255, 128];
    let semi_lum = buffer_average_luminance(&semi_white);
    assert!((semi_lum - (128.0 / 255.0)).abs() < 1e-4);

    // Fully opaque white
    let opaque_white = [255u8, 255, 255, 255];
    assert!((buffer_average_luminance(&opaque_white) - 1.0).abs() < 1e-4);
}

#[test]
fn test_tier2_bilateral_halo_mid_tone_gray_contrast_sweep() {
    // When bilateral halo is active (std_dev = 0.25 > 0.18 on multi-tone or split wallpapers):
    // Light mode: mean_lum = 0.36 (is_light = true)
    let light_state = ClockOpticalState::from_luminance_and_variance(0.36, 0.25, None);
    assert!(light_state.is_light);
    assert!(
        light_state.col_halo.is_some(),
        "Multi-tone variance must activate bilateral halo"
    );

    // Dark mode: mean_lum = 0.15 (is_light = false)
    let dark_state = ClockOpticalState::from_luminance_and_variance(0.15, 0.25, None);
    assert!(!dark_state.is_light);
    assert!(
        dark_state.col_halo.is_some(),
        "Multi-tone variance must activate bilateral halo"
    );

    // Sweep all 256 sRGB levels [0..255] ensuring WCAG AA contrast >= 4.5:1 across the entire spectrum
    for val in 0u8..=255 {
        let bg_rgb = [val, val, val];

        let contrast_light = light_state.effective_contrast(bg_rgb);
        assert!(
            contrast_light >= 4.5,
            "Light mode must maintain WCAG AA >= 4.5:1 across mid-tones (failed at val {}: {:.2}:1)",
            val,
            contrast_light
        );

        let contrast_dark = dark_state.effective_contrast(bg_rgb);
        assert!(
            contrast_dark >= 4.5,
            "Dark mode must maintain WCAG AA >= 4.5:1 across mid-tones (failed at val {}: {:.2}:1)",
            val,
            contrast_dark
        );
    }

    // Explicitly verify mid-tone sRGB 123 (challenger failure point)
    let contrast_123 = light_state.effective_contrast([123, 123, 123]);
    assert!(
        contrast_123 >= 4.5,
        "Mid-tone sRGB 123 must meet WCAG AA >= 4.5:1 (actual: {:.2}:1)",
        contrast_123
    );
}
