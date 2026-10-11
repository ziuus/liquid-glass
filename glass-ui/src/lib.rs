use glass_core::geometry::{GlassSurface, Shape};
use glass_core::material::{Material, MaterialParams};

pub struct GlassPanel {
    pub surface: GlassSurface,
}

impl GlassPanel {
    pub fn new() -> Self {
        Self {
            surface: GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 24.0 })
                .blur(32.0)
                .refraction(0.8)
                .dispersion(0.05)
                .fresnel(0.8)
                .specular(1.0)
                .tint(1.0, 1.0, 1.0, 0.05)
        }
    }
}

pub struct GlassButton {
    pub surface: GlassSurface,
    pub is_hovered: bool,
    pub is_pressed: bool,
}

impl GlassButton {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            surface: GlassSurface::new()
                .material(Material::Acrylic)
                .shape(Shape::Capsule)
                .bounds(x, y, w, h)
                .blur(16.0)
                .fresnel(0.5)
                .specular(0.6)
                .tint(1.0, 1.0, 1.0, 0.1),
            is_hovered: false,
            is_pressed: false,
        }
    }

    pub fn update_state(&mut self) {
        if self.is_pressed {
            self.surface = self.surface.clone()
                .tint(1.0, 1.0, 1.0, 0.2)
                .blur(8.0);
        } else if self.is_hovered {
            self.surface = self.surface.clone()
                .tint(1.0, 1.0, 1.0, 0.15)
                .blur(24.0);
        } else {
            self.surface = self.surface.clone()
                .tint(1.0, 1.0, 1.0, 0.1)
                .blur(16.0);
        }
    }
}

pub struct GlassPopup {
    pub surface: GlassSurface,
}

impl GlassPopup {
    pub fn new() -> Self {
        Self {
            surface: GlassSurface::new()
                .material(Material::FrostedGlass)
                .shape(Shape::RoundedRectangle { radius: 32.0 })
                .blur(48.0)
                .tint(0.0, 0.0, 0.0, 0.4) // Dark mode popup
        }
    }
}
