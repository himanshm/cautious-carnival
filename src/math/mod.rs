pub use glam::{Mat3, Vec2, Vec3};

/// Easing functions for smooth animations
pub mod easing {
    pub fn linear(t: f64) -> f64 {
        t
    }

    pub fn ease_in_out(t: f64) -> f64 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        }
    }
}
