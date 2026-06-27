use crate::mobject::Mobject;
use image::RgbaImage;
use tiny_skia::{FillRule, Paint, Pixmap, Transform};

pub struct Renderer {
    pub width: u32,
    pub height: u32,
    background_color: tiny_skia::Color,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            background_color: tiny_skia::Color::from_rgba8(18, 18, 18, 255), // Dark gray (Manim style)
        }
    }

    pub fn render_frame(&self, mobjects: &[Box<dyn Mobject>]) -> RgbaImage {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(self.background_color);

        // Coordinate Transform:
        // 1 unit = 100 pixels. Center of screen is (0,0). Y-axis points UP.
        let scale = 100.0;
        let tx = self.width as f32 / 2.0;
        let ty = self.height as f32 / 2.0;

        // Scale and flip Y-axis, then translate to center
        let transform = Transform::from_row(scale, 0.0, 0.0, -scale, tx, ty);

        let mut paint = Paint::default();
        paint.anti_alias = true;

        for mobj in mobjects {
            if let Some(path) = mobj.build_path(transform) {
                let c = mobj.color();
                paint.set_color_rgba8(
                    (c.red() * 255.0) as u8,
                    (c.green() * 255.0) as u8,
                    (c.blue() * 255.0) as u8,
                    (c.alpha() * 255.0) as u8,
                );
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
            }
        }

        // Convert Pixmap (RGBA8) to image::RgbaImage
        let data = pixmap.data().to_vec();
        image::RgbaImage::from_raw(self.width, self.height, data).unwrap()
    }
}
