use crate::mobject::Mobject;
use cosmic_text::{
    Attrs, Buffer, CacheKey, CacheKeyFlags, Family, FontSystem, Metrics, Shaping, SubpixelBin,
    SwashCache,
};
use glam::Vec2;
use image::RgbaImage;
use kurbo::Affine;
use peniko::{Blob, Color};
use peniko::{Format, Image};
use std::sync::Arc;
use vello::Scene;

#[derive(Clone)]
pub struct Text {
    id: String,
    position: Vec2,
    color: Color,
    opacity: f32,
    raster: Arc<RgbaImage>,
    tinted_raster: Arc<RgbaImage>,
    offset: Vec2,
}

impl Text {
    pub fn new(content: &str, font_size: f32) -> Self {
        let pixel_size = font_size * 100.0;
        let (raster, width_px, height_px) = Self::rasterize(content, pixel_size);
        let raster = Arc::new(raster);

        let offset = Vec2::new(-(width_px as f32) / 200.0, (height_px as f32) / 200.0);

        Self {
            id: String::new(),
            position: Vec2::ZERO,
            color: Color::new([1.0, 1.0, 1.0, 1.0]),
            opacity: 1.0,
            tinted_raster: raster.clone(),
            raster,
            offset,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self.update_tint();
        self
    }

    fn update_tint(&mut self) {
        let components = self.color.components;
        let is_white = components[0] == 1.0 && components[1] == 1.0 && components[2] == 1.0;
        if is_white && self.opacity == 1.0 {
            self.tinted_raster = self.raster.clone();
        } else {
            self.tinted_raster = Arc::new(Self::tint_image(&self.raster, self.color, self.opacity));
        }
    }

    fn rasterize(content: &str, font_size: f32) -> (RgbaImage, u32, u32) {
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font_size, font_size * 1.3);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        buffer.set_size(Some(4000.0), Some(2000.0));

        let attrs = Attrs::new().family(Family::SansSerif);
        // Fix: pass attrs by value (not reference)
        buffer.set_text(content, attrs, Shaping::Advanced);

        let mut min_x: i32 = i32::MAX;
        let mut min_y: i32 = i32::MAX;
        let mut max_x: i32 = i32::MIN;
        let mut max_y: i32 = i32::MIN;

        let mut glyph_data: Vec<(i32, i32, u32, u32, Vec<u8>)> = Vec::new();
        let mut cache = SwashCache::new();

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let cache_key = CacheKey {
                    font_id: glyph.font_id,
                    glyph_id: glyph.glyph_id,
                    font_size_bits: glyph.font_size.to_bits(),
                    x_bin: SubpixelBin::Zero,
                    y_bin: SubpixelBin::Zero,
                    flags: CacheKeyFlags::empty(),
                    font_weight: glyph.font_weight,
                };

                if let Some(image) = cache.get_image_uncached(&mut font_system, cache_key) {
                    let placement = image.placement;
                    let gx = glyph.x + placement.left as f32;
                    let gy = run.line_top + glyph.y - placement.top as f32;

                    let x0 = gx as i32;
                    let y0 = gy as i32;
                    let w = placement.width;
                    let h = placement.height;

                    if w == 0 || h == 0 {
                        continue;
                    }

                    min_x = min_x.min(x0);
                    min_y = min_y.min(y0);
                    max_x = max_x.max(x0 + w as i32);
                    max_y = max_y.max(y0 + h as i32);

                    glyph_data.push((x0, y0, w, h, image.data));
                }
            }
        }

        if min_x == i32::MAX {
            return (RgbaImage::new(1, 1), 1, 1);
        }

        let width = (max_x - min_x) as u32;
        let height = (max_y - min_y) as u32;
        let mut img = RgbaImage::new(width, height);

        for (gx, gy, w, h, data) in glyph_data {
            let ox = gx - min_x;
            let oy = gy - min_y;
            for y in 0..h {
                for x in 0..w {
                    let alpha = data[(y * w + x) as usize];
                    if alpha == 0 {
                        continue;
                    }
                    let px = (ox + x as i32) as u32;
                    let py = (oy + y as i32) as u32;
                    if px < width && py < height {
                        img.put_pixel(px, py, image::Rgba([255, 255, 255, alpha]));
                    }
                }
            }
        }

        (img, width, height)
    }

    fn tint_image(src: &RgbaImage, color: Color, opacity: f32) -> RgbaImage {
        let components = color.components;
        let cr = (components[0] * 255.0) as u8;
        let cg = (components[1] * 255.0) as u8;
        let cb = (components[2] * 255.0) as u8;
        let alpha_mult = opacity;

        let mut dst = RgbaImage::new(src.width(), src.height());
        for (x, y, pixel) in src.enumerate_pixels() {
            let sa = pixel[3] as f32;
            if sa == 0.0 {
                continue;
            }
            let new_a = (sa * alpha_mult) as u8;
            dst.put_pixel(x, y, image::Rgba([cr, cg, cb, new_a]));
        }
        dst
    }
}

impl Mobject for Text {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn position(&self) -> Vec2 {
        self.position
    }
    fn set_position(&mut self, pos: Vec2) {
        self.position = pos;
    }
    fn color(&self) -> Color {
        self.color
    }

    fn set_color(&mut self, color: Color) {
        self.color = color;
        self.update_tint();
    }

    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
        self.update_tint();
    }

    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }

    fn add_to_scene(&self, scene: &mut Scene, transform: Affine) {
        let top_left = self.position + self.offset;

        let screen_pt = transform * kurbo::Point::new(top_left.x as f64, top_left.y as f64);

        let w = self.tinted_raster.width() as f64;
        let h = self.tinted_raster.height() as f64;

        let raw: Arc<[u8]> = self
            .tinted_raster
            .as_raw()
            .clone()
            .into_boxed_slice()
            .into();
        let blob = Blob::new(raw as Arc<dyn AsRef<[u8]> + Send + Sync>);
        let image = Image::new(
            blob,
            Format::Rgba8,
            self.tinted_raster.width(),
            self.tinted_raster.height(),
        );

        let image_transform =
            Affine::translate((screen_pt.x, screen_pt.y)) * Affine::scale_non_uniform(w, h);
        scene.draw_image(&image, image_transform);
    }
}
