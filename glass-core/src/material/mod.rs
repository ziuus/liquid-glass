use serde::{Deserialize, Serialize};

/// Apple-style material types matching NSVisualEffectView materials
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialType {
    /// Ultra-thin glass for main window backgrounds
    WindowBackground,
    /// Thick glass for sidebars with high blur
    Sidebar,
    /// Medium glass for titlebar areas
    Titlebar,
    /// Frosted glass for menus
    Menu,
    /// Sheet glass for popovers
    Popover,
    /// Minimal glass for tooltips
    Tooltip,
    /// Dark thick glass for HUD overlays
    HUD,
    /// Modal sheet glass
    Sheet,
    /// Content behind page background
    UnderPageBackground,
    /// Custom material with manual parameters
    Custom,
}

/// Blend mode for light/dark appearance
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BlendMode {
    /// Behind window content shows through
    BehindWindow,
    /// Within window content blends
    WithinWindow,
}

/// Core vibrancy material matching Apple's implementation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VibrancyMaterial {
    /// Gaussian blur radius in pixels
    pub blur_radius: f32,
    /// Saturation boost factor (1.0 = no boost, 1.5 = Apple typical)
    pub saturation_boost: f32,
    /// Base material tint color (RGBA)
    pub tint_color: [f32; 4],
    /// How much tint vs background shows through (0.0-1.0)
    pub tint_strength: f32,
    /// Overall material opacity (0.0-1.0)
    pub opacity: f32,
    /// Blend mode for light appearance
    pub light_mode: BlendMode,
    /// Blend mode for dark appearance
    pub dark_mode: BlendMode,
    /// Enable reduced transparency for accessibility
    pub reduced_transparency: bool,
}

impl Default for VibrancyMaterial {
    fn default() -> Self {
        Self::window_background()
    }
}

impl VibrancyMaterial {
    /// Ultra-thin glass for window backgrounds
    pub fn window_background() -> Self {
        Self {
            blur_radius: 80.0,
            saturation_boost: 1.5,
            tint_color: [0.98, 0.98, 0.98, 1.0],
            tint_strength: 0.15,
            opacity: 0.85,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Thick glass for sidebars
    pub fn sidebar() -> Self {
        Self {
            blur_radius: 60.0,
            saturation_boost: 1.6,
            tint_color: [0.96, 0.96, 0.96, 1.0],
            tint_strength: 0.25,
            opacity: 0.95,
            light_mode: BlendMode::WithinWindow,
            dark_mode: BlendMode::WithinWindow,
            reduced_transparency: false,
        }
    }

    /// Medium glass for titlebar
    pub fn titlebar() -> Self {
        Self {
            blur_radius: 40.0,
            saturation_boost: 1.4,
            tint_color: [0.97, 0.97, 0.97, 1.0],
            tint_strength: 0.2,
            opacity: 0.88,
            light_mode: BlendMode::WithinWindow,
            dark_mode: BlendMode::WithinWindow,
            reduced_transparency: false,
        }
    }

    /// Frosted glass for menus
    pub fn menu() -> Self {
        Self {
            blur_radius: 35.0,
            saturation_boost: 1.55,
            tint_color: [0.99, 0.99, 0.99, 1.0],
            tint_strength: 0.3,
            opacity: 0.92,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Sheet glass for popovers
    pub fn popover() -> Self {
        Self {
            blur_radius: 50.0,
            saturation_boost: 1.5,
            tint_color: [0.98, 0.98, 0.98, 1.0],
            tint_strength: 0.25,
            opacity: 0.9,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Minimal glass for tooltips
    pub fn tooltip() -> Self {
        Self {
            blur_radius: 25.0,
            saturation_boost: 1.3,
            tint_color: [0.95, 0.95, 0.95, 1.0],
            tint_strength: 0.4,
            opacity: 0.95,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Dark thick glass for HUD overlays
    pub fn hud() -> Self {
        Self {
            blur_radius: 70.0,
            saturation_boost: 1.7,
            tint_color: [0.15, 0.15, 0.15, 1.0],
            tint_strength: 0.5,
            opacity: 0.92,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Modal sheet glass
    pub fn sheet() -> Self {
        Self {
            blur_radius: 90.0,
            saturation_boost: 1.6,
            tint_color: [0.98, 0.98, 0.98, 1.0],
            tint_strength: 0.2,
            opacity: 0.88,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Content behind page background
    pub fn under_page_background() -> Self {
        Self {
            blur_radius: 100.0,
            saturation_boost: 1.8,
            tint_color: [0.95, 0.95, 0.95, 1.0],
            tint_strength: 0.15,
            opacity: 0.75,
            light_mode: BlendMode::BehindWindow,
            dark_mode: BlendMode::BehindWindow,
            reduced_transparency: false,
        }
    }

    /// Get material for a specific type
    pub fn from_type(material_type: MaterialType) -> Self {
        match material_type {
            MaterialType::WindowBackground => Self::window_background(),
            MaterialType::Sidebar => Self::sidebar(),
            MaterialType::Titlebar => Self::titlebar(),
            MaterialType::Menu => Self::menu(),
            MaterialType::Popover => Self::popover(),
            MaterialType::Tooltip => Self::tooltip(),
            MaterialType::HUD => Self::hud(),
            MaterialType::Sheet => Self::sheet(),
            MaterialType::UnderPageBackground => Self::under_page_background(),
            MaterialType::Custom => Self::default(),
        }
    }

    /// Apply dark mode appearance
    pub fn with_dark_mode(mut self) -> Self {
        // Darken tint and adjust opacity for dark mode
        self.tint_color[0] *= 0.2;
        self.tint_color[1] *= 0.2;
        self.tint_color[2] *= 0.2;
        self.opacity = (self.opacity + 0.1).min(1.0);
        self.tint_strength = (self.tint_strength + 0.1).min(1.0);
        self
    }

    /// Apply reduced transparency for accessibility
    pub fn with_reduced_transparency(mut self) -> Self {
        self.reduced_transparency = true;
        self.blur_radius *= 0.3; // Drastically reduce blur
        self.opacity = (self.opacity + 0.15).min(1.0); // More opaque
        self.tint_strength = (self.tint_strength + 0.3).min(1.0); // More tint
        self
    }
}

/// Legacy material enum for backward compatibility
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(deprecated)]
pub enum Material {
    ClearGlass,
    LiquidGlass,
    FrostedGlass,
    Acrylic,
    Crystal,
    Water,
    Ice,
    Gel,
    SmokedGlass,
    Custom,
}

impl Material {
    /// Legacy method for backwards compatibility
    pub fn default_params(&self) -> MaterialParams {
        match self {
            Material::LiquidGlass => MaterialParams {
                ior: 1.33,
                roughness: 0.05,
                refraction_strength: 0.8,
                dispersion: 0.08,
                fresnel_strength: 0.7,
                specular_strength: 0.9,
                blur_radius: 16.0,
                ..Default::default()
            },
            Material::FrostedGlass => MaterialParams {
                roughness: 0.8,
                blur_radius: 32.0,
                ..Default::default()
            },
            _ => MaterialParams::default(),
        }
    }
}

/// Legacy material parameters
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(deprecated)]
pub struct MaterialParams {
    pub ior: f32,
    pub roughness: f32,
    pub transmission: f32,
    pub absorption: [f32; 3],
    pub refraction_strength: f32,
    pub dispersion: f32,
    pub fresnel_strength: f32,
    pub specular_strength: f32,
    pub tint: [f32; 4],
    pub blur_radius: f32,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            ior: 1.5,
            roughness: 0.1,
            transmission: 1.0,
            absorption: [0.0, 0.0, 0.0],
            refraction_strength: 1.0,
            dispersion: 0.02,
            fresnel_strength: 1.0,
            specular_strength: 0.5,
            tint: [1.0, 1.0, 1.0, 0.9],
            blur_radius: 20.0,
        }
    }
}

/// Glass material combining type and parameters
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GlassMaterial {
    pub material_type: MaterialType,
    pub vibrancy: VibrancyMaterial,
    /// Corner radius in pixels
    pub corner_radius: f32,
    /// Shadow blur radius
    pub shadow_blur: f32,
    /// Shadow offset
    pub shadow_offset: [f32; 2],
    /// Shadow opacity
    pub shadow_opacity: f32,
}

impl Default for GlassMaterial {
    fn default() -> Self {
        Self {
            material_type: MaterialType::WindowBackground,
            vibrancy: VibrancyMaterial::default(),
            corner_radius: 10.0,
            shadow_blur: 40.0,
            shadow_offset: [0.0, 4.0],
            shadow_opacity: 0.25,
        }
    }
}

impl GlassMaterial {
    pub fn new(material_type: MaterialType) -> Self {
        Self {
            material_type,
            vibrancy: VibrancyMaterial::from_type(material_type),
            ..Default::default()
        }
    }

    pub fn sidebar() -> Self {
        Self::new(MaterialType::Sidebar).with_shadow(30.0, [0.0, 2.0], 0.2)
    }

    pub fn menu() -> Self {
        Self::new(MaterialType::Menu)
            .with_corner_radius(8.0)
            .with_shadow(35.0, [0.0, 8.0], 0.3)
    }

    pub fn popover() -> Self {
        Self::new(MaterialType::Popover)
            .with_corner_radius(8.0)
            .with_shadow(50.0, [0.0, 12.0], 0.35)
    }

    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn with_shadow(mut self, blur: f32, offset: [f32; 2], opacity: f32) -> Self {
        self.shadow_blur = blur;
        self.shadow_offset = offset;
        self.shadow_opacity = opacity;
        self
    }

    pub fn with_dark_mode(mut self) -> Self {
        self.vibrancy = self.vibrancy.with_dark_mode();
        self
    }

    pub fn with_reduced_transparency(mut self) -> Self {
        self.vibrancy = self.vibrancy.with_reduced_transparency();
        self
    }
}
