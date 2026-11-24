//! Centralized configuration schema for liquid-shell.
//! Defines configuration structures for widgets, including CenterHeroConfig.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContrastPreset {
    #[default]
    Auto,
    HighContrast,
    Subtle,
    Crystalline,
}

impl ContrastPreset {
    pub fn as_f32(&self) -> f32 {
        match self {
            Self::Auto => 0.0,
            Self::HighContrast => 1.0,
            Self::Subtle => 2.0,
            Self::Crystalline => 3.0,
        }
    }

    pub fn parse_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "high_contrast" | "highcontrast" | "high" => Self::HighContrast,
            "subtle" | "soft" => Self::Subtle,
            "crystalline" | "crystal" => Self::Crystalline,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CenterHeroConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_placement_center")]
    pub placement: String,
    #[serde(default)]
    pub vertical_offset: i32,
    #[serde(default = "default_true")]
    pub show_time: bool,
    #[serde(default)]
    pub show_glass_card: bool,
    #[serde(default)]
    pub contrast_preset: ContrastPreset,
    #[serde(default = "default_luminance_threshold")]
    pub luminance_threshold: f32,
    #[serde(default = "default_rim_boost")]
    pub rim_boost: f32,
    #[serde(default = "default_inner_glow")]
    pub inner_glow: f32,
    #[serde(default = "default_true")]
    pub adaptive_halo: bool,
    #[serde(default = "default_style_mode")]
    pub style_mode: f32,
    #[serde(default = "default_blur_radius")]
    pub blur_radius: f32,
    #[serde(default)]
    pub use_24h_format: bool,
    #[serde(default = "default_date_format")]
    pub date_format: String,
}

fn default_true() -> bool {
    true
}

fn default_placement_center() -> String {
    "center".to_string()
}

fn default_luminance_threshold() -> f32 {
    0.35
}

fn default_rim_boost() -> f32 {
    1.0
}

fn default_inner_glow() -> f32 {
    0.0
}

fn default_style_mode() -> f32 {
    2.0
}

fn default_blur_radius() -> f32 {
    20.0
}

fn default_date_format() -> String {
    "%A, %B %d, %Y".to_string()
}

impl Default for CenterHeroConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placement: default_placement_center(),
            vertical_offset: 0,
            show_time: true,
            show_glass_card: false,
            contrast_preset: ContrastPreset::Auto,
            luminance_threshold: default_luminance_threshold(),
            rim_boost: default_rim_boost(),
            inner_glow: default_inner_glow(),
            adaptive_halo: true,
            style_mode: default_style_mode(),
            blur_radius: default_blur_radius(),
            use_24h_format: false,
            date_format: default_date_format(),
        }
    }
}
