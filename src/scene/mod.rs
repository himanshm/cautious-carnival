use crate::encoder::VideoEncoder;
use crate::math::easing;
use crate::mobject::Mobject;
use crate::renderer::Renderer;
use anyhow::Result;

pub struct Scene {
    mobjects: Vec<Box<dyn Mobject>>,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
}

impl Scene {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            mobjects: Vec::new(),
            fps,
            width,
            height,
        }
    }

    pub fn add(&mut self, mut mobject: Box<dyn Mobject>) {
        // Auto-assign ID if empty
        if mobject.id().is_empty() {
            mobject.set_id(format!("mobj_{}", self.mobjects.len()));
        }
        self.mobjects.push(mobject);
    }

    /// Renders the scene to an MP4 file.
    /// For this foundation, we implement a simple 3-second horizontal move animation.
    pub fn render_to_file(self, output_path: &str) -> Result<()> {
        let mut encoder = VideoEncoder::new(output_path, self.width, self.height, self.fps)?;
        let renderer = Renderer::new(self.width, self.height);

        let total_duration = 3.0; // 3 seconds
        let total_frames = (total_duration * self.fps as f64) as u32;

        println!("🎬 Rendering {} frames...", total_frames);

        for frame_idx in 0..total_frames {
            let t = frame_idx as f64 / self.fps as f64;
            let progress = (t / total_duration) as f32;

            // Apply easing
            let eased = easing::ease_in_out(progress as f64) as f32;

            // Clone the initial state of all mobjects for this frame
            let mut current_mobjects = self.mobjects.clone();

            // Apply animations (Hardcoded for the demo: move "mobj_0" from left to right)
            for mobj in &mut current_mobjects {
                if mobj.id() == "mobj_0" {
                    let start = glam::Vec2::new(-4.0, 0.0);
                    let end = glam::Vec2::new(4.0, 0.0);
                    mobj.set_position(start.lerp(end, eased));
                }
            }

            // Render and write frame
            let frame = renderer.render_frame(&current_mobjects);
            encoder.write_frame(&frame)?;
        }

        encoder.finish()?;
        println!("✅ Saved to {}", output_path);
        Ok(())
    }
}
