use glass_core::animation::{Spring, SpringParams};
use glass_core::geometry::{GlassSurface, Shape};
use glass_core::material::Material;

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
            blur_radius: 16.0,
            refraction: 0.50,
            dispersion: 0.015,
            bevel: 7.0,
            specular: 0.80,
            fresnel: 0.55,
            tint: [1.0, 1.0, 1.0, 0.04],
        }
    }

    pub fn island(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(x, y, w, h)
            .shape(Shape::Capsule)
            .bevel(8.0)
            .specular(0.85)
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
            .bounds(
                self.bounds[0],
                self.bounds[1],
                self.bounds[2],
                self.bounds[3],
            )
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
        px >= self.bounds[0]
            && px <= self.bounds[0] + self.bounds[2]
            && py >= self.bounds[1]
            && py <= self.bounds[1] + self.bounds[3]
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
        self.scale_spring
            .set_target(if self.is_hovered { 1.04 } else { 1.0 });
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
        self.knob_spring
            .set_target(if self.is_on { 1.0 } else { 0.0 });
        self.is_on
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.bounds[0]
            && px <= self.bounds[0] + self.bounds[2]
            && py >= self.bounds[1]
            && py <= self.bounds[1] + self.bounds[3]
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
            .bounds(
                self.bounds[0],
                self.bounds[1],
                self.bounds[2],
                self.bounds[3],
            )
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
        px >= self.bounds[0] - pad
            && px <= self.bounds[0] + self.bounds[2] + pad
            && py >= self.bounds[1] - pad
            && py <= self.bounds[1] + self.bounds[3] + pad
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
            .bounds(
                self.bounds[0],
                self.bounds[1],
                self.bounds[2],
                self.bounds[3],
            )
            .blur(16.0)
            .tint(1.0, 1.0, 1.0, 0.12);

        // Active fill
        let fill_w = self.bounds[2] * self.value;
        let fill = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::Capsule)
            .bounds(
                self.bounds[0],
                self.bounds[1],
                fill_w.max(self.bounds[3]),
                self.bounds[3],
            )
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

/// An interactive liquid glass text input field with animated focus, cursor blink, and text editing.
#[derive(Debug, Clone)]
pub struct GlassTextInput {
    pub bounds: [f32; 4],
    pub text: String,
    pub placeholder: String,
    pub is_focused: bool,
    pub is_hovered: bool,
    pub cursor_pos: usize,
    pub cursor_visible: bool,
    pub cursor_blink_timer: f32,
    pub focus_spring: Spring,
    pub hover_spring: Spring,
    pub shape: Shape,
    pub max_length: Option<usize>,
}

impl GlassTextInput {
    pub fn new(x: f32, y: f32, w: f32, h: f32, placeholder: &str) -> Self {
        Self {
            bounds: [x, y, w, h],
            text: String::new(),
            placeholder: placeholder.to_string(),
            is_focused: false,
            is_hovered: false,
            cursor_pos: 0,
            cursor_visible: true,
            cursor_blink_timer: 0.0,
            focus_spring: Spring::with_params(0.0, SpringParams::smooth()),
            hover_spring: Spring::with_params(0.0, SpringParams::smooth()),
            shape: Shape::RoundedRectangle { radius: 14.0 },
            max_length: None,
        }
    }

    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn with_text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self.cursor_pos = self.text.chars().count();
        self
    }

    pub fn with_max_length(mut self, max_len: usize) -> Self {
        self.max_length = Some(max_len);
        self
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.bounds[0]
            && px <= self.bounds[0] + self.bounds[2]
            && py >= self.bounds[1]
            && py <= self.bounds[1] + self.bounds[3]
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.focus_spring
                .set_target(if focused { 1.0 } else { 0.0 });
            if focused {
                self.cursor_visible = true;
                self.cursor_blink_timer = 0.0;
            }
        }
    }

    pub fn on_pointer_move(&mut self, px: f32, py: f32) {
        let inside = self.contains(px, py);
        if inside != self.is_hovered {
            self.is_hovered = inside;
            self.hover_spring.set_target(if inside { 1.0 } else { 0.0 });
        }
    }

    pub fn on_pointer_down(&mut self, px: f32, py: f32) -> bool {
        if self.contains(px, py) {
            self.set_focused(true);
            let char_width = 9.0;
            let pad_x = 16.0;
            let rel_x = (px - self.bounds[0] - pad_x).max(0.0);
            let target_idx = (rel_x / char_width).round() as usize;
            self.cursor_pos = target_idx.min(self.text.chars().count());
            true
        } else {
            self.set_focused(false);
            false
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(max) = self.max_length {
            if self.text.chars().count() >= max {
                return;
            }
        }
        let byte_pos = self
            .text
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_pos)
            .unwrap_or(self.text.len());
        self.text.insert(byte_pos, c);
        self.cursor_pos += 1;
        self.cursor_visible = true;
        self.cursor_blink_timer = 0.0;
    }

    pub fn insert_text(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn delete_backward(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_pos = self
                .text
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.cursor_pos)
                .unwrap_or(self.text.len());
            self.text.remove(byte_pos);
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;
            true
        } else {
            false
        }
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.cursor_pos < self.text.chars().count() {
            let byte_pos = self
                .text
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.cursor_pos)
                .unwrap_or(self.text.len());
            self.text.remove(byte_pos);
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.text.chars().count() {
            self.cursor_pos += 1;
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;
        }
    }

    pub fn move_cursor_start(&mut self) {
        self.cursor_pos = 0;
        self.cursor_visible = true;
        self.cursor_blink_timer = 0.0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.text.chars().count();
        self.cursor_visible = true;
        self.cursor_blink_timer = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        self.focus_spring.update(dt);
        self.hover_spring.update(dt);

        if self.is_focused {
            self.cursor_blink_timer += dt;
            if self.cursor_blink_timer >= 0.53 {
                self.cursor_visible = !self.cursor_visible;
                self.cursor_blink_timer = 0.0;
            }
        } else {
            self.cursor_visible = false;
        }
    }

    pub fn to_surfaces(&self) -> Vec<GlassSurface> {
        let f = self.focus_spring.value.clamp(0.0, 1.0);
        let h = self.hover_spring.value.clamp(0.0, 1.0);

        // 1. Base input container: elevated blur and accent rim illumination on focus
        let base_tint = [
            0.08 + f * 0.15,
            0.12 + f * 0.25,
            0.18 + f * 0.40,
            0.06 + h * 0.04 + f * 0.10,
        ];

        let base = GlassSurface::new()
            .material(if f > 0.5 {
                Material::LiquidGlass
            } else {
                Material::FrostedGlass
            })
            .shape(self.shape)
            .bounds(
                self.bounds[0],
                self.bounds[1],
                self.bounds[2],
                self.bounds[3],
            )
            .blur(16.0 + f * 12.0)
            .refraction(0.40 + f * 0.25)
            .fresnel(0.50 + f * 0.30)
            .bevel(6.0 + f * 2.0)
            .specular(0.70 + f * 0.25)
            .tint(base_tint[0], base_tint[1], base_tint[2], base_tint[3]);

        let mut surfaces = vec![base];

        // 2. Focused accent meniscus rim / glow if active
        if f > 0.01 {
            let rim = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(self.shape)
                .bounds(
                    self.bounds[0] - 1.0,
                    self.bounds[1] - 1.0,
                    self.bounds[2] + 2.0,
                    self.bounds[3] + 2.0,
                )
                .blur(8.0)
                .refraction(0.70)
                .specular(0.95)
                .tint(0.38, 0.85, 1.0, f * 0.35);
            surfaces.push(rim);
        }

        // 3. Blinking cursor indicator bar when focused and visible
        if self.is_focused && self.cursor_visible {
            let char_width = 9.0;
            let pad_x = 16.0;
            let cursor_x = self.bounds[0] + pad_x + self.cursor_pos as f32 * char_width;
            let cursor_h = self.bounds[3] * 0.55;
            let cursor_y = self.bounds[1] + (self.bounds[3] - cursor_h) * 0.5;

            let cursor = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(cursor_x, cursor_y, 2.0, cursor_h)
                .refraction(0.80)
                .specular(1.0)
                .tint(0.4, 0.9, 1.0, 0.90);
            surfaces.push(cursor);
        }

        surfaces
    }
}

/// A high-level Widget Container holding cards, buttons, sliders, and toggles.
#[derive(Debug, Default)]
pub struct GlassWidget {
    pub cards: Vec<GlassCard>,
    pub buttons: Vec<GlassButton>,
    pub toggles: Vec<GlassToggle>,
    pub sliders: Vec<GlassSlider>,
    pub inputs: Vec<GlassTextInput>,
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

    pub fn add_input(&mut self, input: GlassTextInput) {
        self.inputs.push(input);
    }

    pub fn on_pointer_move(&mut self, px: f32, py: f32) {
        for btn in &mut self.buttons {
            btn.on_pointer_move(px, py);
        }
        for sl in &mut self.sliders {
            sl.on_pointer_move(px, py);
        }
        for input in &mut self.inputs {
            input.on_pointer_move(px, py);
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
        for input in &mut self.inputs {
            if input.on_pointer_down(px, py) {
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
        for input in &mut self.inputs {
            input.update(dt);
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
        for input in &self.inputs {
            surfaces.extend(input.to_surfaces());
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
        assert_eq!(s.bevel, 8.0);
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
    fn test_glass_text_input_builder_and_interaction() {
        let mut input = GlassTextInput::new(10.0, 10.0, 200.0, 40.0, "Search...")
            .with_shape(Shape::Capsule)
            .with_max_length(30);

        assert_eq!(input.placeholder, "Search...");
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_pos, 0);
        assert!(!input.is_focused);

        input.on_pointer_move(50.0, 20.0);
        assert!(input.is_hovered);

        let focused = input.on_pointer_down(50.0, 20.0);
        assert!(focused);
        assert!(input.is_focused);

        input.insert_text("Liquid Glass");
        assert_eq!(input.text, "Liquid Glass");
        assert_eq!(input.cursor_pos, 12);

        input.move_cursor_left();
        input.move_cursor_left();
        assert_eq!(input.cursor_pos, 10);

        input.delete_backward();
        assert_eq!(input.text, "Liquid Glss");

        input.delete_forward();
        assert_eq!(input.text, "Liquid Gls");

        input.move_cursor_start();
        assert_eq!(input.cursor_pos, 0);

        input.move_cursor_end();
        assert_eq!(input.cursor_pos, 10);

        // Test cursor blinking on update
        input.update(0.6);
        assert!(!input.cursor_visible); // toggled off after >0.53s
        input.update(0.6);
        assert!(input.cursor_visible); // toggled back on

        // Focus loss on click outside
        let outside = input.on_pointer_down(500.0, 500.0);
        assert!(!outside);
        assert!(!input.is_focused);
    }

    #[test]
    fn test_widget_collection() {
        let mut widget = GlassWidget::new();
        widget.add_card(GlassCard::new(0.0, 0.0, 1.0, 1.0));
        widget.add_button(GlassButton::new(0.1, 0.1, 0.2, 0.1, "Test"));
        widget.add_toggle(GlassToggle::new(0.4, 0.1, 0.1, 0.05, true));
        widget.add_input(GlassTextInput::new(0.6, 0.1, 0.3, 0.05, "Type..."));

        let surfaces = widget.collect_surfaces();
        // 1 card + 2 toggle + 1 button + 1 input base (unfocused) = 5
        assert_eq!(surfaces.len(), 5);
    }
}
