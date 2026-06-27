use crate::animation::{Animation, MobjectId};
use crate::encoder::VideoEncoder;
use crate::math::easing;
use crate::mobject::Mobject;
use crate::renderer::GpuRenderer;
use anyhow::Result;

pub struct Scene {
    mobjects: Vec<Box<dyn Mobject>>,
    baseline_mobjects: Option<Vec<Box<dyn Mobject>>>,
    animations: Vec<Animation>,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
}

impl Scene {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            mobjects: Vec::new(),
            baseline_mobjects: None,
            animations: Vec::new(),
            fps,
            width,
            height,
        }
    }

    pub fn add(&mut self, mut mobject: Box<dyn Mobject>) -> MobjectId {
        let id = self.mobjects.len();
        if mobject.id().is_empty() {
            mobject.set_id(id.to_string());
        }
        self.mobjects.push(mobject);
        id
    }

    pub fn play(&mut self, mut anim: Animation) {
        if self.baseline_mobjects.is_none() {
            self.baseline_mobjects = Some(self.mobjects.clone());
        }

        let mobj = &self.mobjects[anim.target_id];

        match &mut anim.kind {
            crate::animation::AnimationKind::MoveTo { start, .. } => {
                *start = mobj.position();
            }
            crate::animation::AnimationKind::FadeIn {
                start_alpha,
                end_alpha,
            } => {
                *start_alpha = mobj.opacity();
                *end_alpha = 1.0;
            }
            crate::animation::AnimationKind::FadeOut {
                start_alpha,
                end_alpha,
            } => {
                *start_alpha = mobj.opacity();
                *end_alpha = 0.0;
            }
        }

        match &anim.kind {
            crate::animation::AnimationKind::MoveTo { end, .. } => {
                self.mobjects[anim.target_id].set_position(*end);
            }
            crate::animation::AnimationKind::FadeIn { end_alpha, .. }
            | crate::animation::AnimationKind::FadeOut { end_alpha, .. } => {
                self.mobjects[anim.target_id].set_opacity(*end_alpha);
            }
        }

        self.animations.push(anim);
    }

    pub fn render_to_file(self, output_path: &str) -> Result<()> {
        let total_duration: f64 = self.animations.iter().map(|a| a.duration).sum();
        let total_frames = (total_duration * self.fps as f64).ceil() as u32;

        let mut encoder = VideoEncoder::new(output_path, self.width, self.height, self.fps)?;
        let mut renderer = GpuRenderer::new(self.width, self.height)?;

        println!(
            "🎬 Rendering {} frames ({:.1}s) on GPU...",
            total_frames, total_duration
        );

        let mut current_state = self
            .baseline_mobjects
            .unwrap_or_else(|| self.mobjects.clone());
        let mut anim_idx = 0;
        let mut anim_time: f64 = 0.0;
        let frame_duration = 1.0 / self.fps as f64;

        for _ in 0..total_frames {
            let mut frame_state = current_state.clone();

            if anim_idx < self.animations.len() {
                let anim = &self.animations[anim_idx];
                let t = (anim_time / anim.duration).clamp(0.0, 1.0);
                let eased = easing::ease_in_out(t);

                let mobj = &mut frame_state[anim.target_id];
                match &anim.kind {
                    crate::animation::AnimationKind::MoveTo { start, end } => {
                        mobj.set_position(start.lerp(end.clone(), eased as f32));
                    }
                    crate::animation::AnimationKind::FadeIn {
                        start_alpha,
                        end_alpha,
                    }
                    | crate::animation::AnimationKind::FadeOut {
                        start_alpha,
                        end_alpha,
                    } => {
                        let alpha = start_alpha + (end_alpha - start_alpha) * eased as f32;
                        mobj.set_opacity(alpha);
                    }
                }

                anim_time += frame_duration;

                if anim_time >= anim.duration {
                    let final_mobj = &frame_state[anim.target_id];
                    current_state[anim.target_id].set_position(final_mobj.position());
                    current_state[anim.target_id].set_opacity(final_mobj.opacity());

                    anim_idx += 1;
                    anim_time = 0.0;
                }
            }

            let frame_data = renderer.render_frame(&frame_state);
            encoder.write_frame(frame_data)?;
        }

        encoder.finish()?;
        println!("✅ Saved to {}", output_path);
        Ok(())
    }
}
