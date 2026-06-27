use glam::Vec2;
use kurbo::Affine;
use peniko::Color;
use vello::Scene;

pub trait Mobject: Send + Sync {
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn position(&self) -> Vec2;
    fn set_position(&mut self, pos: Vec2);
    fn color(&self) -> Color;
    fn set_color(&mut self, color: Color);
    fn opacity(&self) -> f32;
    fn set_opacity(&mut self, opacity: f32);

    fn add_to_scene(&self, scene: &mut Scene, transform: Affine);
    fn clone_box(&self) -> Box<dyn Mobject>;
}

impl Clone for Box<dyn Mobject> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

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
            color: Color::new([0.204, 0.596, 0.859, 1.0]), // Blue
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

    fn add_to_scene(&self, scene: &mut Scene, transform: Affine) {
        let circle = kurbo::Circle::new(
            kurbo::Point::new(self.center.x as f64, self.center.y as f64),
            self.radius as f64,
        );

        // Apply opacity to alpha channel
        let components = self.color.components;
        let brush = Color::new([
            components[0],
            components[1],
            components[2],
            components[3] * self.opacity,
        ]);

        scene.fill(Fill::NonZero, transform, &brush, None, &circle);
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
            color: Color::new([0.906, 0.298, 0.235, 1.0]), // Red
            opacity: 1.0,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
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

    fn add_to_scene(&self, scene: &mut Scene, transform: Affine) {
        let half = self.side_length as f64 / 2.0;
        let cx = self.center.x as f64;
        let cy = self.center.y as f64;
        let rect = kurbo::Rect::new(cx - half, cy - half, cx + half, cy + half);

        let components = self.color.components;
        let brush = Color::new([
            components[0],
            components[1],
            components[2],
            components[3] * self.opacity,
        ]);

        scene.fill(Fill::NonZero, transform, &brush, None, &rect);
    }
}
