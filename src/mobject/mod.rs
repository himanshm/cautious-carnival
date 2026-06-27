use glam::Vec2;
use tiny_skia::{Color, PathBuilder, Transform};

/// The core trait for all renderable mathematical objects.
pub trait Mobject: Send + Sync {
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn position(&self) -> Vec2;
    fn set_position(&mut self, pos: Vec2);
    fn color(&self) -> Color;
    fn set_color(&mut self, color: Color);

    /// Converts the Mobject into a vector path for the renderer.
    fn build_path(&self, transform: Transform) -> Option<tiny_skia::Path>;

    /// Required to clone trait objects for frame-by-frame animation interpolation.
    fn clone_box(&self) -> Box<dyn Mobject>;
}

// Implement Clone for Box<dyn Mobject>
impl Clone for Box<dyn Mobject> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// --- Concrete Implementations ---

#[derive(Clone)]
pub struct Circle {
    id: String,
    center: Vec2,
    radius: f32,
    color: Color,
}

impl Circle {
    pub fn new(radius: f32) -> Self {
        Self {
            id: String::new(),
            center: Vec2::ZERO,
            radius,
            color: Color::from_rgba8(52, 152, 219, 255), // Default Blue
        }
    }
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Mobject for Circle {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn position(&self) -> Vec2 {
        self.center
    }
    fn set_position(&mut self, pos: Vec2) {
        self.center = pos;
    }
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }

    fn build_path(&self, transform: Transform) -> Option<tiny_skia::Path> {
        let mut pb = PathBuilder::new();
        // Approximate circle using 4 cubic bezier curves (standard KAPPA constant)
        let k = 0.5522847498 * self.radius;
        let r = self.radius;
        let cx = self.center.x;
        let cy = self.center.y;

        pb.move_to(cx, cy - r);
        pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
        pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
        pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
        pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
        pb.close();

        pb.finish()
            .map(|p| p.clone().transform(transform).unwrap_or(p))
    }
}

#[derive(Clone)]
pub struct Square {
    id: String,
    center: Vec2,
    side_length: f32,
    color: Color,
}

impl Square {
    pub fn new(side_length: f32) -> Self {
        Self {
            id: String::new(),
            center: Vec2::ZERO,
            side_length,
            color: Color::from_rgba8(231, 76, 60, 255), // Default Red
        }
    }
}

impl Mobject for Square {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn position(&self) -> Vec2 {
        self.center
    }
    fn set_position(&mut self, pos: Vec2) {
        self.center = pos;
    }
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }

    fn build_path(&self, transform: Transform) -> Option<tiny_skia::Path> {
        let mut pb = PathBuilder::new();
        let half = self.side_length / 2.0;
        let cx = self.center.x;
        let cy = self.center.y;

        pb.move_to(cx - half, cy - half);
        pb.line_to(cx + half, cy - half);
        pb.line_to(cx + half, cy + half);
        pb.line_to(cx - half, cy + half);
        pb.close();

        pb.finish()
            .map(|p| p.clone().transform(transform).unwrap_or(p))
    }
}
