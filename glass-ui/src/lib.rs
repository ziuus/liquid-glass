use glass_core::geometry::{GlassSurface, Shape};
use glass_core::material::Material;
use glass_core::animation::{Spring, SpringParams};

/// A high-level glass container (window, card, HUD panel, or floating island).
#[derive(Debug, Clone)]
pub struct GlassCard {
    pub bounds: [f32; 4], // [x, y, w, h] normalized 0.0..1.0
    pub shape: Shape,
    pub material: Material,
    pub blur_radius: f32,
    pub refraction: f32,
    pub dispersion: f32,
    pub bevel: f32,
    pub specular: f32,
    pub fresnel: f32,
    pub tint: [f32; 4],
}

impl GlassCard {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            bounds: [x, y, w, h],
            shape: Shape::RoundedRectangle { radius: 24.0 },
            material: Material::LiquidGlass,
            blur_radius: 24.0,
            refraction: 0.70,
            dispersion: 0.03,
            bevel: 10.0,
            specular: 0.95,
            fresnel: 0.70,
            tint: [1.0, 1.0, 1.0, 0.06],
        }
    }

    pub fn island(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(x, y, w, h).shape(Shape::Capsule).bevel(12.0).specular(1.0)
    }

    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    pub fn blur(mut self, blur: f32) -> Self {
        self.blur_radius = blur;
        self
    }

    pub fn refraction(mut self, refraction: f32) -> Self {
        self.refraction = refraction;
        self
    }

    pub fn dispersion(mut self, dispersion: f32) -> Self {
        self.dispersion = dispersion;
        self
    }

    pub fn bevel(mut self, bevel: f32) -> Self {
        self.bevel = bevel;
        self
    }

    pub fn specular(mut self, specular: f32) -> Self {
        self.specular = specular;
        self
    }

    pub fn tint(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.tint = [r, g, b, a];
        self
    }

    pub fn to_surface(&self) -> GlassSurface {
        GlassSurface::new()
            .material(self.material)
            .shape(self.shape)
            .bounds(self.bounds[0], self.bounds[1], self.bounds[2], self.bounds[3])
            .blur(self.blur_radius)
            .refraction(self.refraction)
            .dispersion(self.dispersion)
            .bevel(self.bevel)
            .specular(self.specular)
            .fresnel(self.fresnel)
            .tint(self.tint[0], self.tint[1], self.tint[2], self.tint[3])
    }
}

/// An interactive tactile glass button with spring-driven physics.
#[derive(Debug, Clone)]
pub struct GlassButton {
    pub bounds: [f32; 4],
    pub label: String,
    pub is_hovered: bool,
    pub is_pressed: bool,
    pub scale_spring: Spring,
    pub hover_spring: Spring,
    pub shape: Shape,
}

impl GlassButton {
    pub fn new(x: f32, y: f32, w: f32, h: f32, label: &str) -> Self {
        Self {
            bounds: [x, y, w, h],
            label: label.to_string(),
            is_hovered: false,
            is_pressed: false,
            scale_spring: Spring::with_params(1.0, SpringParams::snappy()),
            hover_spring: Spring::with_params(0.0, SpringParams::smooth()),
            shape: Shape::Capsule,
        }
    }

    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.bounds[0] && px <= self.bounds[0] + self.bounds[2] &&
        py >= self.bounds[1] && py <= self.bounds[1] + self.bounds[3]
    }

    pub fn on_pointer_move(&mut self, px: f32, py: f32) {
        let inside = self.contains(px, py);
        if inside != self.is_hovered {
            self.is_hovered = inside;
            self.hover_spring.set_target(if inside { 1.0 } else { 0.0 });
            if !inside {
                self.is_pressed = false;
                self.scale_spring.set_target(1.0);
            }
        }
    }

    pub fn on_pointer_down(&mut self, px: f32, py: f32) -> bool {
        if self.contains(px, py) {
            self.is_pressed = true;
            self.scale_spring.set_target(0.94); // tactile compression
            true
        } else {
            false
        }
    }

    pub fn on_pointer_up(&mut self, px: f32, py: f32) -> bool {
        let was_pressed = self.is_pressed;
        self.is_pressed = false;
        self.scale_spring.set_target(if self.is_hovered { 1.04 } else { 1.0 });
        was_pressed && self.contains(px, py) // Return true on successful click trigger
    }

    pub fn update(&mut self, dt: f32) {
        self.scale_spring.update(dt);
        self.hover_spring.update(dt);
    }

    pub fn to_surface(&self) -> GlassSurface {
        let scale = self.scale_spring.value;
        let hover = self.hover_spring.value;
        let w = self.bounds[2] * scale;
        let h = self.bounds[3] * scale;
        let x = self.bounds[0] + (self.bounds[2] - w) * 0.5;
        let y = self.bounds[1] + (self.bounds[3] - h) * 0.5;

        let alpha = 0.08 + hover * 0.12;

        GlassSurface::new()
            .material(Material::Acrylic)
            .shape(self.shape)
            .bounds(x, y, w, h)
            .blur(16.0 + hover * 8.0)
            .refraction(0.50)
            .fresnel(0.60)
            .specular(0.80)
            .tint(1.0, 1.0, 1.0, alpha)
    }
}

/// An interactive animated toggle switch.
#[derive(Debug, Clone)]
pub struct GlassToggle {
    pub bounds: [f32; 4],
    pub is_on: bool,
    pub knob_spring: Spring,
}

impl GlassToggle {
    pub fn new(x: f32, y: f32, w: f32, h: f32, initial: bool) -> Self {
        let target = if initial { 1.0 } else { 0.0 };
        Self {
            bounds: [x, y, w, h],
            is_on: initial,
            knob_spring: Spring::with_params(target, SpringParams::smooth()),
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.is_on = !self.is_on;
        self.knob_spring.set_target(if self.is_on { 1.0 } else { 0.0 });
        self.is_on
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.bounds[0] && px <= self.bounds[0] + self.bounds[2] &&
        py >= self.bounds[1] && py <= self.bounds[1] + self.bounds[3]
    }

    pub fn on_pointer_down(&mut self, px: f32, py: f32) -> bool {
        if self.contains(px, py) {
            self.toggle();
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.knob_spring.update(dt);
    }

    pub fn to_surfaces(&self) -> Vec<GlassSurface> {
        let t = self.knob_spring.value.clamp(0.0, 1.0);
        
        // Background track (capsule)
        let track_tint = [
            0.2 + t * 0.66, // cyan/gold accent when active
            0.3 + t * 0.67,
            0.4 + t * -0.14,
            0.15 + t * 0.25,
        ];
        let track = GlassSurface::new()
            .material(Material::FrostedGlass)
            .shape(Shape::Capsule)
            .bounds(self.bounds[0], self.bounds[1], self.bounds[2], self.bounds[3])
            .blur(20.0)
            .tint(track_tint[0], track_tint[1], track_tint[2], track_tint[3]);

        // Draggable Knob (circle)
        let knob_size = self.bounds[3] * 0.82;
        let pad = (self.bounds[3] - knob_size) * 0.5;
        let start_x = self.bounds[0] + pad;
        let end_x = self.bounds[0] + self.bounds[2] - knob_size - pad;
        let knob_x = start_x + (end_x - start_x) * t;
        let knob_y = self.bounds[1] + pad;

        let knob = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::Circle)
            .bounds(knob_x, knob_y, knob_size, knob_size)
            .refraction(0.75)
            .specular(1.0)
            .tint(1.0, 1.0, 1.0, 0.40);

        vec![track, knob]
    }
}

/// An interactive horizontal slider / scrubber bar.
#[derive(Debug, Clone)]
pub struct GlassSlider {
    pub bounds: [f32; 4],
    pub value: f32, // normalized 0.0..=1.0
    pub is_dragging: bool,
}

impl GlassSlider {
    pub fn new(x: f32, y: f32, w: f32, h: f32, initial_value: f32) -> Self {
        Self {
            bounds: [x, y, w, h],
            value: initial_value.clamp(0.0, 1.0),
            is_dragging: false,
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        let pad = 8.0;
        px >= self.bounds[0] - pad && px <= self.bounds[0] + self.bounds[2] + pad &&
        py >= self.bounds[1] - pad && py <= self.bounds[1] + self.bounds[3] + pad
    }

    pub fn on_pointer_down(&mut self, px: f32, py: f32) -> bool {
        if self.contains(px, py) {
            self.is_dragging = true;
            self.update_from_pointer(px);
            true
        } else {
            false
        }
    }

    pub fn on_pointer_move(&mut self, px: f32, _py: f32) {
        if self.is_dragging {
            self.update_from_pointer(px);
        }
    }

    pub fn on_pointer_up(&mut self) {
        self.is_dragging = false;
    }

    fn update_from_pointer(&mut self, px: f32) {
        let norm = (px - self.bounds[0]) / self.bounds[2];
        self.value = norm.clamp(0.0, 1.0);
    }

    pub fn to_surfaces(&self) -> Vec<GlassSurface> {
        // Rail track
        let rail = GlassSurface::new()
            .material(Material::FrostedGlass)
            .shape(Shape::Capsule)
            .bounds(self.bounds[0], self.bounds[1], self.bounds[2], self.bounds[3])
            .blur(16.0)
            .tint(1.0, 1.0, 1.0, 0.12);

        // Active fill
        let fill_w = self.bounds[2] * self.value;
        let fill = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::Capsule)
            .bounds(self.bounds[0], self.bounds[1], fill_w.max(self.bounds[3]), self.bounds[3])
            .refraction(0.60)
            .tint(0.38, 0.85, 1.0, 0.35);

        // Thumb knob
        let knob_r = self.bounds[3] * 1.6;
        let knob_x = self.bounds[0] + fill_w - knob_r * 0.5;
        let knob_y = self.bounds[1] + (self.bounds[3] - knob_r) * 0.5;
        let knob = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::Circle)
            .bounds(knob_x, knob_y, knob_r, knob_r)
            .refraction(0.75)
            .specular(1.0)
            .tint(1.0, 1.0, 1.0, 0.50);

        vec![rail, fill, knob]
    }
}

/// A high-level Widget Container holding cards, buttons, sliders, and toggles.
#[derive(Debug, Default)]
pub struct GlassWidget {
    pub cards: Vec<GlassCard>,
    pub buttons: Vec<GlassButton>,
    pub toggles: Vec<GlassToggle>,
    pub sliders: Vec<GlassSlider>,
}

impl GlassWidget {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_card(&mut self, card: GlassCard) {
        self.cards.push(card);
    }

    pub fn add_button(&mut self, button: GlassButton) {
        self.buttons.push(button);
    }

    pub fn add_toggle(&mut self, toggle: GlassToggle) {
        self.toggles.push(toggle);
    }

    pub fn add_slider(&mut self, slider: GlassSlider) {
        self.sliders.push(slider);
    }

    pub fn on_pointer_move(&mut self, px: f32, py: f32) {
        for btn in &mut self.buttons {
            btn.on_pointer_move(px, py);
        }
        for sl in &mut self.sliders {
            sl.on_pointer_move(px, py);
        }
    }

    pub fn on_pointer_down(&mut self, px: f32, py: f32) -> bool {
        let mut handled = false;
        for btn in &mut self.buttons {
            if btn.on_pointer_down(px, py) {
                handled = true;
            }
        }
        for tg in &mut self.toggles {
            if tg.on_pointer_down(px, py) {
                handled = true;
            }
        }
        for sl in &mut self.sliders {
            if sl.on_pointer_down(px, py) {
                handled = true;
            }
        }
        handled
    }

    pub fn on_pointer_up(&mut self, px: f32, py: f32) -> Option<usize> {
        let mut clicked_button = None;
        for (i, btn) in self.buttons.iter_mut().enumerate() {
            if btn.on_pointer_up(px, py) {
                clicked_button = Some(i);
            }
        }
        for sl in &mut self.sliders {
            sl.on_pointer_up();
        }
        clicked_button
    }

    pub fn update(&mut self, dt: f32) {
        for btn in &mut self.buttons {
            btn.update(dt);
        }
        for tg in &mut self.toggles {
            tg.update(dt);
        }
    }

    pub fn collect_surfaces(&self) -> Vec<GlassSurface> {
        let mut surfaces = Vec::new();
        for card in &self.cards {
            surfaces.push(card.to_surface());
        }
        for tg in &self.toggles {
            surfaces.extend(tg.to_surfaces());
        }
        for sl in &self.sliders {
            surfaces.extend(sl.to_surfaces());
        }
        for btn in &self.buttons {
            surfaces.push(btn.to_surface());
        }
        surfaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glass_card_builder() {
        let card = GlassCard::island(0.1, 0.1, 0.8, 0.8)
            .blur(32.0)
            .refraction(0.85);
        let s = card.to_surface();
        assert_eq!(s.bounds, [0.1, 0.1, 0.8, 0.8]);
        assert_eq!(s.bevel, 12.0);
    }

    #[test]
    fn test_glass_button_interaction() {
        let mut btn = GlassButton::new(10.0, 10.0, 100.0, 40.0, "Play");
        assert!(!btn.is_hovered);
        
        btn.on_pointer_move(50.0, 20.0);
        assert!(btn.is_hovered);

        let pressed = btn.on_pointer_down(50.0, 20.0);
        assert!(pressed);
        assert!(btn.is_pressed);

        let clicked = btn.on_pointer_up(50.0, 20.0);
        assert!(clicked);
        assert!(!btn.is_pressed);
    }

    #[test]
    fn test_glass_toggle() {
        let mut tg = GlassToggle::new(10.0, 10.0, 60.0, 30.0, false);
        assert!(!tg.is_on);
        
        let state = tg.toggle();
        assert!(state);
        assert!(tg.is_on);

        let surfaces = tg.to_surfaces();
        assert_eq!(surfaces.len(), 2); // Track + knob
    }

    #[test]
    fn test_glass_slider() {
        let mut sl = GlassSlider::new(0.0, 0.0, 100.0, 10.0, 0.25);
        assert!((sl.value - 0.25).abs() < 0.01);

        sl.on_pointer_down(75.0, 5.0);
        assert!((sl.value - 0.75).abs() < 0.01);

        let surfaces = sl.to_surfaces();
        assert_eq!(surfaces.len(), 3); // Rail + fill + knob
    }

    #[test]
    fn test_widget_collection() {
        let mut widget = GlassWidget::new();
        widget.add_card(GlassCard::new(0.0, 0.0, 1.0, 1.0));
        widget.add_button(GlassButton::new(0.1, 0.1, 0.2, 0.1, "Test"));
        widget.add_toggle(GlassToggle::new(0.4, 0.1, 0.1, 0.05, true));

        let surfaces = widget.collect_surfaces();
        assert_eq!(surfaces.len(), 4); // 1 card + 2 toggle + 1 button
    }
}
