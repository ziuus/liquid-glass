use crate::material::{Material, MaterialParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Shape {
    RoundedRectangle { radius: f32 },
    Capsule,
    Circle,
    Squircle { radius: f32 },
    CustomSdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassSurface {
    pub shape: Shape,
    pub bounds: [f32; 4], // x, y, w, h (normalized for now)
    pub material: Material,
    pub custom_params: Option<MaterialParams>,
    pub curvature: f32,
    pub bevel: f32,
    pub pointer_pos: [f32; 2],
    pub element_type: f32,
    pub deformation: f32,
    pub style_mode: f32,
}

impl GlassSurface {
    pub fn new() -> Self {
        Self {
            shape: Shape::RoundedRectangle { radius: 16.0 },
            bounds: [0.0, 0.0, 1.0, 1.0],
            material: Material::ClearGlass,
            custom_params: None,
            curvature: 0.0,
            bevel: 0.0,
            pointer_pos: [0.5, 0.5],
            element_type: 0.0,
            deformation: 0.0,
            style_mode: 0.0,
        }
    }

    pub fn material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn bounds(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.bounds = [x, y, w, h];
        self
    }

    fn get_mut_params(&mut self) -> &mut MaterialParams {
        if self.custom_params.is_none() {
            self.custom_params = Some(self.material.default_params());
        }
        self.custom_params.as_mut().unwrap()
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.shape = Shape::RoundedRectangle { radius };
        self
    }

    pub fn blur(mut self, blur_radius: f32) -> Self {
        self.get_mut_params().blur_radius = blur_radius;
        self
    }

    pub fn refraction(mut self, strength: f32) -> Self {
        self.get_mut_params().refraction_strength = strength;
        self
    }

    pub fn dispersion(mut self, amount: f32) -> Self {
        self.get_mut_params().dispersion = amount;
        self
    }

    pub fn fresnel(mut self, strength: f32) -> Self {
        self.get_mut_params().fresnel_strength = strength;
        self
    }

    pub fn specular(mut self, strength: f32) -> Self {
        self.get_mut_params().specular_strength = strength;
        self
    }

    pub fn roughness(mut self, amount: f32) -> Self {
        self.get_mut_params().roughness = amount;
        self
    }

    pub fn tint(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.get_mut_params().tint = [r, g, b, a];
        self
    }

    pub fn curvature(mut self, curvature: f32) -> Self {
        self.curvature = curvature;
        self
    }

    pub fn bevel(mut self, bevel: f32) -> Self {
        self.bevel = bevel;
        self
    }

    pub fn pointer_pos(mut self, x: f32, y: f32) -> Self {
        self.pointer_pos = [x, y];
        self
    }

    pub fn element_type(mut self, el_type: f32) -> Self {
        self.element_type = el_type;
        self
    }

    pub fn deformation(mut self, def: f32) -> Self {
        self.deformation = def;
        self
    }

    pub fn style_mode(mut self, mode: f32) -> Self {
        self.style_mode = mode;
        self
    }
}

impl Default for GlassSurface {
    fn default() -> Self {
        Self::new()
    }
}

pub mod overlap;

pub fn load_from_json(json: &str) -> Result<Vec<GlassSurface>, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn save_to_json(surfaces: &[GlassSurface]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(surfaces)
}
