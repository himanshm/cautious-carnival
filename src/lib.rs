//! # rustimate
//!
//! A programmable mathematical animation engine — a Rust replacement for Manim.
//!
//! The crate is organised around four abstractions:
//!
//! * [`Mobject`] — anything that can be drawn (shapes, text, groups).
//! * [`Animation`] — a time-parameterised transformation of a mobject.
//! * [`Scene`] — the timeline that plays animations and owns the render queue.
//! * [`Renderer`] — the sink that turns a frame into pixels / SVG / video.
//!
//! ## Quick start
//!
//! ```no_run
//! use rustimate::*;
//!
//! let renderer = Box::new(SvgRenderer::new("out.svg", 800, 600));
//! let mut scene = Scene::new(renderer, SceneConfig::default());
//!
//! let circle = Circle::new(1.0).with_color(Color::BLUE);
//! scene.play(FadeIn::new(circle.clone_box(), 1.0));
//! scene.play(MoveTo::new(circle.clone_box(), Vec3::new(2.0, 0.0, 0.0), 1.0));
//! scene.play(FadeOut::new(0.5));
//! ```
//!
//! Everything in this file is self-contained — no other source files in the crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::needless_doctest_main)]

use std::fmt;
use std::time::Duration as StdDuration;

// ---------------------------------------------------------------------------
// Vectors & colour
// ---------------------------------------------------------------------------

/// 3D point / vector in scene units (the Z axis is used only for layering).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// X component (rightward positive).
    pub x: f64,
    /// Y component (upward positive — screen-space Y is flipped at render time).
    pub y: f64,
    /// Z component (forward positive; affects draw order only).
    pub z: f64,
}

impl Vec3 {
    /// Origin point.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    /// Unit vector along +X.
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    /// Unit vector along +Y.
    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    /// Unit vector along +Z.
    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// Construct a new vector.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Construct from a 2D point at z = 0.
    pub const fn xy(x: f64, y: f64) -> Self {
        Self { x, y, z: 0.0 }
    }

    /// Euclidean length.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Dot product.
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Linear interpolation toward `other` by parameter `t` in `[0, 1]`.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

/// Axis-aligned bounding box, in scene units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl BoundingBox {
    /// Construct from min / max corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Center point of the box.
    pub fn center(self) -> Vec3 {
        self.min.lerp(self.max, 0.5)
    }

    /// Width along X.
    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    /// Height along Y.
    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }
}

/// 8-bit RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
    /// Alpha channel, 0–255.
    pub a: u8,
}

impl Color {
    /// Construct from 8-bit RGB (opaque).
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Construct from 8-bit RGBA.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Linear interpolation between two colours.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        let lerp_u8 = |a: u8, b: u8| {
            let v = a as f64 + (b as f64 - a as f64) * t;
            v.round().clamp(0.0, 255.0) as u8
        };
        Self::rgba(
            lerp_u8(self.r, other.r),
            lerp_u8(self.g, other.g),
            lerp_u8(self.b, other.b),
            lerp_u8(self.a, other.a),
        )
    }

    /// CSS hex string, e.g. `#3FA7FF`.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Common palette — Manim-style named colours.
    pub const WHITE: Self = Self::rgb(0xFF, 0xFF, 0xFF);
    pub const BLACK: Self = Self::rgb(0x1C, 0x1C, 0x1C);
    pub const RED: Self = Self::rgb(0xE0, 0x3E, 0x3E);
    pub const GREEN: Self = Self::rgb(0x4F, 0xC3, 0x4F);
    pub const BLUE: Self = Self::rgb(0x3F, 0xA7, 0xFF);
    pub const YELLOW: Self = Self::rgb(0xFF, 0xD1, 0x66);
    pub const PURPLE: Self = Self::rgb(0xB3, 0x8E, 0xFF);
    pub const TEAL: Self = Self::rgb(0x4F, 0xD1, 0xC5);
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "{}", self.to_hex())
        } else {
            write!(
                f,
                "rgba({},{},{},{:.3})",
                self.r,
                self.g,
                self.b,
                self.a as f64 / 255.0
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// A time span measured in seconds.  All animations are driven by `f64`
/// seconds internally so they can be resampled to any output frame rate.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

impl Seconds {
    /// Construct from seconds.
    pub const fn new(s: f64) -> Self {
        Self(s)
    }

    /// Construct from a [`std::time::Duration`].
    pub fn from_std(d: StdDuration) -> Self {
        Self(d.as_secs_f64())
    }

    /// Convert to a [`std::time::Duration`].
    pub fn to_std(self) -> StdDuration {
        StdDuration::from_secs_f64(self.0.max(0.0))
    }

    /// Value in seconds.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl std::ops::Add for Seconds {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Seconds {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

/// Easing functions map normalised time `t ∈ [0, 1]` to a progress value.
///
/// They are pure functions and can be composed freely.
pub type Easing = fn(f64) -> f64;

/// Identity easing — uniform motion.
pub fn linear(t: f64) -> f64 {
    t
}

/// Smooth `S`-curve, equivalent to Manim's `smooth` (cubic Hermite).
pub fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Standard ease-in-out (cubic).
pub fn ease_in_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = 2.0 * t - 2.0;
        1.0 + f * f * f / 2.0
    }
}

/// Ease-out (quadratic).
pub fn ease_out_quad(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease-in (quadratic).
pub fn ease_in_quad(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// There-and-back: maps `[0, 0.5] → [0, 1]` and `[0.5, 1] → [1, 0]`.
pub fn there_and_back(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 2.0 * t } else { 2.0 - 2.0 * t }
}

// ---------------------------------------------------------------------------
// Mobject
// ---------------------------------------------------------------------------

/// A *Mathematical Object* — anything that can be placed in a [`Scene`] and
/// animated.
///
/// Implementors are responsible for storing their own transform state and
/// for knowing how to draw themselves via a [`Renderer`].
pub trait Mobject: std::any::Any + Send {
    /// Current position (anchor point — usually the centroid).
    fn position(&self) -> Vec3;

    /// Set the position.
    fn set_position(&mut self, pos: Vec3);

    /// Current uniform scale factor.
    fn scale(&self) -> f64;

    /// Set the scale factor.
    fn set_scale(&mut self, scale: f64);

    /// Current rotation, in radians, around the Z axis.
    fn rotation(&self) -> f64;

    /// Set the rotation in radians.
    fn set_rotation(&mut self, radians: f64);

    /// Fill / stroke colour.
    fn color(&self) -> Color;

    /// Set the colour.
    fn set_color(&mut self, color: Color);

    /// Opacity in `[0, 1]`.
    fn opacity(&self) -> f64 {
        1.0
    }

    /// Set the opacity.
    fn set_opacity(&mut self, _opacity: f64) {}

    /// Axis-aligned bounding box in scene coordinates.
    fn bbox(&self) -> BoundingBox;

    /// Draw this mobject via the given renderer.
    fn render(&self, renderer: &mut dyn Renderer);

    /// Clone into a boxed, type-erased mobject.
    fn clone_box(&self) -> Box<dyn Mobject>;
}

/// Helper trait — fluent setters for any [`Mobject`].
pub trait MobjectExt: Mobject + Sized {
    /// Move to a new position.
    fn move_to(mut self, pos: Vec3) -> Self {
        self.set_position(pos);
        self
    }

    /// Scale uniformly.
    fn scaled(mut self, factor: f64) -> Self {
        self.set_scale(self.scale() * factor);
        self
    }

    /// Rotate by an angle in radians.
    fn rotated(mut self, radians: f64) -> Self {
        self.set_rotation(self.rotation() + radians);
        self
    }

    /// Change colour.
    fn with_color(mut self, color: Color) -> Self {
        self.set_color(color);
        self
    }

    /// Change opacity.
    fn with_opacity(mut self, opacity: f64) -> Self {
        self.set_opacity(opacity);
        self
    }
}

impl<T: Mobject + Sized> MobjectExt for T {}

// --- Concrete shapes -------------------------------------------------------

/// A circle.
#[derive(Debug, Clone)]
pub struct Circle {
    radius: f64,
    pos: Vec3,
    color: Color,
    opacity: f64,
}

impl Circle {
    /// Construct a circle of the given radius, centred at the origin.
    pub fn new(radius: f64) -> Self {
        Self {
            radius: radius.max(0.0),
            pos: Vec3::ZERO,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// The radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

impl Mobject for Circle {
    fn position(&self) -> Vec3 {
        self.pos
    }
    fn set_position(&mut self, pos: Vec3) {
        self.pos = pos;
    }
    fn scale(&self) -> f64 {
        1.0
    }
    fn set_scale(&mut self, scale: f64) {
        self.radius *= scale.max(0.0);
    }
    fn rotation(&self) -> f64 {
        0.0
    }
    fn set_rotation(&mut self, _: f64) {}
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn opacity(&self) -> f64 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f64) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    fn bbox(&self) -> BoundingBox {
        BoundingBox::new(
            Vec3::new(
                self.pos.x - self.radius,
                self.pos.y - self.radius,
                self.pos.z,
            ),
            Vec3::new(
                self.pos.x + self.radius,
                self.pos.y + self.radius,
                self.pos.z,
            ),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        r.draw_circle(self.pos, self.radius, self.color, self.opacity);
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

/// A square (axis-aligned).
#[derive(Debug, Clone)]
pub struct Square {
    side: f64,
    pos: Vec3,
    color: Color,
    opacity: f64,
}

impl Square {
    /// Construct a square with the given side length.
    pub fn new(side: f64) -> Self {
        Self {
            side: side.max(0.0),
            pos: Vec3::ZERO,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Side length.
    pub fn side(&self) -> f64 {
        self.side
    }
}

impl Mobject for Square {
    fn position(&self) -> Vec3 {
        self.pos
    }
    fn set_position(&mut self, pos: Vec3) {
        self.pos = pos;
    }
    fn scale(&self) -> f64 {
        1.0
    }
    fn set_scale(&mut self, scale: f64) {
        self.side *= scale.max(0.0);
    }
    fn rotation(&self) -> f64 {
        0.0
    }
    fn set_rotation(&mut self, _: f64) {}
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn opacity(&self) -> f64 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f64) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    fn bbox(&self) -> BoundingBox {
        let h = self.side / 2.0;
        BoundingBox::new(
            Vec3::new(self.pos.x - h, self.pos.y - h, self.pos.z),
            Vec3::new(self.pos.x + h, self.pos.y + h, self.pos.z),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        let h = self.side / 2.0;
        r.draw_rect(
            Vec3::new(self.pos.x - h, self.pos.y - h, self.pos.z),
            Vec3::new(self.pos.x + h, self.pos.y + h, self.pos.z),
            self.color,
            self.opacity,
        );
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

/// A straight line segment between two points.
#[derive(Debug, Clone)]
pub struct Line {
    start: Vec3,
    end: Vec3,
    color: Color,
    opacity: f64,
    stroke: f64,
}

impl Line {
    /// Construct a line from `start` to `end`.
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self {
            start,
            end,
            color: Color::WHITE,
            opacity: 1.0,
            stroke: 2.0,
        }
    }

    /// Stroke width in scene units.
    pub fn stroke_width(&self) -> f64 {
        self.stroke
    }

    /// Set the stroke width.
    pub fn set_stroke_width(&mut self, w: f64) {
        self.stroke = w.max(0.0);
    }
}

impl Mobject for Line {
    fn position(&self) -> Vec3 {
        self.start.lerp(self.end, 0.5)
    }
    fn set_position(&mut self, pos: Vec3) {
        let delta = pos - self.position();
        self.start = self.start + delta;
        self.end = self.end + delta;
    }
    fn scale(&self) -> f64 {
        1.0
    }
    fn set_scale(&mut self, _scale: f64) {}
    fn rotation(&self) -> f64 {
        0.0
    }
    fn set_rotation(&mut self, _: f64) {}
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn opacity(&self) -> f64 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f64) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    fn bbox(&self) -> BoundingBox {
        let (min_x, max_x) = (self.start.x.min(self.end.x), self.start.x.max(self.end.x));
        let (min_y, max_y) = (self.start.y.min(self.end.y), self.start.y.max(self.end.y));
        BoundingBox::new(
            Vec3::new(min_x, min_y, self.start.z.min(self.end.z)),
            Vec3::new(max_x, max_y, self.start.z.max(self.end.z)),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        r.draw_line(self.start, self.end, self.color, self.opacity, self.stroke);
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

/// A piece of text.
#[derive(Debug, Clone)]
pub struct Text {
    text: String,
    pos: Vec3,
    font_size: f64,
    color: Color,
    opacity: f64,
}

impl Text {
    /// Construct a text mobject.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            pos: Vec3::ZERO,
            font_size: 24.0,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Font size in scene units.
    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    /// Set the font size.
    pub fn set_font_size(&mut self, size: f64) {
        self.font_size = size.max(1.0);
    }

    /// The text content.
    pub fn content(&self) -> &str {
        &self.text
    }
}

impl Mobject for Text {
    fn position(&self) -> Vec3 {
        self.pos
    }
    fn set_position(&mut self, pos: Vec3) {
        self.pos = pos;
    }
    fn scale(&self) -> f64 {
        1.0
    }
    fn set_scale(&mut self, scale: f64) {
        self.font_size *= scale.max(0.0);
    }
    fn rotation(&self) -> f64 {
        0.0
    }
    fn set_rotation(&mut self, _: f64) {}
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
    fn opacity(&self) -> f64 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f64) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    fn bbox(&self) -> BoundingBox {
        // Crude estimate: 0.6 × font_size per character wide, font_size tall.
        let w = self.font_size * 0.6 * self.text.chars().count() as f64;
        let h = self.font_size;
        BoundingBox::new(
            Vec3::new(self.pos.x - w / 2.0, self.pos.y - h / 2.0, self.pos.z),
            Vec3::new(self.pos.x + w / 2.0, self.pos.y + h / 2.0, self.pos.z),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        r.draw_text(
            &self.text,
            self.pos,
            self.font_size,
            self.color,
            self.opacity,
        );
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

/// A time-parameterised transformation of a mobject.
///
/// Implementors own a *target snapshot* (the mobject as it should appear at
/// `t = 0`) and produce intermediate states by being called with `t ∈ [0, 1]`
/// at each frame.
pub trait Animation: Send {
    /// Total duration of the animation.
    fn duration(&self) -> Seconds;

    /// Called once before the first frame.  Snapshot any state you need.
    fn begin(&mut self, _scene: &mut Scene) {}

    /// Apply the animation at progress `t ∈ [0, 1]` to the scene's mobjects.
    /// The scene has already advanced its internal clock; do not modify the
    /// clock here.
    fn update(&mut self, scene: &mut Scene, t: f64);

    /// Called once after the last frame.  Restore any non-target state.
    fn finish(&mut self, _scene: &mut Scene) {}

    /// Easing curve.  Defaults to [`smooth`].
    fn easing(&self) -> Easing {
        smooth
    }
}

// --- FadeIn / FadeOut ------------------------------------------------------

/// Fade a mobject in from 0 to 1 opacity.
pub struct FadeIn {
    mobject: Box<dyn Mobject>,
    duration: Seconds,
}

impl FadeIn {
    /// Construct a `FadeIn` over `duration` seconds.
    pub fn new(mobject: Box<dyn Mobject>, duration: f64) -> Self {
        Self {
            mobject,
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for FadeIn {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn begin(&mut self, scene: &mut Scene) {
        self.mobject.set_opacity(0.0);
        scene.add(self.mobject.clone_box());
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        let id = scene.last_added_id();
        if let Some(mob) = scene.get_mut(id) {
            mob.set_opacity(t.clamp(0.0, 1.0));
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        let id = scene.last_added_id();
        if let Some(mob) = scene.get_mut(id) {
            mob.set_opacity(1.0);
        }
    }
}

/// Fade a mobject out from 1 to 0 opacity, then remove it from the scene.
pub struct FadeOut {
    target_id: Option<usize>,
    duration: Seconds,
}

impl FadeOut {
    /// Construct a `FadeOut` over `duration` seconds.
    ///
    /// Fades out the *most recently added* mobject.  For finer control use
    /// [`FadeOut::of`] with an explicit id returned from [`Scene::add`].
    pub fn new(_duration: f64) -> Self {
        Self {
            target_id: None,
            duration: Seconds(_duration.max(0.0)),
        }
    }

    /// Fade out the mobject identified by `id`.
    pub fn of(id: usize, duration: f64) -> Self {
        Self {
            target_id: Some(id),
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for FadeOut {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.target_id.unwrap_or_else(|| scene.last_added_id());
        self.target_id = Some(id);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.target_id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_opacity(1.0 - t.clamp(0.0, 1.0));
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.target_id {
            scene.remove(id);
        }
    }
}

// --- MoveTo ----------------------------------------------------------------

/// Animate a mobject from its current position to a target position.
pub struct MoveTo {
    id: Option<usize>,
    target: Vec3,
    start: Vec3,
    duration: Seconds,
    easing_fn: Easing,
}

impl MoveTo {
    /// Construct a `MoveTo` of the most recently added mobject to `target`
    /// over `duration` seconds.
    pub fn new(_mobject: Box<dyn Mobject>, target: Vec3, duration: f64) -> Self {
        Self {
            id: None,
            target,
            start: Vec3::ZERO,
            duration: Seconds(duration.max(0.0)),
            easing_fn: smooth,
        }
    }

    /// Target a specific mobject id.
    pub fn of(id: usize, target: Vec3, duration: f64) -> Self {
        Self {
            id: Some(id),
            target,
            start: Vec3::ZERO,
            duration: Seconds(duration.max(0.0)),
            easing_fn: smooth,
        }
    }

    /// Override the easing function.
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing_fn = easing;
        self
    }
}

impl Animation for MoveTo {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        self.easing_fn
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
        self.start = scene.get(id).map(|m| m.position()).unwrap_or(self.target);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_position(self.start.lerp(self.target, t));
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_position(self.target);
            }
        }
    }
}

// --- Transform -------------------------------------------------------------

/// Morph one mobject into another of the *same type*.  Cross-type morphing is
/// intentionally out of scope for v0.1 — use [`FadeOut`] + [`FadeIn`] for
/// cross-type transitions.
pub struct Transform {
    from_id: Option<usize>,
    to: Box<dyn Mobject>,
    duration: Seconds,
}

impl Transform {
    /// Construct a transform from the most recently added mobject into `to`.
    /// The destination mobject inherits the colour and opacity of the source
    /// at the end of the animation.
    pub fn new(_from: Box<dyn Mobject>, to: Box<dyn Mobject>, duration: f64) -> Self {
        Self {
            from_id: None,
            to,
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for Transform {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn begin(&mut self, scene: &mut Scene) {
        self.from_id = Some(scene.last_added_id());
        // Snapshot the destination position so it animates from the source.
        if let Some(id) = self.from_id {
            if let Some(src) = scene.get(id) {
                self.to.set_position(src.position());
                self.to.set_color(src.color());
                self.to.set_opacity(src.opacity() * 0.0); // start invisible
            }
        }
        scene.add(self.to.clone_box());
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(from_id) = self.from_id {
            // Fade source out, fade destination in.
            if let Some(src) = scene.get_mut(from_id) {
                src.set_opacity(1.0 - t.clamp(0.0, 1.0));
            }
            let dest_id = scene.last_added_id();
            if let Some(dest) = scene.get_mut(dest_id) {
                dest.set_opacity(t.clamp(0.0, 1.0));
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(from_id) = self.from_id {
            scene.remove(from_id);
            let dest_id = scene.last_added_id();
            if let Some(dest) = scene.get_mut(dest_id) {
                dest.set_opacity(1.0);
            }
        }
    }
}

// --- Wait ------------------------------------------------------------------

/// Do nothing for `duration` seconds.  Useful for pacing.
pub struct Wait {
    duration: Seconds,
}

impl Wait {
    /// Construct a `Wait` of the given duration in seconds.
    pub fn new(duration: f64) -> Self {
        Self {
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for Wait {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn update(&mut self, _scene: &mut Scene, _t: f64) {}
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// A sink for frames.  Implementors write to SVG, raster, or video files.
pub trait Renderer: Send {
    /// Begin a new frame at the given absolute scene time.
    fn begin_frame(&mut self, time: Seconds, config: &SceneConfig);

    /// Draw a circle.
    fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64);
    /// Draw an axis-aligned rectangle.
    fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64);
    /// Draw a line segment.
    fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, opacity: f64, stroke: f64);
    /// Draw text.
    fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64);

    /// Finish the current frame.
    fn end_frame(&mut self);

    /// Flush / close the output.  Called once when the scene is dropped.
    fn finish(&mut self);
}

/// Configuration for a scene.
#[derive(Debug, Clone)]
pub struct SceneConfig {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frames per second for time-based renderers.
    pub fps: u32,
    /// Background colour.
    pub background: Color,
    /// Number of scene units that map to the shorter screen dimension.
    /// A value of `8.0` means an 800×600 frame covers approximately
    /// 10.67 × 8.0 scene units.
    pub units_per_short_edge: f64,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 60,
            background: Color::BLACK,
            units_per_short_edge: 8.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

/// An entry in the scene's mobject list.
struct SceneEntry {
    /// Stable, monotonically-increasing id.  Used by animations to reference
    /// specific mobjects across frames.
    id: usize,
    /// The mobject itself.  `None` once removed (we keep the slot so other
    /// ids remain valid).
    mob: Option<Box<dyn Mobject>>,
}

/// The timeline.  Owns the mobject list, the renderer, and the current scene
/// time.  Call [`Scene::play`] to advance the timeline by an animation.
pub struct Scene {
    entries: Vec<SceneEntry>,
    next_id: usize,
    renderer: Box<dyn Renderer>,
    config: SceneConfig,
    time: Seconds,
}

impl Scene {
    /// Construct a new scene with the given renderer and config.
    pub fn new(renderer: Box<dyn Renderer>, config: SceneConfig) -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            renderer,
            config,
            time: Seconds(0.0),
        }
    }

    /// Borrow the scene config.
    pub fn config(&self) -> &SceneConfig {
        &self.config
    }

    /// Current absolute scene time.
    pub fn time(&self) -> Seconds {
        self.time
    }

    /// Add a mobject to the scene and return its id.
    pub fn add(&mut self, mobject: Box<dyn Mobject>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(SceneEntry {
            id,
            mob: Some(mobject),
        });
        id
    }

    /// Remove a mobject by id.  Returns `true` if it was present.
    pub fn remove(&mut self, id: usize) -> bool {
        for entry in &mut self.entries {
            if entry.id == id {
                entry.mob = None;
                return true;
            }
        }
        false
    }

    /// Borrow a mobject by id.
    pub fn get(&self, id: usize) -> Option<&dyn Mobject> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.id == id && e.mob.is_some())
            .and_then(|e| e.mob.as_deref())
    }

    /// Mutably borrow a mobject by id.
    pub fn get_mut(&mut self, id: usize) -> Option<&mut dyn Mobject> {
        self.entries
            .iter_mut()
            .rev()
            .find(|e| e.id == id && e.mob.is_some())
            .and_then(|e| e.mob.as_deref_mut())
    }

    /// The id of the most recently added (and still-present) mobject.
    pub fn last_added_id(&self) -> usize {
        self.entries
            .iter()
            .rev()
            .find(|e| e.mob.is_some())
            .map(|e| e.id)
            .unwrap_or(0)
    }

    /// Play a single animation to completion, sampling at the scene's frame
    /// rate.
    pub fn play(&mut self, mut animation: impl Animation + 'static) {
        let duration = animation.duration();
        let fps = self.config.fps.max(1) as f64;
        let dt = Seconds(1.0 / fps);
        let easing = animation.easing();

        animation.begin(self);

        let mut elapsed = 0.0;
        loop {
            let t = (elapsed / duration.0.max(f64::EPSILON)).clamp(0.0, 1.0);
            let eased = easing(t);
            animation.update(self, eased);
            self.render_frame();
            if t >= 1.0 {
                break;
            }
            elapsed += dt.0;
            self.time = self.time + dt;
        }

        // Make sure we hit exactly t = 1.
        animation.update(self, easing(1.0));
        self.render_frame();
        let remaining = (duration.0 - elapsed.max(0.0)).max(0.0);
        self.time = self.time + Seconds(remaining);

        animation.finish(self);
    }

    /// Render the current state of the scene as a single frame.
    pub fn render_frame(&mut self) {
        self.renderer.begin_frame(self.time, &self.config);
        // Draw in z-order (lower z first).  Stable sort preserves add order.
        let mut order: Vec<(usize, f64)> = self
            .entries
            .iter()
            .filter_map(|e| e.mob.as_ref().map(|m| (e.id, m.position().z)))
            .collect();
        order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (id, _) in order {
            if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
                if let Some(mob) = &entry.mob {
                    mob.render(self.renderer.as_mut());
                }
            }
        }
        self.renderer.end_frame();
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        self.renderer.finish();
    }
}

// ---------------------------------------------------------------------------
// Built-in renderer: SVG (one file per frame, all wrapped in one document)
// ---------------------------------------------------------------------------

/// A renderer that writes a single SVG file containing one `<g>` per frame,
/// layered on top of each other.  This is the simplest possible persistent
/// output and works well for static storyboards.
pub struct SvgRenderer {
    out: std::fs::File,
    width: u32,
    height: u32,
    frame: u32,
    buf: String,
    config: SceneConfig,
}

impl SvgRenderer {
    /// Construct an `SvgRenderer` writing to `path`.
    pub fn new(path: impl AsRef<std::path::Path>, width: u32, height: u32) -> Self {
        let out = std::fs::File::create(path).expect("cannot create svg output");
        Self {
            out,
            width,
            height,
            frame: 0,
            buf: String::new(),
            config: SceneConfig::default(),
        }
    }
}

/// Convert scene coordinates to screen pixel coordinates.
pub fn scene_to_screen(p: Vec3, config: &SceneConfig) -> (f64, f64) {
    let scale = config.units_per_short_edge.max(f64::EPSILON);
    let px_per_unit = config.height.min(config.width) as f64 / scale;
    let cx = config.width as f64 / 2.0 + p.x * px_per_unit;
    let cy = config.height as f64 / 2.0 - p.y * px_per_unit; // Y flip
    (cx, cy)
}

impl Renderer for SvgRenderer {
    fn begin_frame(&mut self, time: Seconds, config: &SceneConfig) {
        self.config = config.clone();
        self.buf.clear();
        self.buf.push_str(&format!(
            "<g id=\"frame{}\" data-t=\"{:.3}\">\n",
            self.frame, time.0
        ));
        // Background rect for this frame.
        let bg = config.background;
        self.buf.push_str(&format!(
            "  <rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\" fill-opacity=\"1.0\"/>\n",
            self.width, self.height, bg
        ));
    }

    fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64) {
        let (cx, cy) = scene_to_screen(center, &self.config);
        let r = radius
            * (self.config.height.min(self.config.width) as f64
                / self.config.units_per_short_edge.max(f64::EPSILON));
        self.buf.push_str(&format!(
            "  <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
            cx, cy, r, color, opacity
        ));
    }

    fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64) {
        let (x1, y1) = scene_to_screen(min, &self.config);
        let (x2, y2) = scene_to_screen(max, &self.config);
        let (x, y) = (x1.min(x2), y1.min(y2));
        let (w, h) = ((x2 - x1).abs(), (y2 - y1).abs());
        self.buf.push_str(&format!(
            "  <rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
            x, y, w, h, color, opacity
        ));
    }

    fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, opacity: f64, stroke: f64) {
        let (x1, y1) = scene_to_screen(start, &self.config);
        let (x2, y2) = scene_to_screen(end, &self.config);
        self.buf.push_str(&format!(
            "  <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>\n",
            x1, y1, x2, y2, color, opacity, stroke
        ));
    }

    fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
        let (cx, cy) = scene_to_screen(pos, &self.config);
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        self.buf.push_str(&format!(
            "  <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"sans-serif\" font-size=\"{:.1}\" fill=\"{}\" fill-opacity=\"{:.3}\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
            cx, cy, size, color, opacity, escaped
        ));
    }

    fn end_frame(&mut self) {
        self.buf.push_str("</g>\n");
        use std::io::Write;
        if self.frame == 0 {
            // Write SVG header on the first frame.
            let header = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
                self.width, self.height, self.width, self.height
            );
            self.out.write_all(header.as_bytes()).ok();
        }
        self.out.write_all(self.buf.as_bytes()).ok();
        self.frame += 1;
    }

    fn finish(&mut self) {
        use std::io::Write;
        self.out.write_all(b"</svg>\n").ok();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec3_lerp_round_trip() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 4.0, 2.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), Vec3::new(5.0, 2.0, 1.0));
    }

    #[test]
    fn color_lerp_endpoints() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        assert_eq!(Color::lerp(a, b, 0.0), a);
        assert_eq!(Color::lerp(a, b, 1.0), b);
        assert_eq!(Color::lerp(a, b, 0.5), Color::rgb(0x8E, 0x8E, 0x8E));
    }

    #[test]
    fn easing_clamps_out_of_range() {
        assert_eq!(linear(-1.0), -1.0); // linear is identity
        assert_eq!(smooth(-1.0), 0.0);
        assert_eq!(smooth(2.0), 1.0);
        assert!(ease_in_out_cubic(-0.5) <= 1e-12);
        assert!((ease_in_out_cubic(2.0) - 1.0).abs() <= 1e-12);
    }

    #[test]
    fn bbox_of_circle() {
        let c = Circle::new(2.0).move_to(Vec3::new(3.0, 0.0, 0.0));
        let bb = c.bbox();
        assert_eq!(bb.min, Vec3::new(1.0, -2.0, 0.0));
        assert_eq!(bb.max, Vec3::new(5.0, 2.0, 0.0));
    }

    #[test]
    fn scene_add_remove_ids() {
        let renderer: Box<dyn Renderer> = Box::new(SvgRenderer::new(
            std::env::temp_dir().join("rustimate_test.svg"),
            100,
            100,
        ));
        let mut scene = Scene::new(renderer, SceneConfig::default());

        let id0 = scene.add(Box::new(Circle::new(1.0)));
        let id1 = scene.add(Box::new(Square::new(1.0)));
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert!(scene.remove(id0));
        assert!(!scene.remove(id0));
        assert!(scene.get(id1).is_some());
    }

    #[test]
    fn wait_animation_advances_time() {
        let renderer: Box<dyn Renderer> = Box::new(SvgRenderer::new(
            std::env::temp_dir().join("rustimate_wait_test.svg"),
            100,
            100,
        ));
        let mut scene = Scene::new(renderer, SceneConfig::default());
        scene.play(Wait::new(1.0));
        assert!((scene.time().0 - 1.0).abs() < 0.05);
    }
}
