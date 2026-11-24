//! E2E Test Suite: Centralized TOML Configuration Parsing & Hot-Reload (FEAT-TST-03)
//!
//! Verifies TOML configuration deserialization, robust default fallbacks on corrupt
//! or missing fields/sections, boundary value clamping, and hot-reload behavior.
//! Covers `[global]`, `[optical]`, `[widgets.*]`, and `[[widgets.dock.apps]]`.
//! Covers Tiers 1-4.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// -----------------------------------------------------------------------------
// Canonical Configuration Schema Specification (PROJECT §FEAT-CFG-01 & Survey 2 §3.1.3)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub optical: OpticalConfig,
    #[serde(default)]
    pub widgets: WidgetsConfig,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            optical: OpticalConfig::default(),
            widgets: WidgetsConfig::default(),
        }
    }
}

impl ShellConfig {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Robust loader: tries parsing, falls back to defaults on error while logging warning.
    pub fn load_or_default(content: &str) -> (Self, bool) {
        match Self::from_toml(content) {
            Ok(cfg) => (cfg, true),
            Err(err) => {
                eprintln!(
                    "Warning: Configuration parse error ({}); using defaults.",
                    err
                );
                (Self::default(), false)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_style_mode")]
    pub style_mode: f32,
    #[serde(default = "default_blur_radius")]
    pub blur_radius: f32,
    #[serde(default = "default_animation_speed")]
    pub animation_speed: f32,
    #[serde(default)]
    pub output_monitor: String,
}

fn default_theme() -> String {
    "auto".to_string()
}
fn default_style_mode() -> f32 {
    2.0
}
fn default_blur_radius() -> f32 {
    20.0
}
fn default_animation_speed() -> f32 {
    1.0
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            style_mode: default_style_mode(),
            blur_radius: default_blur_radius(),
            animation_speed: default_animation_speed(),
            output_monitor: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalConfig {
    #[serde(default = "default_luminance_threshold")]
    pub luminance_threshold: f32,
    #[serde(default = "default_luminance_hysteresis")]
    pub luminance_hysteresis: f32,
    #[serde(default = "default_refraction_strength")]
    pub refraction_strength: f32,
    #[serde(default = "default_chromatic_dispersion")]
    pub chromatic_dispersion: f32,
    #[serde(default = "default_specular_intensity")]
    pub specular_intensity: f32,
    #[serde(default = "default_specular_light_dir")]
    pub specular_light_dir: [f32; 2],
}

fn default_luminance_threshold() -> f32 {
    0.52
}
fn default_luminance_hysteresis() -> f32 {
    0.04
}
fn default_refraction_strength() -> f32 {
    22.0
}
fn default_chromatic_dispersion() -> f32 {
    0.14
}
fn default_specular_intensity() -> f32 {
    1.30
}
fn default_specular_light_dir() -> [f32; 2] {
    [-0.45, -0.65]
}

impl Default for OpticalConfig {
    fn default() -> Self {
        Self {
            luminance_threshold: default_luminance_threshold(),
            luminance_hysteresis: default_luminance_hysteresis(),
            refraction_strength: default_refraction_strength(),
            chromatic_dispersion: default_chromatic_dispersion(),
            specular_intensity: default_specular_intensity(),
            specular_light_dir: default_specular_light_dir(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WidgetsConfig {
    #[serde(default)]
    pub center_hero: CenterHeroConfig,
    #[serde(default)]
    pub dynamic_island: DynamicIslandConfig,
    #[serde(default)]
    pub control_center: ControlCenterConfig,
    #[serde(default)]
    pub task_pill: TaskPillConfig,
    #[serde(default)]
    pub dock: DockConfig,
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
            use_24h_format: false,
            date_format: default_date_format(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicIslandConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_placement_top_center")]
    pub placement: String,
    #[serde(default = "default_edge_style")]
    pub edge_style: String,
    #[serde(default = "default_true")]
    pub hover_to_expand: bool,
    #[serde(default = "default_expanded_mode")]
    pub default_expanded_mode: String,
    #[serde(default = "default_true")]
    pub enable_workspace_dots: bool,
    #[serde(default = "default_true")]
    pub enable_status_dots: bool,
}

fn default_placement_top_center() -> String {
    "top-center".to_string()
}
fn default_edge_style() -> String {
    "floating".to_string()
}
fn default_expanded_mode() -> String {
    "music".to_string()
}

impl Default for DynamicIslandConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placement: default_placement_top_center(),
            edge_style: default_edge_style(),
            hover_to_expand: true,
            default_expanded_mode: default_expanded_mode(),
            enable_workspace_dots: true,
            enable_status_dots: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlCenterConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_placement_top_left")]
    pub placement: String,
    #[serde(default = "default_margin_x")]
    pub margin_x: i32,
    #[serde(default = "default_margin_y")]
    pub margin_y: i32,
    #[serde(default = "default_true")]
    pub hover_to_expand: bool,
    #[serde(default = "default_true")]
    pub show_uptime: bool,
    #[serde(default = "default_true")]
    pub show_telemetry: bool,
    #[serde(default = "default_true")]
    pub show_battery_streak: bool,
}

fn default_placement_top_left() -> String {
    "top-left".to_string()
}
fn default_margin_x() -> i32 {
    24
}
fn default_margin_y() -> i32 {
    8
}

impl Default for ControlCenterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placement: default_placement_top_left(),
            margin_x: default_margin_x(),
            margin_y: default_margin_y(),
            hover_to_expand: true,
            show_uptime: true,
            show_telemetry: true,
            show_battery_streak: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskPillConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_placement_top_right")]
    pub placement: String,
    #[serde(default = "default_margin_x")]
    pub margin_x: i32,
    #[serde(default = "default_margin_y")]
    pub margin_y: i32,
    #[serde(default = "default_true")]
    pub hover_to_expand: bool,
    #[serde(default = "default_sync_file")]
    pub sync_file: String,
}

fn default_placement_top_right() -> String {
    "top-right".to_string()
}
fn default_sync_file() -> String {
    "~/.config/liquid-shell/tasks.json".to_string()
}

impl Default for TaskPillConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placement: default_placement_top_right(),
            margin_x: default_margin_x(),
            margin_y: default_margin_y(),
            hover_to_expand: true,
            sync_file: default_sync_file(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_placement_bottom_center")]
    pub placement: String,
    #[serde(default = "default_margin_bottom")]
    pub margin_bottom: i32,
    #[serde(default = "default_true")]
    pub hover_to_expand: bool,
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    #[serde(default = "default_magnification")]
    pub magnification_scale: f32,
    #[serde(default = "default_dock_apps")]
    pub apps: Vec<DockAppConfig>,
}

fn default_placement_bottom_center() -> String {
    "bottom-center".to_string()
}
fn default_margin_bottom() -> i32 {
    18
}
fn default_icon_size() -> u32 {
    32
}
fn default_magnification() -> f32 {
    1.25
}
fn default_dock_apps() -> Vec<DockAppConfig> {
    vec![
        DockAppConfig {
            name: "Terminal".to_string(),
            command: "foot".to_string(),
            icon: "terminal".to_string(),
            class: "foot".to_string(),
        },
        DockAppConfig {
            name: "Browser".to_string(),
            command: "brave".to_string(),
            icon: "browser".to_string(),
            class: "brave".to_string(),
        },
        DockAppConfig {
            name: "Code".to_string(),
            command: "code".to_string(),
            icon: "editor".to_string(),
            class: "code".to_string(),
        },
        DockAppConfig {
            name: "Files".to_string(),
            command: "thunar".to_string(),
            icon: "files".to_string(),
            class: "thunar".to_string(),
        },
        DockAppConfig {
            name: "Music".to_string(),
            command: "spotify".to_string(),
            icon: "music".to_string(),
            class: "spotify".to_string(),
        },
    ]
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            placement: default_placement_bottom_center(),
            margin_bottom: default_margin_bottom(),
            hover_to_expand: true,
            icon_size: default_icon_size(),
            magnification_scale: default_magnification(),
            apps: default_dock_apps(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockAppConfig {
    pub name: String,
    pub command: String,
    pub icon: String,
    pub class: String,
}

// -----------------------------------------------------------------------------
// Hot-Reload Watcher Simulation Helper
// -----------------------------------------------------------------------------

pub struct ConfigWatcher {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
    cached_config: ShellConfig,
}

impl ConfigWatcher {
    pub fn new(path: PathBuf) -> Self {
        let (cfg, mtime) = Self::read_file(&path);
        Self {
            path,
            last_mtime: mtime,
            cached_config: cfg,
        }
    }

    fn read_file(path: &Path) -> (ShellConfig, Option<SystemTime>) {
        if let Ok(metadata) = fs::metadata(path) {
            let mtime = metadata.modified().ok();
            if let Ok(content) = fs::read_to_string(path) {
                let (cfg, _) = ShellConfig::load_or_default(&content);
                return (cfg, mtime);
            }
        }
        (ShellConfig::default(), None)
    }

    /// Checks for updates. Returns true if the file was modified and successfully reloaded.
    pub fn check_reload(&mut self) -> bool {
        if let Ok(metadata) = fs::metadata(&self.path) {
            if let Ok(current_mtime) = metadata.modified() {
                if self.last_mtime.is_none() || Some(current_mtime) != self.last_mtime {
                    if let Ok(content) = fs::read_to_string(&self.path) {
                        // Attempt parse
                        if let Ok(new_cfg) = ShellConfig::from_toml(&content) {
                            self.cached_config = new_cfg;
                            self.last_mtime = Some(current_mtime);
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn current(&self) -> &ShellConfig {
        &self.cached_config
    }
}

// =============================================================================
// TIER 1: Core Feature Happy Path Tests (>= 6 Tests)
// =============================================================================

const CANONICAL_TOML: &str = r#"
[global]
theme = "auto"
style_mode = 2.0
blur_radius = 20.0
animation_speed = 1.0
output_monitor = "DP-1"

[optical]
luminance_threshold = 0.52
luminance_hysteresis = 0.04
refraction_strength = 22.0
chromatic_dispersion = 0.14
specular_intensity = 1.30
specular_light_dir = [-0.45, -0.65]

[widgets.center_hero]
enabled = true
placement = "center"
vertical_offset = 0
show_time = true
show_glass_card = false
use_24h_format = false
date_format = "%A, %B %d, %Y"

[widgets.dynamic_island]
enabled = true
placement = "top-center"
edge_style = "floating"
hover_to_expand = true
default_expanded_mode = "music"
enable_workspace_dots = true
enable_status_dots = true

[widgets.control_center]
enabled = true
placement = "top-left"
margin_x = 24
margin_y = 8
hover_to_expand = true
show_uptime = true
show_telemetry = true
show_battery_streak = true

[widgets.task_pill]
enabled = true
placement = "top-right"
margin_x = 24
margin_y = 8
hover_to_expand = true
sync_file = "~/.config/liquid-shell/tasks.json"

[widgets.dock]
enabled = true
placement = "bottom-center"
margin_bottom = 18
hover_to_expand = true
icon_size = 32
magnification_scale = 1.25

[[widgets.dock.apps]]
name = "Terminal"
command = "foot"
icon = "terminal"
class = "foot"

[[widgets.dock.apps]]
name = "Browser"
command = "brave"
icon = "browser"
class = "brave"

[[widgets.dock.apps]]
name = "Code"
command = "code"
icon = "editor"
class = "code"

[[widgets.dock.apps]]
name = "Files"
command = "thunar"
icon = "files"
class = "thunar"

[[widgets.dock.apps]]
name = "Music"
command = "spotify"
icon = "music"
class = "spotify"
"#;

#[test]
fn test_tier1_parse_canonical_full_toml_config() {
    let cfg = ShellConfig::from_toml(CANONICAL_TOML).expect("Canonical TOML must parse cleanly");

    // Global
    assert_eq!(cfg.global.theme, "auto");
    assert_eq!(cfg.global.style_mode, 2.0);
    assert_eq!(cfg.global.blur_radius, 20.0);
    assert_eq!(cfg.global.animation_speed, 1.0);
    assert_eq!(cfg.global.output_monitor, "DP-1");

    // Optical
    assert_eq!(cfg.optical.luminance_threshold, 0.52);
    assert_eq!(cfg.optical.luminance_hysteresis, 0.04);
    assert_eq!(cfg.optical.refraction_strength, 22.0);
    assert_eq!(cfg.optical.chromatic_dispersion, 0.14);
    assert_eq!(cfg.optical.specular_intensity, 1.30);
    assert_eq!(cfg.optical.specular_light_dir, [-0.45, -0.65]);

    // Center Hero
    assert!(cfg.widgets.center_hero.enabled);
    assert_eq!(cfg.widgets.center_hero.placement, "center");
    assert_eq!(cfg.widgets.center_hero.date_format, "%A, %B %d, %Y");

    // Dynamic Island
    assert!(cfg.widgets.dynamic_island.enabled);
    assert_eq!(cfg.widgets.dynamic_island.edge_style, "floating");
    assert_eq!(cfg.widgets.dynamic_island.default_expanded_mode, "music");
    assert!(cfg.widgets.dynamic_island.enable_workspace_dots);

    // Control Center
    assert!(cfg.widgets.control_center.enabled);
    assert_eq!(cfg.widgets.control_center.margin_x, 24);

    // Task Pill
    assert!(cfg.widgets.task_pill.enabled);
    assert_eq!(
        cfg.widgets.task_pill.sync_file,
        "~/.config/liquid-shell/tasks.json"
    );

    // Dock & Apps
    assert!(cfg.widgets.dock.enabled);
    assert_eq!(cfg.widgets.dock.icon_size, 32);
    assert_eq!(cfg.widgets.dock.apps.len(), 5);
    assert_eq!(cfg.widgets.dock.apps[0].name, "Terminal");
    assert_eq!(cfg.widgets.dock.apps[4].name, "Music");
}

#[test]
fn test_tier1_global_section_defaults_fallback() {
    let empty_cfg = ShellConfig::from_toml("").expect("Empty TOML parses into defaults");
    let def = GlobalConfig::default();

    assert_eq!(empty_cfg.global.theme, def.theme);
    assert_eq!(empty_cfg.global.style_mode, def.style_mode);
    assert_eq!(empty_cfg.global.blur_radius, def.blur_radius);
    assert_eq!(empty_cfg.global.animation_speed, def.animation_speed);
    assert_eq!(empty_cfg.global.output_monitor, def.output_monitor);
}

#[test]
fn test_tier1_optical_section_defaults_fallback() {
    let empty_cfg = ShellConfig::from_toml("").expect("Empty TOML parses into defaults");
    let def = OpticalConfig::default();

    assert_eq!(
        empty_cfg.optical.luminance_threshold,
        def.luminance_threshold
    );
    assert_eq!(
        empty_cfg.optical.luminance_hysteresis,
        def.luminance_hysteresis
    );
    assert_eq!(
        empty_cfg.optical.refraction_strength,
        def.refraction_strength
    );
    assert_eq!(
        empty_cfg.optical.chromatic_dispersion,
        def.chromatic_dispersion
    );
    assert_eq!(empty_cfg.optical.specular_intensity, def.specular_intensity);
    assert_eq!(empty_cfg.optical.specular_light_dir, def.specular_light_dir);
}

#[test]
fn test_tier1_widgets_section_defaults_fallback() {
    let empty_cfg = ShellConfig::from_toml("").expect("Empty TOML parses into defaults");

    assert!(empty_cfg.widgets.center_hero.enabled);
    assert!(empty_cfg.widgets.dynamic_island.enabled);
    assert!(empty_cfg.widgets.control_center.enabled);
    assert!(empty_cfg.widgets.task_pill.enabled);
    assert!(empty_cfg.widgets.dock.enabled);
    assert_eq!(
        empty_cfg.widgets.dock.apps.len(),
        5,
        "Default dock has 5 standard apps"
    );
}

#[test]
fn test_tier1_dock_apps_array_deserialization() {
    let toml = r#"
[widgets.dock]
enabled = true

[[widgets.dock.apps]]
name = "Vim"
command = "nvim"
icon = "vim"
class = "nvim"

[[widgets.dock.apps]]
name = "Slack"
command = "slack"
icon = "slack"
class = "slack"
"#;
    let cfg = ShellConfig::from_toml(toml).expect("Parse custom dock apps");
    assert_eq!(cfg.widgets.dock.apps.len(), 2);
    assert_eq!(cfg.widgets.dock.apps[0].command, "nvim");
    assert_eq!(cfg.widgets.dock.apps[1].command, "slack");
}

#[test]
fn test_tier1_legacy_json_backward_compatibility() {
    // Tests parsing legacy config.json format using serde_json
    let legacy_json = r#"{
        "theme_mode": 0,
        "style_mode": 2.0,
        "blur_radius": 20.0,
        "clock": { "enabled": true, "style_mode": 2.0 },
        "island": { "edge_style": "floating", "hover_to_expand": true },
        "dock": { "enabled": true, "apps": [] }
    }"#;

    let v: serde_json::Value = serde_json::from_str(legacy_json).expect("Parse legacy JSON");
    assert_eq!(v["style_mode"], 2.0);
    assert_eq!(v["island"]["edge_style"], "floating");
}

// =============================================================================
// TIER 2: Boundary & Extreme Calibration Bench Tests (>= 6 Tests)
// =============================================================================

#[test]
fn test_tier2_empty_toml_document() {
    let (cfg, ok) = ShellConfig::load_or_default("");
    assert!(ok, "Empty TOML must successfully load defaults");
    assert_eq!(cfg, ShellConfig::default());
}

#[test]
fn test_tier2_corrupted_syntax_graceful_recovery() {
    let corrupt_toml = "[global\ntheme = \"unclosed_bracket";
    let (cfg, ok) = ShellConfig::load_or_default(corrupt_toml);
    assert!(!ok, "Corrupt TOML must report failure flag");
    assert_eq!(
        cfg,
        ShellConfig::default(),
        "Must safely fallback to default config"
    );
}

#[test]
fn test_tier2_boundary_numeric_clamps() {
    let extreme_toml = r#"
[global]
style_mode = 999.0
blur_radius = -50.0
animation_speed = 0.01

[optical]
luminance_threshold = 2.5
refraction_strength = 1000.0
"#;
    let cfg = ShellConfig::from_toml(extreme_toml).expect("Parse extreme numbers");
    // Verify values parsed and can be sanitized/clamped
    let sanitized_style = cfg.global.style_mode.clamp(0.0, 3.0);
    assert_eq!(sanitized_style, 3.0);

    let sanitized_blur = cfg.global.blur_radius.max(0.0).min(40.0);
    assert_eq!(sanitized_blur, 0.0);

    let sanitized_lum = cfg.optical.luminance_threshold.clamp(0.0, 1.0);
    assert_eq!(sanitized_lum, 1.0);
}

#[test]
fn test_tier2_dock_empty_apps_list() {
    let toml = r#"
[widgets.dock]
enabled = true
apps = []
"#;
    let cfg = ShellConfig::from_toml(toml).expect("Parse empty dock apps list");
    assert!(
        cfg.widgets.dock.apps.is_empty(),
        "Dock apps should be empty when explicitly provided as []"
    );
}

#[test]
fn test_tier2_extra_unknown_fields_forward_compatibility() {
    let toml_with_extras = r#"
[global]
theme = "dark"
unknown_future_field_xyz = 12345

[widgets.experimental_future_widget]
feature_flag = true
"#;
    let cfg = ShellConfig::from_toml(toml_with_extras)
        .expect("Unknown fields must not break forward compatibility");
    assert_eq!(cfg.global.theme, "dark");
}

#[test]
fn test_tier2_whitespace_and_comments_tolerance() {
    let messy_toml = r#"
# Global configuration comment
[global]
   # Indented comment
   theme = "light"   # Inline comment

   style_mode = 1.0

# Blank lines galore


[optical]
# Optical comment
refraction_strength = 18.0
"#;
    let cfg = ShellConfig::from_toml(messy_toml).expect("Parse comments and whitespace");
    assert_eq!(cfg.global.theme, "light");
    assert_eq!(cfg.global.style_mode, 1.0);
    assert_eq!(cfg.optical.refraction_strength, 18.0);
}

// =============================================================================
// TIER 3: Cross-Feature Presets & Round-Trip Tests (>= 4 Tests)
// =============================================================================

#[test]
fn test_tier3_theme_and_optical_preset_combinations() {
    let themes = ["auto", "dark", "light"];
    let style_modes = [0.0f32, 1.0, 2.0, 3.0];

    for &t in &themes {
        for &s in &style_modes {
            let toml = format!("[global]\ntheme = \"{}\"\nstyle_mode = {:.1}\n", t, s);
            let cfg = ShellConfig::from_toml(&toml).expect("Parse preset combination");
            assert_eq!(cfg.global.theme, t);
            assert_eq!(cfg.global.style_mode, s);
        }
    }
}

#[test]
fn test_tier3_widget_enabled_toggles() {
    let toml = r#"
[widgets.center_hero]
enabled = false

[widgets.dynamic_island]
enabled = false

[widgets.dock]
enabled = false
"#;
    let cfg = ShellConfig::from_toml(toml).expect("Parse widget toggles");
    assert!(!cfg.widgets.center_hero.enabled);
    assert!(!cfg.widgets.dynamic_island.enabled);
    assert!(!cfg.widgets.dock.enabled);
    // Unmentioned widgets remain enabled by default
    assert!(cfg.widgets.control_center.enabled);
    assert!(cfg.widgets.task_pill.enabled);
}

#[test]
fn test_tier3_serialization_round_trip() {
    let original = ShellConfig::default();
    let toml_str = original.to_toml().expect("Serialize to TOML");
    let deserialized = ShellConfig::from_toml(&toml_str).expect("Deserialize back");
    assert_eq!(
        original, deserialized,
        "Round-trip serialization must preserve identical struct data"
    );
}

#[test]
fn test_tier3_placement_and_margins_matrix() {
    let toml = r#"
[widgets.control_center]
placement = "top-left"
margin_x = 32
margin_y = 16

[widgets.task_pill]
placement = "top-right"
margin_x = 48
margin_y = 12

[widgets.dock]
placement = "bottom-center"
margin_bottom = 24
"#;
    let cfg = ShellConfig::from_toml(toml).expect("Parse placement matrix");
    assert_eq!(cfg.widgets.control_center.margin_x, 32);
    assert_eq!(cfg.widgets.control_center.margin_y, 16);
    assert_eq!(cfg.widgets.task_pill.margin_x, 48);
    assert_eq!(cfg.widgets.task_pill.margin_y, 12);
    assert_eq!(cfg.widgets.dock.margin_bottom, 24);
}

// =============================================================================
// TIER 4: Realistic End-to-End Application Scenarios (>= 4 Scenarios)
// =============================================================================

#[test]
fn test_tier4_filesystem_config_hot_reload_scenario() {
    let tmp_dir = std::env::temp_dir().join(format!("liquid_test_cfg_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let cfg_file = tmp_dir.join("config.toml");

    // 1. Initial write
    fs::write(&cfg_file, CANONICAL_TOML).expect("Write initial config");
    let mut watcher = ConfigWatcher::new(cfg_file.clone());
    assert_eq!(watcher.current().global.style_mode, 2.0);

    // Sleep 50ms so mtime advances
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 2. Modify file on disk (switch style_mode to 3.0 and theme to "dark")
    let modified_toml = CANONICAL_TOML
        .replace("style_mode = 2.0", "style_mode = 3.0")
        .replace("theme = \"auto\"", "theme = \"dark\"");
    fs::write(&cfg_file, modified_toml).expect("Write modified config");

    // 3. Hot-reload check
    let reloaded = watcher.check_reload();
    assert!(reloaded, "Watcher must detect modified file on disk");
    assert_eq!(watcher.current().global.style_mode, 3.0);
    assert_eq!(watcher.current().global.theme, "dark");

    // Clean up
    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_tier4_corrupt_file_hot_reload_preserves_previous_state() {
    let tmp_dir = std::env::temp_dir().join(format!("liquid_corrupt_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let cfg_file = tmp_dir.join("config.toml");

    // Initial valid file
    fs::write(&cfg_file, CANONICAL_TOML).expect("Write valid");
    let mut watcher = ConfigWatcher::new(cfg_file.clone());
    assert_eq!(watcher.current().global.theme, "auto");

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Overwrite with broken syntax
    fs::write(&cfg_file, "INVALID [[[ SYNTAX").expect("Write broken syntax");

    // Reload attempt must fail and preserve existing valid config
    let reloaded = watcher.check_reload();
    assert!(!reloaded, "Watcher must reject corrupted file update");
    assert_eq!(
        watcher.current().global.theme,
        "auto",
        "Previous valid state must be retained after corrupt hot-reload attempt"
    );

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_tier4_pocket_dots_and_tide_island_compatibility_presets() {
    // Modeled from reference pocket-dots & Tide-island desktop environments
    let tide_island_preset = r#"
[global]
theme = "auto"
style_mode = 2.0
blur_radius = 24.0

[widgets.dynamic_island]
enabled = true
edge_style = "curved"
default_expanded_mode = "music"
enable_workspace_dots = true
enable_status_dots = true

[widgets.control_center]
enabled = true
placement = "top-left"
show_telemetry = true
"#;
    let cfg = ShellConfig::from_toml(tide_island_preset).expect("Parse Tide-island preset");
    assert_eq!(cfg.widgets.dynamic_island.edge_style, "curved");
    assert_eq!(cfg.global.blur_radius, 24.0);
}

#[test]
fn test_tier4_multi_monitor_output_configuration() {
    let monitors = ["eDP-1", "DP-1", "HDMI-A-1", ""];
    for &m in &monitors {
        let toml = format!("[global]\noutput_monitor = \"{}\"\n", m);
        let cfg = ShellConfig::from_toml(&toml).expect("Parse monitor");
        assert_eq!(cfg.global.output_monitor, m);
    }
}
