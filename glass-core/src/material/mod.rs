use serde::{Serialize, Deserialize};
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct MaterialParams {
    pub ior: f32,
    pub roughness: f32,
    pub transmission: f32,
    pub absorption: [f32; 3], // RGB tint/absorption
    pub refraction_strength: f32,
    pub dispersion: f32,
    pub fresnel_strength: f32,
    pub specular_strength: f32,
    pub tint: [f32; 4], // RGBA tint
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
            dispersion: 0.05,
            fresnel_strength: 1.0,
            specular_strength: 1.0,
            tint: [1.0, 1.0, 1.0, 0.0],
            blur_radius: 10.0,
        }
    }
}

pub trait GlassMaterial {
    fn params(&self) -> MaterialParams;
}

impl Material {
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
            // ... more presets
            _ => MaterialParams::default(),
        }
    }
}
