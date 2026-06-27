use crate::mobject::Mobject;
use image::RgbaImage;
use tiny_skia::{Pixmap, Transform};

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
            background_color: tiny_skia::Color::from_rgba8(18, 18, 18, 255),
        }
    }

    pub fn render_frame(&self, mobjects: &[Box<dyn Mobject>]) -> RgbaImage {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(self.background_color);

        let scale = 100.0;
        let tx = self.width as f32 / 2.0;
        let ty = self.height as f32 / 2.0;
        let transform = Transform::from_row(scale, 0.0, 0.0, -scale, tx, ty);

        for mobj in mobjects {
            mobj.render_onto(&mut pixmap, transform);
        }

        let data = pixmap.data().to_vec();
        image::RgbaImage::from_raw(self.width, self.height, data).unwrap()
    }
}
