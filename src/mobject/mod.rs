use glam::Vec2;
use tiny_skia::{Color, Pixmap, Transform};

pub trait Mobject: Send + Sync {
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn position(&self) -> Vec2;
    fn set_position(&mut self, pos: Vec2);
    fn color(&self) -> Color;
    fn set_color(&mut self, color: Color);
    fn opacity(&self) -> f32;
    fn set_opacity(&mut self, opacity: f32);

    /// Renders the mobject onto the given pixmap using the provided coordinate transform.
    fn render_onto(&self, pixmap: &mut Pixmap, transform: Transform);

    fn clone_box(&self) -> Box<dyn Mobject>;
}

impl Clone for Box<dyn Mobject> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// --- Circle and Square now implement render_onto ---
// (Move the fill_path logic from renderer/mod.rs into here)

#[derive(Clone)]
pub struct Circle {
    id: String,
    center: Vec2,
    radius: f32,
    color: Color,
    opacity: f32,
}

impl Circle {
    pub fn new(radius: f32) -> Self {
        Self {
            id: String::new(),
            center: Vec2::ZERO,
            radius,
            color: Color::from_rgba8(52, 152, 219, 255),
            opacity: 1.0,
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
    fn opacity(&self) -> f32 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }

    fn render_onto(&self, pixmap: &mut Pixmap, transform: Transform) {
        use tiny_skia::{FillRule, Paint, PathBuilder};
        let mut pb = PathBuilder::new();
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

        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            let c = self.color;
            let final_alpha = (c.alpha() * self.opacity * 255.0) as u8;
            paint.set_color_rgba8(
                (c.red() * 255.0) as u8,
                (c.green() * 255.0) as u8,
                (c.blue() * 255.0) as u8,
                final_alpha,
            );
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
    }
}

#[derive(Clone)]
pub struct Square {
    id: String,
    center: Vec2,
    side_length: f32,
    color: Color,
    opacity: f32,
}

impl Square {
    pub fn new(side_length: f32) -> Self {
        Self {
            id: String::new(),
            center: Vec2::ZERO,
            side_length,
            color: Color::from_rgba8(231, 76, 60, 255),
            opacity: 1.0,
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
    fn opacity(&self) -> f32 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }

    fn render_onto(&self, pixmap: &mut Pixmap, transform: Transform) {
        use tiny_skia::{FillRule, Paint, PathBuilder};
        let mut pb = PathBuilder::new();
        let half = self.side_length / 2.0;
        let cx = self.center.x;
        let cy = self.center.y;

        pb.move_to(cx - half, cy - half);
        pb.line_to(cx + half, cy - half);
        pb.line_to(cx + half, cy + half);
        pb.line_to(cx - half, cy + half);
        pb.close();

        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            let c = self.color;
            let final_alpha = (c.alpha() * self.opacity * 255.0) as u8;
            paint.set_color_rgba8(
                (c.red() * 255.0) as u8,
                (c.green() * 255.0) as u8,
                (c.blue() * 255.0) as u8,
                final_alpha,
            );
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
    }
}
