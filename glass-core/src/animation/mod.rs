/// Spring-based animation system for liquid glass motion
///
/// Implements damped spring physics matching Apple's animation feel:
/// - Natural acceleration and deceleration
/// - Proper settling behavior
/// - Configurable stiffness and damping
///
/// Based on the spring-damper system used in Core Animation

use serde::{Serialize, Deserialize};

/// Spring animation parameters matching Apple's CASpringAnimation
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct SpringParams {
    /// Spring stiffness (200-300 is typical for UI)
    pub stiffness: f32,
    /// Damping ratio (0.7-0.85 is typical for UI)
    /// 1.0 = critically damped, <1.0 = underdamped (bouncy), >1.0 = overdamped (slow)
    pub damping: f32,
    /// Mass of the animated object (0.8-1.2 typical)
    pub mass: f32,
    /// Initial velocity
    pub initial_velocity: f32,
    /// Threshold for considering animation complete
    pub epsilon: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self::default_ui()
    }
}

impl SpringParams {
    /// Default spring for UI elements (smooth, quick settling)
    pub fn default_ui() -> Self {
        Self {
            stiffness: 250.0,
            damping: 0.75,
            mass: 1.0,
            initial_velocity: 0.0,
            epsilon: 0.001,
        }
    }

    /// Bouncy spring for playful interactions
    pub fn bouncy() -> Self {
        Self {
            stiffness: 200.0,
            damping: 0.6,
            mass: 0.8,
            initial_velocity: 0.0,
            epsilon: 0.001,
        }
    }

    /// Smooth spring for gentle transitions
    pub fn smooth() -> Self {
        Self {
            stiffness: 300.0,
            damping: 0.85,
            mass: 1.2,
            initial_velocity: 0.0,
            epsilon: 0.001,
        }
    }

    /// Snappy spring for immediate response
    pub fn snappy() -> Self {
        Self {
            stiffness: 400.0,
            damping: 0.8,
            mass: 0.7,
            initial_velocity: 0.0,
            epsilon: 0.001,
        }
    }

    /// Gentle spring for large motions
    pub fn gentle() -> Self {
        Self {
            stiffness: 180.0,
            damping: 0.9,
            mass: 1.5,
            initial_velocity: 0.0,
            epsilon: 0.001,
        }
    }
}

/// Spring animation state for a single value
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    /// Current value
    pub value: f32,
    /// Target value
    pub target: f32,
    /// Current velocity
    pub velocity: f32,
    /// Spring parameters
    pub params: SpringParams,
    /// Time accumulated
    pub time: f32,
}

impl Spring {
    pub fn new(initial_value: f32) -> Self {
        Self {
            value: initial_value,
            target: initial_value,
            velocity: 0.0,
            params: SpringParams::default(),
            time: 0.0,
        }
    }

    pub fn with_params(initial_value: f32, params: SpringParams) -> Self {
        Self {
            value: initial_value,
            target: initial_value,
            velocity: params.initial_velocity,
            params,
            time: 0.0,
        }
    }

    /// Set new target value
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
        self.time = 0.0;
    }

    /// Set target with initial velocity
    pub fn set_target_with_velocity(&mut self, target: f32, velocity: f32) {
        self.target = target;
        self.velocity = velocity;
        self.time = 0.0;
    }

    /// Update spring simulation
    pub fn update(&mut self, delta_time: f32) {
        if self.is_at_rest() {
            return;
        }

        self.time += delta_time;

        let displacement = self.value - self.target;

        // Spring-damper equation
        // F = -kx - cv
        // where k is stiffness, c is damping coefficient, x is displacement, v is velocity

        let spring_force = -self.params.stiffness * displacement;
        let c = if self.params.damping < 5.0 { 2.0 * self.params.damping * (self.params.stiffness * self.params.mass).sqrt() } else { self.params.damping };
        let damping_force = -c * self.velocity;

        let acceleration = (spring_force + damping_force) / self.params.mass;

        // Semi-implicit Euler integration (more stable than explicit Euler)
        self.velocity += acceleration * delta_time;
        self.value += self.velocity * delta_time;
    }

    /// Check if spring is at rest
    pub fn is_at_rest(&self) -> bool {
        let displacement = (self.value - self.target).abs();
        let velocity = self.velocity.abs();
        displacement < self.params.epsilon && velocity < self.params.epsilon
    }

    /// Snap to target immediately
    pub fn snap_to_target(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
    }
}

/// Multi-dimensional spring for vectors
#[derive(Debug, Clone)]
pub struct SpringVec2 {
    pub x: Spring,
    pub y: Spring,
}

impl SpringVec2 {
    pub fn new(initial: [f32; 2]) -> Self {
        Self {
            x: Spring::new(initial[0]),
            y: Spring::new(initial[1]),
        }
    }

    pub fn with_params(initial: [f32; 2], params: SpringParams) -> Self {
        Self {
            x: Spring::with_params(initial[0], params),
            y: Spring::with_params(initial[1], params),
        }
    }

    pub fn set_target(&mut self, target: [f32; 2]) {
        self.x.set_target(target[0]);
        self.y.set_target(target[1]);
    }

    pub fn update(&mut self, delta_time: f32) {
        self.x.update(delta_time);
        self.y.update(delta_time);
    }

    pub fn value(&self) -> [f32; 2] {
        [self.x.value, self.y.value]
    }

    pub fn is_at_rest(&self) -> bool {
        self.x.is_at_rest() && self.y.is_at_rest()
    }
}

/// Spring for Vec3 (useful for colors, positions)
#[derive(Debug, Clone)]
pub struct SpringVec3 {
    pub x: Spring,
    pub y: Spring,
    pub z: Spring,
}

impl SpringVec3 {
    pub fn new(initial: [f32; 3]) -> Self {
        Self {
            x: Spring::new(initial[0]),
            y: Spring::new(initial[1]),
            z: Spring::new(initial[2]),
        }
    }

    pub fn with_params(initial: [f32; 3], params: SpringParams) -> Self {
        Self {
            x: Spring::with_params(initial[0], params),
            y: Spring::with_params(initial[1], params),
            z: Spring::with_params(initial[2], params),
        }
    }

    pub fn set_target(&mut self, target: [f32; 3]) {
        self.x.set_target(target[0]);
        self.y.set_target(target[1]);
        self.z.set_target(target[2]);
    }

    pub fn update(&mut self, delta_time: f32) {
        self.x.update(delta_time);
        self.y.update(delta_time);
        self.z.update(delta_time);
    }

    pub fn value(&self) -> [f32; 3] {
        [self.x.value, self.y.value, self.z.value]
    }

    pub fn is_at_rest(&self) -> bool {
        self.x.is_at_rest() && self.y.is_at_rest() && self.z.is_at_rest()
    }
}

/// Spring for Vec4 (useful for colors with alpha)
#[derive(Debug, Clone)]
pub struct SpringVec4 {
    pub x: Spring,
    pub y: Spring,
    pub z: Spring,
    pub w: Spring,
}

impl SpringVec4 {
    pub fn new(initial: [f32; 4]) -> Self {
        Self {
            x: Spring::new(initial[0]),
            y: Spring::new(initial[1]),
            z: Spring::new(initial[2]),
            w: Spring::new(initial[3]),
        }
    }

    pub fn with_params(initial: [f32; 4], params: SpringParams) -> Self {
        Self {
            x: Spring::with_params(initial[0], params),
            y: Spring::with_params(initial[1], params),
            z: Spring::with_params(initial[2], params),
            w: Spring::with_params(initial[3], params),
        }
    }

    pub fn set_target(&mut self, target: [f32; 4]) {
        self.x.set_target(target[0]);
        self.y.set_target(target[1]);
        self.z.set_target(target[2]);
        self.w.set_target(target[3]);
    }

    pub fn update(&mut self, delta_time: f32) {
        self.x.update(delta_time);
        self.y.update(delta_time);
        self.z.update(delta_time);
        self.w.update(delta_time);
    }

    pub fn value(&self) -> [f32; 4] {
        [self.x.value, self.y.value, self.z.value, self.w.value]
    }

    pub fn is_at_rest(&self) -> bool {
        self.x.is_at_rest() && self.y.is_at_rest() &&
        self.z.is_at_rest() && self.w.is_at_rest()
    }
}

/// Coordinated animation for glass materials
/// Animates blur, opacity, scale together with proper timing
#[derive(Debug, Clone)]
pub struct GlassTransition {
    pub blur_radius: Spring,
    pub opacity: Spring,
    pub scale: Spring,
}

impl GlassTransition {
    pub fn new() -> Self {
        Self {
            blur_radius: Spring::with_params(0.0, SpringParams::smooth()),
            opacity: Spring::with_params(1.0, SpringParams::smooth()),
            scale: Spring::with_params(1.0, SpringParams::default_ui()),
        }
    }

    /// Start hover animation
    pub fn hover_in(&mut self, base_blur: f32) {
        // Sharpen slightly on hover
        self.blur_radius.set_target(base_blur * 0.9);
        self.opacity.set_target(0.95);
        self.scale.set_target(1.02); // Subtle scale up
    }

    /// End hover animation
    pub fn hover_out(&mut self, base_blur: f32) {
        self.blur_radius.set_target(base_blur);
        self.opacity.set_target(1.0);
        self.scale.set_target(1.0);
    }

    /// Press down animation
    pub fn press(&mut self, base_blur: f32) {
        self.blur_radius.set_target(base_blur * 1.1); // More blur when pressed
        self.opacity.set_target(0.85);
        self.scale.set_target(0.97); // Scale down
    }

    /// Release animation
    pub fn release(&mut self, base_blur: f32) {
        self.blur_radius.set_target(base_blur);
        self.opacity.set_target(1.0);
        self.scale.set_target(1.0);
    }

    pub fn update(&mut self, delta_time: f32) {
        self.blur_radius.update(delta_time);
        self.opacity.update(delta_time);
        self.scale.update(delta_time);
    }

    pub fn is_at_rest(&self) -> bool {
        self.blur_radius.is_at_rest() &&
        self.opacity.is_at_rest() &&
        self.scale.is_at_rest()
    }
}

impl Default for GlassTransition {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_settles() {
        let mut spring = Spring::new(0.0);
        spring.set_target(100.0);

        // Simulate for a few seconds
        for _ in 0..120 {
            spring.update(1.0 / 60.0); // 60 FPS
        }

        assert!(spring.is_at_rest());
        assert!((spring.value - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_spring_vec2() {
        let mut spring = SpringVec2::new([0.0, 0.0]);
        spring.set_target([100.0, 50.0]);

        for _ in 0..120 {
            spring.update(1.0 / 60.0);
        }

        assert!(spring.is_at_rest());
        let value = spring.value();
        assert!((value[0] - 100.0).abs() < 0.01);
        assert!((value[1] - 50.0).abs() < 0.01);
    }
}
