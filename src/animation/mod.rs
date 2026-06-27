use glam::Vec2;

pub type MobjectId = usize;

/// Represents a single animation event in the timeline.
pub struct Animation {
    pub target_id: MobjectId,
    pub duration: f64,
    pub kind: AnimationKind,
}

/// The specific mathematical transformation applied over time.
pub enum AnimationKind {
    MoveTo { start: Vec2, end: Vec2 },
    FadeIn { start_alpha: f32, end_alpha: f32 },
    FadeOut { start_alpha: f32, end_alpha: f32 },
}

impl Animation {
    /// Creates a MoveTo animation. The `start` position will be automatically
    /// resolved by the Scene when `play()` is called.
    pub fn move_to(target: MobjectId, end: Vec2, duration: f64) -> Self {
        Self {
            target_id: target,
            duration,
            kind: AnimationKind::MoveTo {
                start: Vec2::ZERO, // Placeholder, resolved by Scene
                end,
            },
        }
    }

    pub fn fade_in(target: MobjectId, duration: f64) -> Self {
        Self {
            target_id: target,
            duration,
            kind: AnimationKind::FadeIn {
                start_alpha: 0.0, // Placeholder
                end_alpha: 1.0,
            },
        }
    }

    pub fn fade_out(target: MobjectId, duration: f64) -> Self {
        Self {
            target_id: target,
            duration,
            kind: AnimationKind::FadeOut {
                start_alpha: 1.0, // Placeholder
                end_alpha: 0.0,
            },
        }
    }
}
