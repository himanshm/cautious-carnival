pub mod encoder;
pub mod math;
pub mod mobject;
pub mod renderer;
pub mod scene;

// Re-exports for a clean public API
pub use math::*;
pub use mobject::{Circle, Mobject, Square};
pub use scene::Scene;
