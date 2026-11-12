//! Unified Liquid Glass Design Tokens.
//!
//! Enforces Apple HIG optical design discipline across all widgets:
//! - Single primary accent with strict semantic statuses (error, warning, success)
//! - Typography scale: 11 / 13 / 15 / 20 / 48 px with Medium/SemiBold weights
//! - Obsidian backdrop tint for dark mode and window occlusion
//! - Standardized corner radii and inner/outer stroke hierarchy

pub struct Tokens;

impl Tokens {
    // ── Typography Scale ──
    pub const TYPE_CAPTION: f32 = 11.0;
    pub const TYPE_BODY_SM: f32 = 13.0;
    pub const TYPE_BODY: f32 = 15.0;
    pub const TYPE_TITLE: f32 = 20.0;
    pub const TYPE_CLOCK: f32 = 48.0;

    // ── Geometry & Layout ──
    pub const RADIUS_OUTER: f32 = 28.0;
    pub const RADIUS_INNER_TILE: f32 = 14.0;
    pub const RADIUS_PILL: f32 = 100.0;

    pub const ISLAND_COMPACT_W: f32 = 230.0;
    pub const ISLAND_COMPACT_H: f32 = 38.0;
    pub const ISLAND_EXPANDED_W: f32 = 460.0;
    pub const ISLAND_EXPANDED_H: f32 = 148.0;

    // ── Palette (Single Crisp Accent + Semantic Statuses) ──
    /// Single unified accent: Apple electric cyan-blue
    pub const ACCENT: [u8; 4] = [58, 163, 255, 255];
    /// Soft translucent accent fill (12% opacity)
    pub const ACCENT_FILL: [u8; 4] = [58, 163, 255, 30];

    // Semantic status colors
    pub const STATUS_RED: [u8; 4] = [255, 69, 58, 255]; // Destructive / Power / Alert
    pub const STATUS_AMBER: [u8; 4] = [255, 159, 10, 255]; // Warning / Low battery
    pub const STATUS_GREEN: [u8; 4] = [48, 209, 88, 255]; // Success / Charging

    // Text hierarchy
    pub const TEXT_PRIMARY: [u8; 4] = [245, 247, 250, 255];
    pub const TEXT_SECONDARY: [u8; 4] = [200, 208, 222, 255];
    pub const TEXT_TERTIARY: [u8; 4] = [140, 150, 168, 210];
    pub const TEXT_SHADOW: [u8; 4] = [0, 0, 0, 90];

    // High-contrast Light Mode Text (WCAG AAA > 15:1 against light glass)
    pub const TEXT_PRIMARY_LIGHT: [u8; 4] = [8, 12, 20, 255];
    pub const TEXT_SECONDARY_LIGHT: [u8; 4] = [18, 24, 34, 255];
    pub const TEXT_TERTIARY_LIGHT: [u8; 4] = [28, 36, 50, 255];
    pub const TEXT_SHADOW_LIGHT: [u8; 4] = [0, 0, 0, 0];

    // Obsidian Glass (Apple Dynamic Island dark smoky glass backdrop)
    pub const OBSIDIAN_BACKDROP: [u8; 4] = [16, 18, 24, 230]; // 90% opacity
    pub const CLEAR_BACKDROP_DARK: [u8; 4] = [18, 22, 28, 90]; // 35% opacity
    pub const CLEAR_BACKDROP_LIGHT: [u8; 4] = [245, 247, 252, 140]; // 55% opacity

    // Rims & Outlines
    /// Outer shell specular rim (subtle 1px highlight, ONLY on outer shell)
    pub const RIM_OUTER_DARK: [u8; 4] = [255, 255, 255, 45];
    /// Outer shell boundary in light mode (subtle dark hairline for crisp demarcation)
    pub const RIM_OUTER_LIGHT: [u8; 4] = [0, 0, 0, 42];

    // Inner element fills
    pub const INNER_TILE_BG_DARK: [u8; 4] = [255, 255, 255, 18]; // 7% white
    pub const INNER_TILE_BG_ROW: [u8; 4] = [255, 255, 255, 22]; // 8.6% white
    pub const INNER_TILE_BG_HOVER_DARK: [u8; 4] = [255, 255, 255, 30]; // 12% white
    pub const INNER_TILE_BG_LIGHT: [u8; 4] = [0, 0, 0, 16]; // 6% black fill for crisp contrast
    pub const INNER_TILE_BG_HOVER_LIGHT: [u8; 4] = [0, 0, 0, 30]; // 12% black fill on hover

    /// Top highlight on inner tiles (subtle bevel instead of 4-sided wireframe, soft 15-18% white composite)
    pub const INNER_TOP_HIGHLIGHT_DARK: [u8; 4] = [255, 255, 255, 18];
    pub const INNER_TOP_HIGHLIGHT_LIGHT: [u8; 4] = [255, 255, 255, 180];
}
