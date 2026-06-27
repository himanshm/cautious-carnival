use crate::mobject::Mobject;
use cosmic_text::{
    Attrs, Buffer, CacheKey, CacheKeyFlags, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use glam::Vec2;
use tiny_skia::{Color, Pixmap, PremultipliedColorU8, Transform};

/// A text mobject rendered via cosmic-text.
/// The text is rasterized once at creation and cached as a Pixmap.
#[derive(Clone)]
pub struct Text {
    id: String,
    position: Vec2,
    color: Color,
    opacity: f32,
    /// Pre-rasterized text bitmap (RGBA, premultiplied).
    raster: Pixmap,
    /// Offset from `position` to the top-left of the raster (in Manim units).
    offset: Vec2,
}

impl Text {
    /// Creates a new Text mobject. `font_size` is in Manim units (1 unit = 100 px).
    pub fn new(content: &str, font_size: f32) -> Self {
        let pixel_size = font_size * 100.0;
        let (raster, width_px, height_px) = Self::rasterize(content, pixel_size, Color::WHITE);

        // Center the text horizontally and vertically around the origin
        let offset = Vec2::new(-(width_px as f32) / 200.0, (height_px as f32) / 200.0);

        Self {
            id: String::new(),
            position: Vec2::ZERO,
            color: Color::WHITE,
            opacity: 1.0,
            raster,
            offset,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Rasterizes text into an RGBA Pixmap. Returns (pixmap, width, height).
    fn rasterize(content: &str, font_size: f32, _default_color: Color) -> (Pixmap, u32, u32) {
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font_size, font_size * 1.3);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        buffer.set_size(&mut font_system, Some(4000.0), Some(2000.0));

        let fs = Attrs::new().family(Family::SansSerif);
        buffer.set_text(&mut font_system, content, &fs, Shaping::Advanced, None);

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
                    x_bin: cosmic_text::SubpixelBin::Zero,
                    y_bin: cosmic_text::SubpixelBin::Zero,
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
            return (Pixmap::new(1, 1).unwrap(), 1, 1);
        }

        let width = (max_x - min_x) as u32;
        let height = (max_y - min_y) as u32;
        let mut pixmap = Pixmap::new(width, height).unwrap();

        for (gx, gy, w, h, data) in glyph_data {
            let ox = gx - min_x;
            let oy = gy - min_y;
            Self::blend_glyph(&mut pixmap, ox, oy, w, h, &data);
        }

        (pixmap, width, height)
    }

    /// Blends a single glyph's alpha mask into the pixmap as white pixels.
    fn blend_glyph(pixmap: &mut Pixmap, ox: i32, oy: i32, w: u32, h: u32, data: &[u8]) {
        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let pixels = pixmap.pixels_mut();

        for y in 0..h as i32 {
            let dst_y = oy + y;
            if dst_y < 0 || dst_y >= ph {
                continue;
            }
            for x in 0..w as i32 {
                let dst_x = ox + x;
                if dst_x < 0 || dst_x >= pw {
                    continue;
                }

                let alpha = data[(y * w as i32 + x) as usize];
                if alpha == 0 {
                    continue;
                }

                let idx = (dst_y * pw + dst_x) as usize;
                let pixel = &mut pixels[idx];

                let sa = alpha as u32;
                let inv_sa = 255 - sa;

                let da = pixel.alpha() as u32;
                let dr = pixel.red() as u32;
                let dg = pixel.green() as u32;
                let db = pixel.blue() as u32;

                // Source-over blend (premultiplied alpha)
                let out_a = sa + (da * inv_sa) / 255;
                let out_r = (255 * sa + dr * inv_sa) / 255;
                let out_g = (255 * sa + dg * inv_sa) / 255;
                let out_b = (255 * sa + db * inv_sa) / 255;

                // FIX: Pass the PREMULIPLIED values directly.
                // from_rgba expects r <= a, g <= a, b <= a.
                *pixel = PremultipliedColorU8::from_rgba(
                    out_r as u8,
                    out_g as u8,
                    out_b as u8,
                    out_a as u8,
                )
                .unwrap();
            }
        }
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
        let top_left = self.position + self.offset;

        let mut screen_pt = tiny_skia::Point::from_xy(top_left.x, top_left.y);
        transform.map_point(&mut screen_pt);

        let dst_x = screen_pt.x as i32;
        let dst_y = screen_pt.y as i32;

        let scale = transform.sx;

        let tinted = Self::tint_pixmap(&self.raster, self.color, self.opacity);

        pixmap.draw_pixmap(
            dst_x,
            dst_y,
            tinted.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::from_scale(scale, scale),
            None,
        );
    }
}

impl Text {
    /// Creates a color-tinted copy of the raster.
    fn tint_pixmap(src: &Pixmap, color: Color, opacity: f32) -> Pixmap {
        let mut dst = Pixmap::new(src.width(), src.height()).unwrap();
        let src_pixels = src.pixels();
        let dst_pixels = dst.pixels_mut();

        let cr = (color.red() * 255.0) as u32;
        let cg = (color.green() * 255.0) as u32;
        let cb = (color.blue() * 255.0) as u32;
        let alpha_mult = (opacity * 255.0) as u32;

        for (s, d) in src_pixels.iter().zip(dst_pixels.iter_mut()) {
            let sa = s.alpha() as u32;
            if sa == 0 {
                *d = PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap();
                continue;
            }

            let orig_a = sa;
            let new_a = (orig_a * alpha_mult) / 255;

            // FIX: We must premultiply the target color by the new alpha
            // before passing it to from_rgba, otherwise r > a and it returns None.
            let new_r = (cr * new_a) / 255;
            let new_g = (cg * new_a) / 255;
            let new_b = (cb * new_a) / 255;

            *d =
                PremultipliedColorU8::from_rgba(new_r as u8, new_g as u8, new_b as u8, new_a as u8)
                    .unwrap();
        }
        dst
    }
}
