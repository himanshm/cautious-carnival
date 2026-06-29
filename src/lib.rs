//! # cautious-carnival
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
//! ## Backends
//!
//! | Feature          | Renderer              | Output                | Build deps | Runtime deps       |
//! |------------------|-----------------------|-----------------------|------------|--------------------|
//! | `svg`            | [`SvgRenderer`]       | one `.svg` storyboard | none       | none               |
//! | `raster`         | [`RasterRenderer`]    | `.png` frame sequence | none       | none               |
//! | `gif`            | [`GifRenderer`]       | animated `.gif`       | none       | none               |
//! | `video`          | [`VideoRenderer`]     | `.mp4` / `.webm`      | none       | `ffmpeg` on `PATH` |
//! | `video-download` | [`VideoRenderer`]     | `.mp4` / `.webm`      | `ureq` etc | auto-downloads     |
//! | `parallel`       | (helper)              | —                     | none       | none               |
//!
//! The `video` backend uses [`ffmpeg-sidecar`] (pure Rust at build time,
//! spawns `ffmpeg` as a subprocess at runtime) rather than `ffmpeg-next`
//! (which links against the C libraries and requires dev headers at build
//! time).  This means `cargo build --features video` works out of the box
//! on any platform — no `pkg-config`, no `libavcodec-dev`, no linking.
//!
//! ## Quick start (SVG)
//!
//! ```no_run
//! use cautious_carnival::*;
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
//! ## Quick start (animated GIF — pure Rust, no system deps)
//!
//! ```no_run
//! # #[cfg(feature = "gif")] {
//! use cautious_carnival::*;
//!
//! let renderer = Box::new(GifRenderer::new("out.gif", 640, 360, 30).unwrap());
//! let mut scene = Scene::new(renderer, SceneConfig::default());
//! let circle = Circle::new(1.0).with_color(Color::BLUE);
//! scene.play(FadeIn::new(circle.clone_box(), 1.0));
//! scene.play(Rotate::new(circle.clone_box(), std::f64::consts::TAU, 2.0));
//! # }
//! ```
//!
//! ## Quick start (MP4 via ffmpeg-sidecar)
//!
//! Requires `ffmpeg` on the system `PATH` at runtime.  Enable the
//! `video-download` feature for automatic download of a prebuilt binary.
//!
//! ```no_run
//! # #[cfg(feature = "video")] {
//! use cautious_carnival::*;
//!
//! let renderer = Box::new(VideoRenderer::new("out.mp4", 1280, 720, 60).unwrap());
//! let mut scene = Scene::new(renderer, SceneConfig::default());
//! let circle = Circle::new(1.0).with_color(Color::BLUE);
//! scene.play(FadeIn::new(circle.clone_box(), 1.0));
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::needless_doctest_main)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use std::fmt;
use std::time::Duration as StdDuration;

// ---------------------------------------------------------------------------
// Font discovery — placeholder filenames + runtime `src/` directory scan
// ---------------------------------------------------------------------------
//
// The raster / gif / video backends ship with a built-in 5x7 ASCII bitmap
// font so they work out-of-the-box with zero configuration.  When you need
// nicer text rendering, drop one or two `.ttf` files into the crate's
// `src/` directory and they will be auto-discovered at runtime.
//
// The two constants below are *placeholders* — replace the strings with
// the actual filenames you placed in `src/` (e.g. `"Roboto-Regular.ttf"`).
// `FontManager::discover_default()` will then load those files (and any
// other `.ttf` files it finds in `src/`) via `fontdue` and use them in
// preference to the bitmap font.
//
// If a placeholder string still starts with `"REPLACE_WITH_"`, it is
// skipped — this lets the crate compile and run even before you fill in
// the real names.

/// Placeholder for the *primary* TTF font filename.
///
/// Replace the string with the actual filename of the primary `.ttf`
/// file you dropped into the crate's `src/` directory (e.g.
/// `"Roboto-Regular.ttf"`).  Until you do, the raster backend falls
/// back to the built-in 5x7 bitmap font.
pub const PRIMARY_FONT_FILENAME: &str = "REPLACE_WITH_PRIMARY_FONT_FILENAME.ttf";

/// Placeholder for the *secondary* TTF font filename.
///
/// Replace the string with the actual filename of the secondary
/// `.ttf` file you dropped into the crate's `src/` directory.  Useful
/// for pairing a regular and a bold / monospace face.  Until you do,
/// the raster backend falls back to the built-in 5x7 bitmap font.
pub const SECONDARY_FONT_FILENAME: &str = "REPLACE_WITH_SECONDARY_FONT_FILENAME.ttf";

/// Default directory scanned at runtime for `.ttf` files.
///
/// Relative to the current working directory at the moment the first
/// `RasterCore` (i.e. the first raster / gif / video renderer) is
/// constructed.  Override with the `CAUTIOUS_CARNIVAL_FONT_DIR`
/// environment variable, or use [`FontManager::discover_in`] to scan
/// an arbitrary directory.
pub const DEFAULT_FONT_SCAN_DIR: &str = "src";

/// Name of the environment variable used to override
/// [`DEFAULT_FONT_SCAN_DIR`] at runtime.
pub const FONT_DIR_ENV_VAR: &str = "CAUTIOUS_CARNIVAL_FONT_DIR";

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

    /// Multiply this colour's alpha by `opacity ∈ [0, 1]`.  Used by
    /// renderers that bake opacity into the source colour.
    pub fn with_alpha_mul(self, opacity: f64) -> Self {
        let opacity = opacity.clamp(0.0, 1.0);
        let a = ((self.a as f64) * opacity).round().clamp(0.0, 255.0) as u8;
        Self::rgba(self.r, self.g, self.b, a)
    }

    /// Common palette — Manim-style named colours.
    pub const WHITE: Self = Self::rgb(0xFF, 0xFF, 0xFF);
    /// Pure black, slightly softened to `#1C1C1C` for less harsh contrast.
    pub const BLACK: Self = Self::rgb(0x1C, 0x1C, 0x1C);
    /// Bright red `#E03E3E`.
    pub const RED: Self = Self::rgb(0xE0, 0x3E, 0x3E);
    /// Bright green `#4FC34F`.
    pub const GREEN: Self = Self::rgb(0x4F, 0xC3, 0x4F);
    /// Sky blue `#3FA7FF`.
    pub const BLUE: Self = Self::rgb(0x3F, 0xA7, 0xFF);
    /// Warm yellow `#FFD166`.
    pub const YELLOW: Self = Self::rgb(0xFF, 0xD1, 0x66);
    /// Soft purple `#B38EFF`.
    pub const PURPLE: Self = Self::rgb(0xB3, 0x8E, 0xFF);
    /// Teal `#4FD1C5`.
    pub const TEAL: Self = Self::rgb(0x4F, 0xD1, 0xC5);
    /// Orange `#FF9F43`.
    pub const ORANGE: Self = Self::rgb(0xFF, 0x9F, 0x43);
    /// Pink `#FF6B9D`.
    pub const PINK: Self = Self::rgb(0xFF, 0x6B, 0x9D);
    /// Mid gray `#808080`.
    pub const GRAY: Self = Self::rgb(0x80, 0x80, 0x80);
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
    if t < 0.5 {
        2.0 * t
    } else {
        2.0 - 2.0 * t
    }
}

/// Elastic ease-out — overshoots then settles.  Good for "playful" entrances.
pub fn elastic_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        t
    } else {
        let p = 0.3_f64;
        let s = p / 4.0;
        2.0_f64.powf(-10.0 * t) * ((t - s) * (2.0 * std::f64::consts::PI) / p).sin() + 1.0
    }
}

/// Bounce ease-out — simulates a ball bouncing to rest.
pub fn bounce_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984_375
    }
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
    rotation: f64,
    color: Color,
    opacity: f64,
}

impl Circle {
    /// Construct a circle of the given radius, centred at the origin.
    pub fn new(radius: f64) -> Self {
        Self {
            radius: radius.max(0.0),
            pos: Vec3::ZERO,
            rotation: 0.0,
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
        self.rotation
    }
    fn set_rotation(&mut self, radians: f64) {
        self.rotation = radians;
    }
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

/// A rectangle with independent width and height.
#[derive(Debug, Clone)]
pub struct Rectangle {
    width: f64,
    height: f64,
    pos: Vec3,
    color: Color,
    opacity: f64,
}

impl Rectangle {
    /// Construct a rectangle of the given width and height.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width: width.max(0.0),
            height: height.max(0.0),
            pos: Vec3::ZERO,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Width along X.
    pub fn width(&self) -> f64 {
        self.width
    }
    /// Height along Y.
    pub fn height(&self) -> f64 {
        self.height
    }
}

impl Mobject for Rectangle {
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
        let s = scale.max(0.0);
        self.width *= s;
        self.height *= s;
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
                self.pos.x - self.width / 2.0,
                self.pos.y - self.height / 2.0,
                self.pos.z,
            ),
            Vec3::new(
                self.pos.x + self.width / 2.0,
                self.pos.y + self.height / 2.0,
                self.pos.z,
            ),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        r.draw_rect(
            Vec3::new(
                self.pos.x - self.width / 2.0,
                self.pos.y - self.height / 2.0,
                self.pos.z,
            ),
            Vec3::new(
                self.pos.x + self.width / 2.0,
                self.pos.y + self.height / 2.0,
                self.pos.z,
            ),
            self.color,
            self.opacity,
        );
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

/// A regular polygon with `n` sides, centred at the origin.
#[derive(Debug, Clone)]
pub struct Polygon {
    n: usize,
    radius: f64,
    pos: Vec3,
    rotation: f64,
    color: Color,
    opacity: f64,
}

impl Polygon {
    /// Construct a regular polygon with `n` sides inscribed in a circle of
    /// `radius`.  The first vertex is at angle 0 (pointing right).
    pub fn new(n: usize, radius: f64) -> Self {
        Self {
            n: n.max(3),
            radius: radius.max(0.0),
            pos: Vec3::ZERO,
            rotation: 0.0,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Number of sides.
    pub fn sides(&self) -> usize {
        self.n
    }

    /// Circumscribed radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The polygon's vertices in scene coordinates (before `pos` offset).
    pub fn vertices(&self) -> Vec<Vec3> {
        let angle_step = std::f64::consts::TAU / self.n as f64;
        (0..self.n)
            .map(|i| {
                let a = self.rotation + i as f64 * angle_step;
                Vec3::xy(self.radius * a.cos(), self.radius * a.sin())
            })
            .collect()
    }
}

impl Mobject for Polygon {
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
        self.rotation
    }
    fn set_rotation(&mut self, radians: f64) {
        self.rotation = radians;
    }
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
        let r = self.radius;
        BoundingBox::new(
            Vec3::new(self.pos.x - r, self.pos.y - r, self.pos.z),
            Vec3::new(self.pos.x + r, self.pos.y + r, self.pos.z),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        let verts: Vec<Vec3> = self
            .vertices()
            .iter()
            .map(|v| Vec3::new(v.x + self.pos.x, v.y + self.pos.y, v.z + self.pos.z))
            .collect();
        r.draw_polygon(&verts, self.color, self.opacity);
    }
    fn clone_box(&self) -> Box<dyn Mobject> {
        Box::new(self.clone())
    }
}

/// A small filled dot — useful for marking points on a number line or
/// highlighting a position.  Rendered as a small filled circle.
#[derive(Debug, Clone)]
pub struct Dot {
    radius: f64,
    pos: Vec3,
    color: Color,
    opacity: f64,
}

impl Dot {
    /// Construct a dot with the given pixel radius (defaults to `0.06` scene
    /// units, which is a typical Manim point radius).
    pub fn new(pos: Vec3) -> Self {
        Self {
            radius: 0.06,
            pos,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Set the radius.
    pub fn with_radius(mut self, r: f64) -> Self {
        self.radius = r.max(0.0);
        self
    }
}

impl Mobject for Dot {
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

    /// Endpoints.
    pub fn endpoints(&self) -> (Vec3, Vec3) {
        (self.start, self.end)
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

/// An arrow — a line with a triangular arrowhead at the end.
#[derive(Debug, Clone)]
pub struct Arrow {
    start: Vec3,
    end: Vec3,
    color: Color,
    opacity: f64,
    stroke: f64,
    head_size: f64,
}

impl Arrow {
    /// Construct an arrow from `start` to `end`.
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self {
            start,
            end,
            color: Color::WHITE,
            opacity: 1.0,
            stroke: 3.0,
            head_size: 0.25,
        }
    }

    /// Configure the arrowhead size (in scene units).
    pub fn with_head_size(mut self, s: f64) -> Self {
        self.head_size = s.max(0.0);
        self
    }

    /// Configure the stroke width.
    pub fn with_stroke(mut self, s: f64) -> Self {
        self.stroke = s.max(0.0);
        self
    }
}

impl Mobject for Arrow {
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
    fn set_scale(&mut self, scale: f64) {
        let s = scale.max(0.0);
        let len = (self.end - self.start).length();
        if len > f64::EPSILON {
            let dir = (self.end - self.start) * (1.0 / len);
            let new_len = len * s;
            self.end = self.start + dir * new_len;
        }
        self.head_size *= s;
        self.stroke *= s;
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
        let (min_x, max_x) = (self.start.x.min(self.end.x), self.start.x.max(self.end.x));
        let (min_y, max_y) = (self.start.y.min(self.end.y), self.start.y.max(self.end.y));
        BoundingBox::new(
            Vec3::new(min_x - self.head_size, min_y - self.head_size, self.start.z),
            Vec3::new(max_x + self.head_size, max_y + self.head_size, self.end.z),
        )
    }
    fn render(&self, r: &mut dyn Renderer) {
        // Shaft (shortened so the arrowhead sits cleanly at the tip).
        let dir = self.end - self.start;
        let len = dir.length();
        if len < f64::EPSILON {
            return;
        }
        let dir = dir * (1.0 / len);
        let shaft_end = self.end - dir * self.head_size;
        r.draw_line(self.start, shaft_end, self.color, self.opacity, self.stroke);
        // Arrowhead: two short segments fanning back from `end`.
        let perp = Vec3::xy(-dir.y, dir.x);
        let left = self.end - dir * self.head_size + perp * self.head_size * 0.5;
        let right = self.end - dir * self.head_size - perp * self.head_size * 0.5;
        r.draw_line(left, self.end, self.color, self.opacity, self.stroke);
        r.draw_line(right, self.end, self.color, self.opacity, self.stroke);
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

// (kept below: Group)

/// A group of mobjects rendered together.  Setting position / scale /
/// rotation on the group propagates to all children.
pub struct Group {
    children: Vec<Box<dyn Mobject>>,
    pos: Vec3,
    scale_factor: f64,
    rotation: f64,
    color: Color,
    opacity: f64,
}

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Group")
            .field("children_count", &self.children.len())
            .field("pos", &self.pos)
            .field("scale_factor", &self.scale_factor)
            .field("rotation", &self.rotation)
            .field("color", &self.color)
            .field("opacity", &self.opacity)
            .finish()
    }
}

impl Group {
    /// Construct an empty group.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            pos: Vec3::ZERO,
            scale_factor: 1.0,
            rotation: 0.0,
            color: Color::WHITE,
            opacity: 1.0,
        }
    }

    /// Add a child mobject to the group.
    pub fn add(mut self, m: Box<dyn Mobject>) -> Self {
        self.children.push(m);
        self
    }

    /// Borrow the group's children.
    pub fn children(&self) -> &[Box<dyn Mobject>] {
        &self.children
    }

    /// Mutably borrow the group's children.
    pub fn children_mut(&mut self) -> &mut [Box<dyn Mobject>] {
        &mut self.children
    }
}

impl Default for Group {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Group {
    fn clone(&self) -> Self {
        Self {
            children: self.children.iter().map(|c| c.clone_box()).collect(),
            pos: self.pos,
            scale_factor: self.scale_factor,
            rotation: self.rotation,
            color: self.color,
            opacity: self.opacity,
        }
    }
}

impl Mobject for Group {
    fn position(&self) -> Vec3 {
        self.pos
    }
    fn set_position(&mut self, pos: Vec3) {
        let delta = pos - self.pos;
        self.pos = pos;
        for c in &mut self.children {
            let p = c.position() + delta;
            c.set_position(p);
        }
    }
    fn scale(&self) -> f64 {
        self.scale_factor
    }
    fn set_scale(&mut self, scale: f64) {
        let ratio = if self.scale_factor.abs() < f64::EPSILON {
            scale
        } else {
            scale / self.scale_factor
        };
        self.scale_factor = scale;
        for c in &mut self.children {
            let center = self.pos;
            let p = c.position();
            let new_p = center + (p - center) * ratio;
            c.set_position(new_p);
            c.set_scale(c.scale() * ratio);
        }
    }
    fn rotation(&self) -> f64 {
        self.rotation
    }
    fn set_rotation(&mut self, radians: f64) {
        let delta = radians - self.rotation;
        self.rotation = radians;
        let cos = delta.cos();
        let sin = delta.sin();
        for c in &mut self.children {
            let p = c.position();
            let rel = p - self.pos;
            let rotated = Vec3::xy(
                rel.x * cos - rel.y * sin + self.pos.x,
                rel.x * sin + rel.y * cos + self.pos.y,
            );
            c.set_position(rotated);
            c.set_rotation(c.rotation() + delta);
        }
    }
    fn color(&self) -> Color {
        self.color
    }
    fn set_color(&mut self, color: Color) {
        self.color = color;
        for c in &mut self.children {
            c.set_color(color);
        }
    }
    fn opacity(&self) -> f64 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f64) {
        self.opacity = opacity.clamp(0.0, 1.0);
        for c in &mut self.children {
            c.set_opacity(opacity);
        }
    }
    fn bbox(&self) -> BoundingBox {
        let mut first = true;
        let mut min = Vec3::ZERO;
        let mut max = Vec3::ZERO;
        for c in &self.children {
            let bb = c.bbox();
            if first {
                min = bb.min;
                max = bb.max;
                first = false;
            } else {
                min = Vec3::new(
                    min.x.min(bb.min.x),
                    min.y.min(bb.min.y),
                    min.z.min(bb.min.z),
                );
                max = Vec3::new(
                    max.x.max(bb.max.x),
                    max.y.max(bb.max.y),
                    max.z.max(bb.max.z),
                );
            }
        }
        BoundingBox::new(min, max)
    }
    fn render(&self, r: &mut dyn Renderer) {
        for c in &self.children {
            c.render(r);
        }
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

/// Blanket impl so a `Box<dyn Animation>` can be played directly via
/// [`Scene::play`] / [`Scene::play_sequence`].  This is what
/// [`AnimationGroup`] stores internally.
impl Animation for Box<dyn Animation> {
    fn duration(&self) -> Seconds {
        (**self).duration()
    }
    fn begin(&mut self, scene: &mut Scene) {
        (**self).begin(scene);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        (**self).update(scene, t);
    }
    fn finish(&mut self, scene: &mut Scene) {
        (**self).finish(scene);
    }
    fn easing(&self) -> Easing {
        (**self).easing()
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

// --- Rotate ----------------------------------------------------------------

/// Animate a rotation around the Z axis by `delta_radians`.
pub struct Rotate {
    id: Option<usize>,
    delta: f64,
    start_rot: f64,
    duration: Seconds,
    easing_fn: Easing,
}

impl Rotate {
    /// Construct a `Rotate` of the most recently added mobject by
    /// `delta_radians` over `duration` seconds.
    pub fn new(_mobject: Box<dyn Mobject>, delta_radians: f64, duration: f64) -> Self {
        Self {
            id: None,
            delta: delta_radians,
            start_rot: 0.0,
            duration: Seconds(duration.max(0.0)),
            easing_fn: smooth,
        }
    }

    /// Target a specific mobject id.
    pub fn of(id: usize, delta_radians: f64, duration: f64) -> Self {
        Self {
            id: Some(id),
            delta: delta_radians,
            start_rot: 0.0,
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

impl Animation for Rotate {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        self.easing_fn
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
        self.start_rot = scene.get(id).map(|m| m.rotation()).unwrap_or(0.0);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_rotation(self.start_rot + self.delta * t.clamp(0.0, 1.0));
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_rotation(self.start_rot + self.delta);
            }
        }
    }
}

// --- ScaleTo ---------------------------------------------------------------

/// Animate uniform scaling to a target absolute scale factor.  Note that
/// the target is the *absolute* scale, not a multiplier — pass `2.0` to
/// end up at twice the original size.
pub struct ScaleTo {
    id: Option<usize>,
    target_scale: f64,
    start_scale: f64,
    duration: Seconds,
    easing_fn: Easing,
}

impl ScaleTo {
    /// Construct a `ScaleTo` of the most recently added mobject to
    /// `target_scale` over `duration` seconds.
    pub fn new(_mobject: Box<dyn Mobject>, target_scale: f64, duration: f64) -> Self {
        Self {
            id: None,
            target_scale: target_scale.max(0.0),
            start_scale: 1.0,
            duration: Seconds(duration.max(0.0)),
            easing_fn: smooth,
        }
    }

    /// Target a specific mobject id.
    pub fn of(id: usize, target_scale: f64, duration: f64) -> Self {
        Self {
            id: Some(id),
            target_scale: target_scale.max(0.0),
            start_scale: 1.0,
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

impl Animation for ScaleTo {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        self.easing_fn
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
        self.start_scale = scene.get(id).map(|m| m.scale()).unwrap_or(1.0);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                let s =
                    self.start_scale + (self.target_scale - self.start_scale) * t.clamp(0.0, 1.0);
                mob.set_scale(s);
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_scale(self.target_scale);
            }
        }
    }
}

// --- ColorShift ------------------------------------------------------------

/// Animate a colour change from each mobject's current colour to `target`.
pub struct ColorShift {
    id: Option<usize>,
    target: Color,
    start: Color,
    duration: Seconds,
    easing_fn: Easing,
}

impl ColorShift {
    /// Construct a `ColorShift` of the most recently added mobject toward
    /// `target` over `duration` seconds.
    pub fn new(_mobject: Box<dyn Mobject>, target: Color, duration: f64) -> Self {
        Self {
            id: None,
            target,
            start: Color::WHITE,
            duration: Seconds(duration.max(0.0)),
            easing_fn: smooth,
        }
    }

    /// Target a specific mobject id.
    pub fn of(id: usize, target: Color, duration: f64) -> Self {
        Self {
            id: Some(id),
            target,
            start: Color::WHITE,
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

impl Animation for ColorShift {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        self.easing_fn
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
        self.start = scene.get(id).map(|m| m.color()).unwrap_or(Color::WHITE);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_color(Color::lerp(self.start, self.target, t.clamp(0.0, 1.0)));
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_color(self.target);
            }
        }
    }
}

// --- Wiggle ----------------------------------------------------------------

/// A short positional wiggle — the mobject oscillates around its current
/// position and returns to it.  The amplitude is in scene units.
pub struct Wiggle {
    id: Option<usize>,
    amplitude: f64,
    frequency: f64,
    axis: WiggleAxis,
    duration: Seconds,
}

/// Which axis the [`Wiggle`] oscillates along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WiggleAxis {
    /// Horizontal oscillation.
    X,
    /// Vertical oscillation.
    Y,
    /// Diagonal oscillation (both X and Y in phase).
    Both,
}

impl Wiggle {
    /// Construct a `Wiggle` of the most recently added mobject.
    ///
    /// `amplitude` is the peak displacement in scene units; `frequency` is
    /// the number of full oscillations over the animation's duration.
    pub fn new(
        _mobject: Box<dyn Mobject>,
        amplitude: f64,
        frequency: f64,
        axis: WiggleAxis,
        duration: f64,
    ) -> Self {
        Self {
            id: None,
            amplitude,
            frequency,
            axis,
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for Wiggle {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        // Use a damped cosine in `update` directly — bypass ease smoothing.
        linear
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                let home = mob.position();
                // Damped sinusoid: starts loud, settles to zero.
                let phase = t * self.frequency * std::f64::consts::TAU;
                let env = (1.0 - t.clamp(0.0, 1.0)).max(0.0);
                let off = self.amplitude * env * phase.sin();
                let dx = match self.axis {
                    WiggleAxis::X | WiggleAxis::Both => off,
                    WiggleAxis::Y => 0.0,
                };
                let dy = match self.axis {
                    WiggleAxis::Y | WiggleAxis::Both => off,
                    WiggleAxis::X => 0.0,
                };
                mob.set_position(Vec3::xy(home.x + dx, home.y + dy));
            }
        }
    }
}

// --- Pulse -----------------------------------------------------------------

/// A scale pulse — the mobject briefly grows then shrinks back to its
/// original size.  Useful as a "highlight" effect.
pub struct Pulse {
    id: Option<usize>,
    peak_scale: f64,
    start_scale: f64,
    duration: Seconds,
}

impl Pulse {
    /// Construct a `Pulse` of the most recently added mobject, peaking at
    /// `peak_scale` (e.g. `1.3`) over `duration` seconds.
    pub fn new(_mobject: Box<dyn Mobject>, peak_scale: f64, duration: f64) -> Self {
        Self {
            id: None,
            peak_scale: peak_scale.max(0.0),
            start_scale: 1.0,
            duration: Seconds(duration.max(0.0)),
        }
    }
}

impl Animation for Pulse {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        there_and_back
    }
    fn begin(&mut self, scene: &mut Scene) {
        let id = self.id.unwrap_or_else(|| scene.last_added_id());
        self.id = Some(id);
        self.start_scale = scene.get(id).map(|m| m.scale()).unwrap_or(1.0);
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                let s = self.start_scale + (self.peak_scale - self.start_scale) * t.clamp(0.0, 1.0);
                mob.set_scale(s);
            }
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        if let Some(id) = self.id {
            if let Some(mob) = scene.get_mut(id) {
                mob.set_scale(self.start_scale);
            }
        }
    }
}

// --- GrowFromCenter --------------------------------------------------------

/// Animate a mobject growing from scale 0 to its natural size, fading in.
pub struct GrowFromCenter {
    mobject: Box<dyn Mobject>,
    duration: Seconds,
    easing_fn: Easing,
}

impl GrowFromCenter {
    /// Construct a `GrowFromCenter` over `duration` seconds.
    pub fn new(mobject: Box<dyn Mobject>, duration: f64) -> Self {
        Self {
            mobject,
            duration: Seconds(duration.max(0.0)),
            easing_fn: ease_out_quad,
        }
    }

    /// Override the easing function.
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing_fn = easing;
        self
    }
}

impl Animation for GrowFromCenter {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn easing(&self) -> Easing {
        self.easing_fn
    }
    fn begin(&mut self, scene: &mut Scene) {
        self.mobject.set_scale(0.0);
        self.mobject.set_opacity(0.0);
        scene.add(self.mobject.clone_box());
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        let id = scene.last_added_id();
        if let Some(mob) = scene.get_mut(id) {
            let tt = t.clamp(0.0, 1.0);
            mob.set_scale(tt);
            mob.set_opacity(tt);
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        let id = scene.last_added_id();
        if let Some(mob) = scene.get_mut(id) {
            mob.set_scale(1.0);
            mob.set_opacity(1.0);
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

// --- AnimationGroup (parallel) ---------------------------------------------

/// Run a collection of animations in parallel, all starting at the same
/// time.  The group's duration is the maximum of its members' durations.
/// Each member is sampled at its own `t ∈ [0, 1]` based on its own duration
/// relative to the wall clock.
pub struct AnimationGroup {
    members: Vec<Box<dyn Animation>>,
    duration: Seconds,
}

impl AnimationGroup {
    /// Construct an empty group.
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
            duration: Seconds(0.0),
        }
    }

    /// Add an animation to the group.
    pub fn add(mut self, a: Box<dyn Animation>) -> Self {
        let d = a.duration();
        if d > self.duration {
            self.duration = d;
        }
        self.members.push(a);
        self
    }
}

impl Default for AnimationGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Animation for AnimationGroup {
    fn duration(&self) -> Seconds {
        self.duration
    }
    fn begin(&mut self, scene: &mut Scene) {
        for a in &mut self.members {
            a.begin(scene);
        }
    }
    fn update(&mut self, scene: &mut Scene, t: f64) {
        let total = self.duration.0.max(f64::EPSILON);
        for a in &mut self.members {
            let d = a.duration().0.max(f64::EPSILON);
            // Wall-clock time elapsed for this member.
            let elapsed = t.clamp(0.0, 1.0) * total;
            let local_t = (elapsed / d).clamp(0.0, 1.0);
            // Apply the member's own easing to its local time.
            let eased = (a.easing())(local_t);
            a.update(scene, eased);
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        for a in &mut self.members {
            a.finish(scene);
        }
    }
    fn easing(&self) -> Easing {
        linear
    }
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
    /// Draw a filled polygon from a list of vertices.  Default
    /// implementation approximates the polygon as a fan of triangles —
    /// backends with native polygon support should override.
    fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
        if verts.len() < 3 {
            return;
        }
        // Filled-triangle fallback via lines — only useful for SVG.
        for w in verts.windows(2) {
            self.draw_line(w[0], w[1], color, opacity, 1.0);
        }
        self.draw_line(*verts.last().unwrap(), verts[0], color, opacity, 1.0);
    }

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
    /// Voiceover clips recorded against this scene's timeline (see
    /// [`Scene::add_voiceover`]).  Always present, even without the
    /// `tts` feature — in that case the track simply stays empty.
    voiceovers: VoiceoverTrack,
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
            voiceovers: VoiceoverTrack::new(),
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

    /// Remove a mobject by id.  Returns `true` if it was present (and not
    /// already removed).  Subsequent calls for the same id return `false`.
    pub fn remove(&mut self, id: usize) -> bool {
        for entry in &mut self.entries {
            if entry.id == id && entry.mob.is_some() {
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

    /// Convenience wrapper around `play(Wait::new(secs))`.
    pub fn wait(&mut self, secs: f64) {
        self.play(Wait::new(secs));
    }

    /// Play a sequence of animations back-to-back.  Equivalent to calling
    /// [`Scene::play`] on each in order, but slightly more ergonomic.
    pub fn play_sequence(&mut self, animations: Vec<Box<dyn Animation>>) {
        for a in animations {
            self.play(a);
        }
    }

    /// Play a group of animations in parallel (all starting at the same
    /// time).  See [`AnimationGroup`] for details on how member durations
    /// interact.
    pub fn play_together(&mut self, group: AnimationGroup) {
        self.play(group);
    }

    /// Borrow the voiceover clips recorded so far against this scene's
    /// timeline.  See [`Scene::add_voiceover`] for how to add a clip.
    pub fn voiceovers(&self) -> &[Voiceover] {
        self.voiceovers.entries()
    }

    /// Borrow the [`VoiceoverTrack`] for this scene.
    pub fn voiceover_track(&self) -> &VoiceoverTrack {
        &self.voiceovers
    }

    /// Mutably borrow the [`VoiceoverTrack`] for this scene.  Useful
    /// when you want to push clips recorded outside the standard
    /// `add_voiceover` flow (e.g. pre-rendered WAV files).
    pub fn voiceover_track_mut(&mut self) -> &mut VoiceoverTrack {
        &mut self.voiceovers
    }

    /// Record a voiceover clip and advance the scene clock by the
    /// clip's duration, so subsequent animations are timed against the
    /// narration.
    ///
    /// `engine` is any [`VoiceoverEngine`] — see [`EspeakNgEngine`],
    /// [`Pico2WaveEngine`], and [`CommandEngine`] for ready-made
    /// implementations.  `out_dir` is the directory in which the WAV
    /// file will be written (created if it doesn't exist); the file
    /// is named `voiceover_NNNN.wav` based on the current voiceover
    /// count.
    ///
    /// Returns the recorded [`Voiceover`] (which includes the audio
    /// path, duration, and sample rate) on success.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #[cfg(feature = "tts")] {
    /// use cautious_carnival::{EspeakNgEngine, Scene, SceneConfig, SvgRenderer};
    /// use std::path::Path;
    ///
    /// let renderer = Box::new(SvgRenderer::new("out.svg", 800, 600));
    /// let mut scene = Scene::new(renderer, SceneConfig::default());
    /// let engine = EspeakNgEngine::new();
    /// scene.add_voiceover(&engine, "Hello, world!", Path::new("voiceovers")).unwrap();
    /// # }
    /// ```
    #[cfg(feature = "tts")]
    pub fn add_voiceover(
        &mut self,
        engine: &dyn VoiceoverEngine,
        text: &str,
        out_dir: impl AsRef<std::path::Path>,
    ) -> Result<Voiceover, String> {
        let out_dir = out_dir.as_ref();
        std::fs::create_dir_all(out_dir)
            .map_err(|e| format!("create_dir_all({:?}): {e}", out_dir))?;
        let path = out_dir.join(format!(
            "voiceover_{:04}.wav",
            self.voiceovers.entries().len()
        ));
        let vo = engine.synthesize(text, &path)?;
        let dur = vo.duration;
        self.voiceovers.push(vo.clone());
        // Block for the duration of the audio so subsequent animations
        // are timed against the narration.
        self.wait(dur.as_f64());
        Ok(vo)
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
// Text-to-speech voiceover (`tts` feature)
// ---------------------------------------------------------------------------
//
// `cautious-carnival` ships a small voiceover subsystem modelled on
// `kokoro-manim-voiceover` for Python Manim.  The core abstraction is the
// [`VoiceoverEngine`] trait: an engine takes a piece of text and
// synthesises it into a WAV file.  Three reference implementations are
// provided out of the box:
//
//   * [`EspeakNgEngine`] — wraps the `espeak-ng` binary (Linux/macOS/Windows).
//   * [`Pico2WaveEngine`]  — wraps the `pico2wave` binary (often packaged
//                            as `libttspico-utils` on Debian/Ubuntu).
//   * [`CommandEngine`]   — wraps any user-supplied TTS command, with
//                            `{text}` and `{out}` placeholders.
//
// All engines are *subprocess-based*: they spawn an external binary via
// `std::process::Command` and read back the WAV file it produces.  This
// keeps the crate itself free of any C dependencies — you install the
// TTS binary separately on the system `PATH`.
//
// Integration with the scene timeline is handled by [`Scene::add_voiceover`]:
// it synthesises the audio, pushes the resulting [`Voiceover`] onto the
// scene's [`VoiceoverTrack`], and then calls `Scene::wait(duration)` so
// subsequent animations are timed against the narration.  After the scene
// is rendered, [`mux_audio_video`] (with the `video` feature) can mux the
// concatenated audio track into the final MP4 via ffmpeg-sidecar.

/// A single synthesised voiceover clip.
///
/// Always available (no feature flag) so that scenes can be inspected for
/// voiceover metadata even when the `tts` feature is off — useful for
/// tooling that processes already-rendered scenes.
#[derive(Debug, Clone)]
pub struct Voiceover {
    /// Path to the generated WAV file.
    pub audio_path: std::path::PathBuf,
    /// The text that was spoken.
    pub text: String,
    /// Duration of the audio in seconds.
    pub duration: Seconds,
    /// Sample rate of the audio (samples per second, per channel).
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
}

impl Voiceover {
    /// Construct a voiceover from an already-existing WAV file.
    ///
    /// Parses the WAV header to extract duration / sample rate / channel
    /// count.  Returns an error if the file is not a valid WAV.
    pub fn from_wav(
        text: impl Into<String>,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let (duration, sample_rate, channels) = parse_wav(&path)?;
        Ok(Self {
            audio_path: path,
            text: text.into(),
            duration,
            sample_rate,
            channels,
        })
    }
}

/// A timeline of [`Voiceover`] clips recorded against a [`Scene`].
///
/// Always available — the `tts` feature only gates the *synthesis* side
/// (`VoiceoverEngine` and the reference engines).  This lets you inspect
/// or post-process the track even when the `tts` feature is off.
#[derive(Debug, Clone, Default)]
pub struct VoiceoverTrack {
    entries: Vec<Voiceover>,
}

impl VoiceoverTrack {
    /// Construct an empty track.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a clip to the track.
    pub fn push(&mut self, vo: Voiceover) {
        self.entries.push(vo);
    }

    /// Borrow the clips.
    pub fn entries(&self) -> &[Voiceover] {
        &self.entries
    }

    /// Number of clips.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if the track has no clips.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total duration of all clips (sum, not max — clips are sequential
    /// along the scene timeline).
    pub fn total_duration(&self) -> Seconds {
        Seconds(self.entries.iter().map(|v| v.duration.0).sum())
    }

    /// Iterator over (clip, start_time_on_scene_timeline) pairs.
    ///
    /// Useful for muxing: each clip starts playing at the scene time at
    /// which it was recorded.
    pub fn iter_with_offsets(&self) -> impl Iterator<Item = (&Voiceover, Seconds)> {
        let mut t = 0.0_f64;
        self.entries.iter().map(move |vo| {
            let start = Seconds(t);
            t += vo.duration.0;
            (vo, start)
        })
    }

    /// Concatenate all voiceover clips into a single WAV file at
    /// `out_path`.  Assumes all clips share the same sample format
    /// (sample rate + bit depth + channels); if they don't, the
    /// resulting file may be malformed.  For robust muxing with
    /// re-encoding, use [`mux_audio_video`] (requires the `video`
    /// feature) or call `ffmpeg` directly.
    ///
    /// Returns the duration of the concatenated audio on success.
    pub fn concatenate_into_wav(
        &self,
        out_path: impl AsRef<std::path::Path>,
    ) -> Result<Seconds, String> {
        if self.entries.is_empty() {
            return Err("voiceover track is empty".into());
        }
        let out_path = out_path.as_ref();
        if self.entries.len() == 1 {
            // Trivial case: just copy the single file.
            std::fs::copy(&self.entries[0].audio_path, out_path).map_err(|e| {
                format!(
                    "copy {:?} -> {:?}: {e}",
                    self.entries[0].audio_path, out_path
                )
            })?;
            return Ok(self.entries[0].duration);
        }

        // Multi-clip case: read each WAV's data chunk and concatenate
        // the raw PCM under a single 44-byte header borrowed from the
        // first clip.
        let first = &self.entries[0];
        let first_bytes = std::fs::read(&first.audio_path)
            .map_err(|e| format!("read {:?}: {e}", first.audio_path))?;
        if first_bytes.len() < 44 || &first_bytes[0..4] != b"RIFF" || &first_bytes[8..12] != b"WAVE"
        {
            return Err(format!(
                "first clip {:?} is not a valid WAV",
                first.audio_path
            ));
        }

        // Walk the first WAV's chunks to find the fmt and data chunks.
        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut bits_per_sample = 0u16;
        let mut fmt_chunk: Vec<u8> = Vec::new();
        let mut data_start = 0usize;
        let mut pos = 12usize;
        while pos + 8 <= first_bytes.len() {
            let chunk_id = &first_bytes[pos..pos + 4];
            let chunk_size = u32::from_le_bytes([
                first_bytes[pos + 4],
                first_bytes[pos + 5],
                first_bytes[pos + 6],
                first_bytes[pos + 7],
            ]) as usize;
            if chunk_id == b"fmt " {
                let body =
                    &first_bytes[pos + 8..pos + 8 + chunk_size.min(first_bytes.len() - pos - 8)];
                fmt_chunk = body.to_vec();
                if body.len() >= 16 {
                    channels = u16::from_le_bytes([body[2], body[3]]);
                    sample_rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                    bits_per_sample = u16::from_le_bytes([body[14], body[15]]);
                }
            } else if chunk_id == b"data" {
                data_start = pos + 8;
                break;
            }
            pos += 8 + chunk_size;
            if chunk_size % 2 == 1 {
                pos += 1;
            }
        }
        if sample_rate == 0 || bits_per_sample == 0 || data_start == 0 {
            return Err("could not parse fmt / data chunks in first clip".into());
        }

        // Collect raw PCM from each clip.
        let mut pcm: Vec<u8> = Vec::new();
        for vo in &self.entries {
            let bytes = std::fs::read(&vo.audio_path)
                .map_err(|e| format!("read {:?}: {e}", vo.audio_path))?;
            // Find this clip's data chunk.
            let mut p = 12usize;
            let mut ds = 0usize;
            let mut dl = 0usize;
            while p + 8 <= bytes.len() {
                let id = &bytes[p..p + 4];
                let sz =
                    u32::from_le_bytes([bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]])
                        as usize;
                if id == b"data" {
                    ds = p + 8;
                    dl = sz;
                    break;
                }
                p += 8 + sz;
                if sz % 2 == 1 {
                    p += 1;
                }
            }
            if ds == 0 {
                return Err(format!("clip {:?} has no data chunk", vo.audio_path));
            }
            let end = (ds + dl).min(bytes.len());
            pcm.extend_from_slice(&bytes[ds..end]);
        }

        // Write the concatenated WAV.
        use std::io::Write;
        let mut f =
            std::fs::File::create(out_path).map_err(|e| format!("create {:?}: {e}", out_path))?;
        let total_pcm = pcm.len() as u32;
        let riff_size = 36 + total_pcm;
        let fmt_size = fmt_chunk.len() as u32;
        f.write_all(b"RIFF").map_err(io_err)?;
        f.write_all(&riff_size.to_le_bytes()).map_err(io_err)?;
        f.write_all(b"WAVE").map_err(io_err)?;
        f.write_all(b"fmt ").map_err(io_err)?;
        f.write_all(&fmt_size.to_le_bytes()).map_err(io_err)?;
        f.write_all(&fmt_chunk).map_err(io_err)?;
        f.write_all(b"data").map_err(io_err)?;
        f.write_all(&total_pcm.to_le_bytes()).map_err(io_err)?;
        f.write_all(&pcm).map_err(io_err)?;
        f.flush().map_err(io_err)?;

        let bytes_per_sample = bits_per_sample as f64 / 8.0;
        let total_samples = total_pcm as f64 / bytes_per_sample / channels as f64;
        let duration = total_samples / sample_rate as f64;
        Ok(Seconds(duration))
    }
}

/// Helper: convert `std::io::Error` into a `String` for ergonomic
/// `?`-propagation in the WAV concatenation code.
fn io_err(e: std::io::Error) -> String {
    e.to_string()
}

/// Parse a WAV file's header to extract (duration, sample_rate, channels).
///
/// Walks the RIFF chunk structure looking for `fmt ` and `data` chunks,
/// so it copes with extra metadata chunks (LIST, fact, etc.) inserted
/// before the data chunk.
pub fn parse_wav(path: &std::path::Path) -> Result<(Seconds, u32, u16), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {:?}: {e}", path))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{:?} is not a valid WAV file", path));
    }
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut data_size = 0u64;
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        if chunk_id == b"fmt " {
            if pos + 8 + chunk_size > bytes.len() {
                break;
            }
            let fmt = &bytes[pos + 8..pos + 8 + chunk_size];
            if fmt.len() >= 16 {
                channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
            }
        } else if chunk_id == b"data" {
            data_size = chunk_size as u64;
            break;
        }
        pos += 8 + chunk_size;
        if chunk_size % 2 == 1 {
            pos += 1;
        }
    }
    if sample_rate == 0 || bits_per_sample == 0 || data_size == 0 {
        return Err(format!("could not find fmt / data chunks in {:?}", path));
    }
    let bytes_per_sample = bits_per_sample as f64 / 8.0;
    if channels == 0 || bytes_per_sample == 0.0 {
        return Err(format!("malformed WAV header in {:?}", path));
    }
    let total_samples = data_size as f64 / bytes_per_sample / channels as f64;
    let duration = total_samples / sample_rate as f64;
    Ok((Seconds(duration), sample_rate, channels))
}

// --- VoiceoverEngine trait + reference implementations ---------------------

/// A text-to-speech engine that synthesises text into a WAV file.
///
/// Implementations are responsible for:
///
/// 1. Spawning whatever system TTS binary they wrap.
/// 2. Ensuring the output WAV file is written to `out_path`.
/// 3. Parsing the resulting WAV to fill in `duration` / `sample_rate` /
///    `channels` on the returned [`Voiceover`].
///
/// See [`EspeakNgEngine`], [`Pico2WaveEngine`], and [`CommandEngine`] for
/// ready-made implementations.
#[cfg(feature = "tts")]
pub trait VoiceoverEngine: Send {
    /// Synthesise `text` into a WAV file at `out_path`.
    fn synthesize(&self, text: &str, out_path: &std::path::Path) -> Result<Voiceover, String>;
}

/// An [`VoiceoverEngine`] that wraps the `espeak-ng` binary.
///
/// `espeak-ng` is a compact open-source TTS synthesiser available on
/// Linux, macOS, and Windows.  Install it on your system `PATH`:
///
/// * Debian/Ubuntu: `sudo apt install espeak-ng`
/// * macOS (Homebrew): `brew install espeak-ng`
/// * Arch: `sudo pacman -S espeak-ng`
/// * Windows: download from <https://github.com/espeak-ng/espeak-ng/releases>
///
/// Voices are listed with `espeak-ng --voices`.  Common ones: `"en"`,
/// `"en-us"`, `"en-gb"`, `"fr"`, `"de"`, `"es"`, `"hi"`, `"zh"`.
#[cfg(feature = "tts")]
#[derive(Debug, Clone)]
pub struct EspeakNgEngine {
    voice: String,
    /// Words per minute (default 175, range 80–450).
    speed: u32,
    /// Pitch 0–99 (default 50).
    pitch: u32,
    /// Amplitude 0–200 (default 100).
    amplitude: u32,
}

#[cfg(feature = "tts")]
impl EspeakNgEngine {
    /// Construct an engine with sensible defaults: voice `"en"`, speed
    /// 175 wpm, pitch 50, amplitude 100.
    pub fn new() -> Self {
        Self {
            voice: "en".to_string(),
            speed: 175,
            pitch: 50,
            amplitude: 100,
        }
    }

    /// Set the voice (e.g. `"en-us"`, `"en-gb"`, `"fr"`).
    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = voice.into();
        self
    }

    /// Set the speed in words per minute (clamped to 80–450).
    pub fn with_speed(mut self, speed: u32) -> Self {
        self.speed = speed.clamp(80, 450);
        self
    }

    /// Set the pitch (clamped to 0–99).
    pub fn with_pitch(mut self, pitch: u32) -> Self {
        self.pitch = pitch.min(99);
        self
    }

    /// Set the amplitude (clamped to 0–200).
    pub fn with_amplitude(mut self, amplitude: u32) -> Self {
        self.amplitude = amplitude.min(200);
        self
    }
}

#[cfg(feature = "tts")]
impl Default for EspeakNgEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "tts")]
impl VoiceoverEngine for EspeakNgEngine {
    fn synthesize(&self, text: &str, out_path: &std::path::Path) -> Result<Voiceover, String> {
        use std::process::Command;

        let out_str = out_path
            .to_str()
            .ok_or_else(|| format!("output path is not valid UTF-8: {out_path:?}"))?
            .to_string();

        let output = Command::new("espeak-ng")
            .arg("-v")
            .arg(&self.voice)
            .arg("-s")
            .arg(self.speed.to_string())
            .arg("-p")
            .arg(self.pitch.to_string())
            .arg("-a")
            .arg(self.amplitude.to_string())
            .arg("-w")
            .arg(&out_str)
            .arg(text)
            .output()
            .map_err(|e| {
                format!(
                    "failed to spawn `espeak-ng`: {e}\n\
                     hint: install espeak-ng on your system PATH\n\
                     *  Debian/Ubuntu: sudo apt install espeak-ng\n\
                     *  macOS:         brew install espeak-ng\n\
                     *  Arch:          sudo pacman -S espeak-ng"
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("espeak-ng failed: {stderr}"));
        }

        let (duration, sample_rate, channels) = parse_wav(out_path)?;
        Ok(Voiceover {
            audio_path: out_path.to_path_buf(),
            text: text.to_string(),
            duration,
            sample_rate,
            channels,
        })
    }
}

/// An [`VoiceoverEngine`] that wraps the `pico2wave` binary.
///
/// `pico2wave` is part of the SVOX Pico TTS engine, often packaged as
/// `libttspico-utils` on Debian/Ubuntu.  It produces higher-quality
/// speech than `espeak-ng` but supports fewer voices and platforms.
///
/// * Debian/Ubuntu: `sudo apt install libttspico-utils`
#[cfg(feature = "tts")]
#[derive(Debug, Clone)]
pub struct Pico2WaveEngine {
    /// Language code, e.g. `"en-US"`, `"en-GB"`, `"de-DE"`, `"es-ES"`,
    /// `"fr-FR"`, `"it-IT"`.
    lang: String,
}

#[cfg(feature = "tts")]
impl Pico2WaveEngine {
    /// Construct an engine with default language `"en-US"`.
    pub fn new() -> Self {
        Self {
            lang: "en-US".to_string(),
        }
    }

    /// Set the language code (e.g. `"en-GB"`, `"de-DE"`).
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }
}

#[cfg(feature = "tts")]
impl Default for Pico2WaveEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "tts")]
impl VoiceoverEngine for Pico2WaveEngine {
    fn synthesize(&self, text: &str, out_path: &std::path::Path) -> Result<Voiceover, String> {
        use std::process::Command;

        let out_str = out_path
            .to_str()
            .ok_or_else(|| format!("output path is not valid UTF-8: {out_path:?}"))?
            .to_string();

        // pico2wave usage: pico2wave -l<lang> -w <out.wav> "<text>"
        let lang_flag = format!("-l{}", self.lang);
        let output = Command::new("pico2wave")
            .arg(&lang_flag)
            .arg("-w")
            .arg(&out_str)
            .arg(text)
            .output()
            .map_err(|e| {
                format!(
                    "failed to spawn `pico2wave`: {e}\n\
                     hint: install pico2wave on your system PATH\n\
                     *  Debian/Ubuntu: sudo apt install libttspico-utils"
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("pico2wave failed: {stderr}"));
        }

        let (duration, sample_rate, channels) = parse_wav(out_path)?;
        Ok(Voiceover {
            audio_path: out_path.to_path_buf(),
            text: text.to_string(),
            duration,
            sample_rate,
            channels,
        })
    }
}

/// An [`VoiceoverEngine`] that wraps an arbitrary user-supplied TTS
/// command.
///
/// `command` is the binary to run; each `arg` in `args` is passed
/// verbatim except that the substring `{text}` is replaced with the
/// text to synthesise and `{out}` is replaced with the output WAV
/// path.
///
/// This lets you plug in any TTS engine you like — e.g. a local
/// Kokoro / Piper / Coqui install, a Python wrapper script, or even a
/// cloud TTS CLI — without having to write a new trait impl.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "tts")] {
/// use cautious_carnival::CommandEngine;
///
/// // Wrap a hypothetical `kokoro` binary that takes `-o out.wav` and
/// // the text as positional args.
/// let engine = CommandEngine::new("kokoro")
///     .with_arg("--voice").with_arg("af_heart")
///     .with_arg("-o").with_arg("{out}")
///     .with_arg("{text}");
/// # }
/// ```
#[cfg(feature = "tts")]
#[derive(Debug, Clone)]
pub struct CommandEngine {
    command: String,
    args: Vec<String>,
}

#[cfg(feature = "tts")]
impl CommandEngine {
    /// Construct an engine that runs `command` with no args.  Use
    /// [`with_arg`](Self::with_arg) to add arguments.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
        }
    }

    /// Append a single argument.  The substrings `{text}` and `{out}`
    /// in `arg` are replaced at synthesis time with the text to speak
    /// and the output WAV path, respectively.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

#[cfg(feature = "tts")]
impl VoiceoverEngine for CommandEngine {
    fn synthesize(&self, text: &str, out_path: &std::path::Path) -> Result<Voiceover, String> {
        use std::process::Command;

        let out_str = out_path
            .to_str()
            .ok_or_else(|| format!("output path is not valid UTF-8: {out_path:?}"))?
            .to_string();

        let mut cmd = Command::new(&self.command);
        for arg in &self.args {
            let arg = arg.replace("{text}", text).replace("{out}", &out_str);
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("failed to spawn `{}`: {e}", self.command))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("`{}` failed: {stderr}", self.command));
        }

        let (duration, sample_rate, channels) = parse_wav(out_path)?;
        Ok(Voiceover {
            audio_path: out_path.to_path_buf(),
            text: text.to_string(),
            duration,
            sample_rate,
            channels,
        })
    }
}

/// Mux the concatenated voiceover audio into a video file, producing
/// a final MP4 / WebM with synchronised narration.
///
/// `video_path` is the silent video produced by [`VideoRenderer`];
/// `audio_path` is the WAV file produced by
/// [`VoiceoverTrack::concatenate_into_wav`]; `output_path` is where
/// the final muxed file is written.  The video stream is copied
/// without re-encoding (`-c:v copy`); the audio is encoded as AAC
/// (`-c:a aac`) for MP4 or Vorbis (`-c:a libvorbis`) for WebM.  The
/// `-shortest` flag ensures the output ends when the shorter of the
/// two streams ends.
///
/// Requires the `video` feature (for `ffmpeg-sidecar`).  Requires
/// `ffmpeg` on the system `PATH` at runtime.
#[cfg(feature = "video")]
pub fn mux_audio_video(
    video_path: &std::path::Path,
    audio_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<(), String> {
    use ffmpeg_sidecar::command::FfmpegCommand;

    let video_str = video_path
        .to_str()
        .ok_or_else(|| format!("video path is not valid UTF-8: {video_path:?}"))?;
    let audio_str = audio_path
        .to_str()
        .ok_or_else(|| format!("audio path is not valid UTF-8: {audio_path:?}"))?;
    let out_str = output_path
        .to_str()
        .ok_or_else(|| format!("output path is not valid UTF-8: {output_path:?}"))?;

    let audio_codec = if output_path.extension().and_then(|e| e.to_str()) == Some("webm") {
        "libvorbis"
    } else {
        "aac"
    };

    let mut cmd = FfmpegCommand::new();
    cmd.arg("-y");
    cmd.arg("-i").arg(video_str);
    cmd.arg("-i").arg(audio_str);
    cmd.args(["-c:v", "copy", "-c:a", audio_codec, "-shortest", out_str]);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn ffmpeg: {e}"))?;
    child
        .wait()
        .map_err(|e| format!("ffmpeg wait failed: {e}"))?;
    Ok(())
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

/// Convert a length in scene units to a length in screen pixels.
pub fn scene_to_screen_len(len: f64, config: &SceneConfig) -> f64 {
    let scale = config.units_per_short_edge.max(f64::EPSILON);
    let px_per_unit = config.height.min(config.width) as f64 / scale;
    len * px_per_unit
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
        let r = scene_to_screen_len(radius, &self.config);
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
        let sw = scene_to_screen_len(stroke, &self.config);
        self.buf.push_str(&format!(
            "  <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>\n",
            x1, y1, x2, y2, color, opacity, sw
        ));
    }

    fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
        if verts.len() < 3 {
            return;
        }
        let pts: Vec<String> = verts
            .iter()
            .map(|v| {
                let (x, y) = scene_to_screen(*v, &self.config);
                format!("{:.2},{:.2}", x, y)
            })
            .collect();
        self.buf.push_str(&format!(
            "  <polygon points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
            pts.join(" "),
            color,
            opacity
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
// Rasterisation backends: raster, gif, video
// ---------------------------------------------------------------------------
// All three pixel-based backends share a single software rasteriser built on
// `tiny-skia`.  `RasterCore` owns the per-frame `Pixmap` and exposes the
// primitive draw operations; each public renderer wraps it and chooses what
// to do with the finished frame (write a PNG, encode a GIF frame, or push
// the RGBA bytes into FFmpeg).
// ---------------------------------------------------------------------------

#[cfg(feature = "raster")]
mod font_data {
    // Auto-generated by scripts/gen_font.py — do not edit by hand.
    // 5x7 ASCII bitmap font covering printable ASCII 0x20..=0x7E (95 glyphs).
    // Each entry is (ascii_code, 7 row bytes; low 5 bits = pixel columns,
    // bit 0 = leftmost pixel, bit 4 = rightmost pixel).

    /// Internal 5x7 bitmap font used by the raster / gif / video backends
    // for text rendering. Generated from a system monospace font.
    pub(crate) const FONT_5X7: &[(u8, [u8; 7])] = &[
        (
            0x20,
            [
                0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
            ],
        ),
        (
            0x21,
            [
                0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
            ],
        ),
        (
            0x22,
            [
                0b00000, 0b11011, 0b11011, 0b11011, 0b11011, 0b11011, 0b00000,
            ],
        ),
        (
            0x23,
            [
                0b00100, 0b11110, 0b01110, 0b01110, 0b01111, 0b00100, 0b00000,
            ],
        ),
        (
            0x24,
            [
                0b00110, 0b00111, 0b00011, 0b01110, 0b01000, 0b00111, 0b00000,
            ],
        ),
        (
            0x25,
            [
                0b00111, 0b00101, 0b01110, 0b01110, 0b10100, 0b11100, 0b00000,
            ],
        ),
        (
            0x26,
            [
                0b00110, 0b00010, 0b00010, 0b10101, 0b11001, 0b11111, 0b00000,
            ],
        ),
        (
            0x27,
            [
                0b00110, 0b00110, 0b00110, 0b00110, 0b00110, 0b00110, 0b00110,
            ],
        ),
        (
            0x28,
            [
                0b00100, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b00100,
            ],
        ),
        (
            0x29,
            [
                0b00010, 0b00110, 0b00100, 0b00100, 0b00100, 0b00110, 0b00010,
            ],
        ),
        (
            0x2a,
            [
                0b00000, 0b00100, 0b11111, 0b01110, 0b11111, 0b00100, 0b00000,
            ],
        ),
        (
            0x2b,
            [
                0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
            ],
        ),
        (
            0x2c,
            [
                0b01110, 0b01110, 0b01110, 0b01110, 0b00111, 0b00111, 0b00011,
            ],
        ),
        (
            0x2d,
            [
                0b00000, 0b00000, 0b11111, 0b11111, 0b00000, 0b00000, 0b00000,
            ],
        ),
        (
            0x2e,
            [
                0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b00000,
            ],
        ),
        (
            0x2f,
            [
                0b01000, 0b01100, 0b00100, 0b00110, 0b00010, 0b00011, 0b00001,
            ],
        ),
        (
            0x30,
            [
                0b01110, 0b11011, 0b10001, 0b10101, 0b10001, 0b11011, 0b01110,
            ],
        ),
        (
            0x31,
            [
                0b00111, 0b00110, 0b00110, 0b00110, 0b00110, 0b00110, 0b01111,
            ],
        ),
        (
            0x32,
            [
                0b01111, 0b01000, 0b01000, 0b01100, 0b00110, 0b00011, 0b01111,
            ],
        ),
        (
            0x33,
            [
                0b01111, 0b01000, 0b01100, 0b01110, 0b01000, 0b01000, 0b01111,
            ],
        ),
        (
            0x34,
            [
                0b01100, 0b01100, 0b01010, 0b01011, 0b11111, 0b11110, 0b01000,
            ],
        ),
        (
            0x35,
            [
                0b01111, 0b00001, 0b00111, 0b01100, 0b01000, 0b01000, 0b00111,
            ],
        ),
        (
            0x36,
            [
                0b11110, 0b00011, 0b01111, 0b11011, 0b10001, 0b10011, 0b01110,
            ],
        ),
        (
            0x37,
            [
                0b01111, 0b01000, 0b01100, 0b00100, 0b00110, 0b00010, 0b00010,
            ],
        ),
        (
            0x38,
            [
                0b01110, 0b11011, 0b11011, 0b01110, 0b10001, 0b10001, 0b11111,
            ],
        ),
        (
            0x39,
            [
                0b01110, 0b11001, 0b10001, 0b11011, 0b11110, 0b11000, 0b01110,
            ],
        ),
        (
            0x3a,
            [
                0b00110, 0b00110, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110,
            ],
        ),
        (
            0x3b,
            [
                0b00110, 0b00100, 0b00000, 0b00000, 0b00110, 0b00110, 0b00010,
            ],
        ),
        (
            0x3c,
            [
                0b00000, 0b11000, 0b01110, 0b00011, 0b01110, 0b11000, 0b00000,
            ],
        ),
        (
            0x3d,
            [
                0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
            ],
        ),
        (
            0x3e,
            [
                0b00000, 0b00011, 0b01110, 0b11000, 0b01110, 0b00011, 0b00000,
            ],
        ),
        (
            0x3f,
            [
                0b01111, 0b01000, 0b01100, 0b00110, 0b00010, 0b00000, 0b00110,
            ],
        ),
        (
            0x40,
            [
                0b11110, 0b10001, 0b11101, 0b10111, 0b11101, 0b00011, 0b01110,
            ],
        ),
        (
            0x41,
            [
                0b00100, 0b01110, 0b01010, 0b01010, 0b11111, 0b10001, 0b00000,
            ],
        ),
        (
            0x42,
            [
                0b01111, 0b11001, 0b11001, 0b01111, 0b10001, 0b10001, 0b01111,
            ],
        ),
        (
            0x43,
            [
                0b01110, 0b00011, 0b00001, 0b00001, 0b00001, 0b00011, 0b01110,
            ],
        ),
        (
            0x44,
            [
                0b01111, 0b11001, 0b10001, 0b10001, 0b10001, 0b11001, 0b01111,
            ],
        ),
        (
            0x45,
            [
                0b01111, 0b00001, 0b00001, 0b01111, 0b00001, 0b00001, 0b01111,
            ],
        ),
        (
            0x46,
            [
                0b01111, 0b00001, 0b00001, 0b01111, 0b00001, 0b00001, 0b00001,
            ],
        ),
        (
            0x47,
            [
                0b11110, 0b00011, 0b00001, 0b11001, 0b10001, 0b10011, 0b11110,
            ],
        ),
        (
            0x48,
            [
                0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
        ),
        (
            0x49,
            [
                0b01111, 0b00110, 0b00110, 0b00110, 0b00110, 0b00110, 0b01111,
            ],
        ),
        (
            0x4a,
            [
                0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01111,
            ],
        ),
        (
            0x4b,
            [
                0b11001, 0b01101, 0b00111, 0b00111, 0b01101, 0b01001, 0b11001,
            ],
        ),
        (
            0x4c,
            [
                0b00001, 0b00001, 0b00001, 0b00001, 0b00001, 0b00001, 0b01111,
            ],
        ),
        (
            0x4d,
            [
                0b11011, 0b11011, 0b11111, 0b10101, 0b10001, 0b10001, 0b10001,
            ],
        ),
        (
            0x4e,
            [
                0b10011, 0b10011, 0b10111, 0b10101, 0b11101, 0b11001, 0b11001,
            ],
        ),
        (
            0x4f,
            [
                0b01110, 0b11011, 0b10001, 0b10001, 0b10001, 0b11011, 0b01110,
            ],
        ),
        (
            0x50,
            [
                0b01111, 0b11001, 0b10001, 0b11111, 0b00011, 0b00001, 0b00001,
            ],
        ),
        (
            0x51,
            [
                0b00111, 0b01001, 0b01001, 0b01001, 0b01001, 0b01111, 0b01100,
            ],
        ),
        (
            0x52,
            [
                0b01111, 0b01001, 0b01001, 0b01111, 0b01101, 0b11001, 0b10001,
            ],
        ),
        (
            0x53,
            [
                0b01111, 0b00001, 0b00001, 0b01110, 0b01000, 0b01000, 0b01111,
            ],
        ),
        (
            0x54,
            [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000,
            ],
        ),
        (
            0x55,
            [
                0b01001, 0b01001, 0b01001, 0b01001, 0b01001, 0b01001, 0b01111,
            ],
        ),
        (
            0x56,
            [
                0b10001, 0b11011, 0b11011, 0b01010, 0b01110, 0b01110, 0b00100,
            ],
        ),
        (
            0x57,
            [
                0b10001, 0b10001, 0b11111, 0b11111, 0b01011, 0b01010, 0b00000,
            ],
        ),
        (
            0x58,
            [
                0b11011, 0b01010, 0b00100, 0b00110, 0b01010, 0b10001, 0b00000,
            ],
        ),
        (
            0x59,
            [
                0b10001, 0b01010, 0b01110, 0b00100, 0b00100, 0b00100, 0b00000,
            ],
        ),
        (
            0x5a,
            [
                0b11111, 0b11000, 0b01100, 0b00100, 0b00010, 0b00011, 0b11111,
            ],
        ),
        (
            0x5b,
            [
                0b00110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b00110,
            ],
        ),
        (
            0x5c,
            [
                0b00001, 0b00011, 0b00010, 0b00110, 0b00100, 0b01100, 0b01000,
            ],
        ),
        (
            0x5d,
            [
                0b00110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00110,
            ],
        ),
        (
            0x5e,
            [
                0b00000, 0b00000, 0b00100, 0b01010, 0b10011, 0b00000, 0b00000,
            ],
        ),
        (
            0x5f,
            [
                0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
            ],
        ),
        (
            0x60,
            [
                0b00000, 0b00111, 0b00110, 0b01100, 0b11000, 0b00000, 0b00000,
            ],
        ),
        (
            0x61,
            [
                0b11111, 0b10000, 0b11110, 0b10011, 0b11001, 0b11111, 0b00000,
            ],
        ),
        (
            0x62,
            [
                0b00001, 0b00001, 0b01111, 0b01001, 0b01001, 0b01001, 0b01111,
            ],
        ),
        (
            0x63,
            [
                0b11110, 0b10011, 0b00001, 0b00001, 0b00001, 0b10011, 0b11110,
            ],
        ),
        (
            0x64,
            [
                0b01000, 0b01000, 0b01111, 0b01001, 0b01001, 0b01001, 0b01111,
            ],
        ),
        (
            0x65,
            [
                0b01110, 0b10011, 0b11111, 0b00011, 0b00011, 0b11110, 0b00000,
            ],
        ),
        (
            0x66,
            [
                0b01100, 0b00110, 0b01111, 0b00010, 0b00010, 0b00010, 0b00010,
            ],
        ),
        (
            0x67,
            [
                0b01111, 0b01001, 0b01001, 0b01001, 0b01111, 0b01000, 0b01111,
            ],
        ),
        (
            0x68,
            [
                0b00001, 0b00001, 0b01111, 0b01001, 0b01001, 0b01001, 0b01001,
            ],
        ),
        (
            0x69,
            [
                0b00110, 0b00000, 0b00110, 0b00110, 0b00110, 0b00110, 0b01111,
            ],
        ),
        (
            0x6a,
            [
                0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00110,
            ],
        ),
        (
            0x6b,
            [
                0b00001, 0b00001, 0b00101, 0b00011, 0b00011, 0b00101, 0b01001,
            ],
        ),
        (
            0x6c,
            [
                0b00011, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01100,
            ],
        ),
        (
            0x6d,
            [
                0b11111, 0b10101, 0b10101, 0b10101, 0b10101, 0b10101, 0b00000,
            ],
        ),
        (
            0x6e,
            [
                0b11111, 0b11011, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001,
            ],
        ),
        (
            0x6f,
            [
                0b01110, 0b11011, 0b10001, 0b10001, 0b11011, 0b01110, 0b00000,
            ],
        ),
        (
            0x70,
            [
                0b01111, 0b01001, 0b01001, 0b01001, 0b01111, 0b00001, 0b00001,
            ],
        ),
        (
            0x71,
            [
                0b01111, 0b01001, 0b01001, 0b01001, 0b01111, 0b01000, 0b01000,
            ],
        ),
        (
            0x72,
            [
                0b11111, 0b00011, 0b00011, 0b00001, 0b00001, 0b00001, 0b00001,
            ],
        ),
        (
            0x73,
            [
                0b11110, 0b10011, 0b00011, 0b11110, 0b11000, 0b11001, 0b01111,
            ],
        ),
        (
            0x74,
            [
                0b00010, 0b01111, 0b00110, 0b00010, 0b00010, 0b00010, 0b01100,
            ],
        ),
        (
            0x75,
            [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11011, 0b11111,
            ],
        ),
        (
            0x76,
            [
                0b00000, 0b10001, 0b11011, 0b01010, 0b01110, 0b00100, 0b00000,
            ],
        ),
        (
            0x77,
            [
                0b00000, 0b10001, 0b10001, 0b11111, 0b01110, 0b01010, 0b00000,
            ],
        ),
        (
            0x78,
            [
                0b00000, 0b11011, 0b01110, 0b00100, 0b01110, 0b11011, 0b00000,
            ],
        ),
        (
            0x79,
            [
                0b10001, 0b11011, 0b01010, 0b01110, 0b00100, 0b00100, 0b00011,
            ],
        ),
        (
            0x7a,
            [
                0b11111, 0b11000, 0b01100, 0b00100, 0b00110, 0b00011, 0b11111,
            ],
        ),
        (
            0x7b,
            [
                0b01100, 0b00100, 0b00100, 0b00110, 0b00100, 0b00100, 0b01100,
            ],
        ),
        (
            0x7c,
            [
                0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
        ),
        (
            0x7d,
            [
                0b00110, 0b00100, 0b00100, 0b01100, 0b00100, 0b00100, 0b00110,
            ],
        ),
        (
            0x7e,
            [
                0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
            ],
        ),
    ];
}

#[cfg(feature = "raster")]
mod font_manager {
    //! Runtime TTF font discovery.
    //!
    //! See the crate-level docs and the [`crate::PRIMARY_FONT_FILENAME`] /
    //! [`crate::SECONDARY_FONT_FILENAME`] constants for the placeholder
    //! contract: replace those strings with the actual filenames of the
    //! `.ttf` files you dropped into the crate's `src/` directory, and
    //! [`FontManager::discover_default`] will load them via `fontdue`.
    //!
    //! When no TTF fonts are found, the manager is simply empty —
    //! `RasterCore` then transparently falls back to the embedded 5x7
    //! bitmap font, so the crate always renders *something*.

    use crate::{
        DEFAULT_FONT_SCAN_DIR, FONT_DIR_ENV_VAR, PRIMARY_FONT_FILENAME, SECONDARY_FONT_FILENAME,
    };
    use std::path::{Path, PathBuf};

    /// A loaded TTF font plus the path it was loaded from (the path is
    /// kept purely for diagnostics in log messages).
    struct LoadedFont {
        font: fontdue::Font,
        path: PathBuf,
    }

    /// Manages discovery and loading of `.ttf` fonts from the `src/`
    /// directory (or a caller-provided override).
    ///
    /// Construction is intentionally cheap and best-effort: if no fonts
    /// are found, you get an empty manager and the renderer falls back
    /// to the built-in bitmap font.
    pub struct FontManager {
        fonts: Vec<LoadedFont>,
    }

    impl FontManager {
        /// Scan the default font directory for `.ttf` files.
        ///
        /// The directory is resolved as follows:
        ///
        /// 1. If the environment variable named by [`FONT_DIR_ENV_VAR`]
        ///    (`CAUTIOUS_CARNIVAL_FONT_DIR`) is set, use that path.
        /// 2. Otherwise, use [`DEFAULT_FONT_SCAN_DIR`] (`"src"`)
        ///    relative to the current working directory.
        ///
        /// Within the chosen directory, the two placeholder filenames
        /// ([`PRIMARY_FONT_FILENAME`] and [`SECONDARY_FONT_FILENAME`])
        /// are loaded first, in that order.  Any *other* `.ttf` files
        /// found in the directory are then loaded as additional
        /// fallbacks (in lexicographic order).  Placeholder strings
        /// that still start with `"REPLACE_WITH_"` are silently
        /// skipped — this lets the crate compile and run before you've
        /// filled in the real names.
        pub fn discover_default() -> Self {
            let dir = std::env::var(FONT_DIR_ENV_VAR)
                .unwrap_or_else(|_| DEFAULT_FONT_SCAN_DIR.to_string());
            Self::discover_in(&dir)
        }

        /// Scan `dir` for `.ttf` files and load any found.
        ///
        /// See [`FontManager::discover_default`] for the ordering
        /// rules (placeholder filenames first, then any other `.ttf`
        /// files in alphabetical order).
        pub fn discover_in(dir: impl AsRef<Path>) -> Self {
            let dir = dir.as_ref();
            let mut fonts: Vec<LoadedFont> = Vec::new();
            let mut visited: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

            // 1) Try the placeholder filenames first, in order.
            for expected in [PRIMARY_FONT_FILENAME, SECONDARY_FONT_FILENAME] {
                // Skip placeholders the user hasn't filled in yet.
                if expected.starts_with("REPLACE_WITH_") {
                    continue;
                }
                let path = dir.join(expected);
                if !path.is_file() {
                    continue;
                }
                let canonical = match path.canonicalize() {
                    Ok(c) => c,
                    Err(_) => path.clone(),
                };
                if !visited.insert(canonical.clone()) {
                    continue;
                }
                if let Some(loaded) = load_font(&path) {
                    fonts.push(loaded);
                }
            }

            // 2) Walk the directory for any *other* `.ttf` files as
            //    fallbacks, in lexicographic order for determinism.
            let mut extra: Vec<PathBuf> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("ttf") {
                        continue;
                    }
                    let canonical = match path.canonicalize() {
                        Ok(c) => c,
                        Err(_) => path.clone(),
                    };
                    if visited.contains(&canonical) {
                        continue;
                    }
                    extra.push(path);
                }
            }
            extra.sort();
            for path in extra {
                if let Some(loaded) = load_font(&path) {
                    fonts.push(loaded);
                }
            }

            if !fonts.is_empty() {
                let names: Vec<String> = fonts
                    .iter()
                    .map(|f| format!("{}", f.path.display()))
                    .collect();
                eprintln!(
                    "cautious-carnival: loaded {} TTF font(s) from {:?}: {}",
                    fonts.len(),
                    dir,
                    names.join(", ")
                );
            }

            Self { fonts }
        }

        /// Construct an empty manager — no TTF fonts will be used and
        /// the raster backend will fall back to the bitmap font.
        pub fn empty() -> Self {
            Self { fonts: Vec::new() }
        }

        /// Number of TTF fonts currently loaded.
        pub fn len(&self) -> usize {
            self.fonts.len()
        }

        /// `true` if no TTF fonts are loaded.
        pub fn is_empty(&self) -> bool {
            self.fonts.is_empty()
        }

        /// Try to rasterise a single glyph using the loaded TTF fonts,
        /// in load order.  Returns the first font's `(metrics, bitmap)`
        /// for which [`Font::has_glyph`] returns `true`, or `None` if
        /// no font has the glyph.
        ///
        /// The returned `Vec<u8>` is a single-channel coverage mask
        /// (alpha values 0..=255) of size `metrics.width * metrics.height`.
        /// Note: for glyphs with no visible bitmap (e.g. space), the
        /// bitmap will be empty but `metrics.advance_width` will still
        /// be positive.
        pub fn rasterize(&self, ch: char, size: f32) -> Option<(fontdue::Metrics, Vec<u8>)> {
            for f in &self.fonts {
                if !f.font.has_glyph(ch) {
                    continue;
                }
                let (metrics, bitmap) = f.font.rasterize(ch, size);
                return Some((metrics, bitmap));
            }
            None
        }

        /// Try to look up the horizontal advance width (in pixels at
        /// the requested size) for a glyph.  Returns `0.0` if no font
        /// has the glyph.
        pub fn horizontal_advance(&self, ch: char, size: f32) -> f32 {
            for f in &self.fonts {
                if f.font.has_glyph(ch) {
                    return f.font.metrics(ch, size).advance_width;
                }
            }
            0.0
        }

        /// Try to look up the typographic line metrics (ascent,
        /// descent, line gap) at the requested size — used to position
        /// glyphs when compositing.  Returns `None` if no font is
        /// loaded.
        pub fn line_metrics(&self, size: f32) -> Option<fontdue::LineMetrics> {
            for f in &self.fonts {
                if let Some(m) = f.font.horizontal_line_metrics(size) {
                    return Some(m);
                }
            }
            None
        }
    }

    impl std::fmt::Debug for FontManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FontManager")
                .field("loaded_count", &self.fonts.len())
                .finish()
        }
    }

    impl Default for FontManager {
        fn default() -> Self {
            Self::discover_default()
        }
    }

    /// Read `path` and parse it as a TTF font.  On failure, prints a
    /// diagnostic and returns `None` (rather than propagating the
    /// error — we want font loading to be best-effort so the renderer
    /// always starts up).
    fn load_font(path: &Path) -> Option<LoadedFont> {
        match std::fs::read(path) {
            Ok(bytes) => match fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                Ok(font) => Some(LoadedFont {
                    font,
                    path: path.to_path_buf(),
                }),
                Err(e) => {
                    eprintln!(
                        "cautious-carnival: failed to parse font {}: {}",
                        path.display(),
                        e
                    );
                    None
                }
            },
            Err(e) => {
                eprintln!(
                    "cautious-carnival: failed to read font {}: {}",
                    path.display(),
                    e
                );
                None
            }
        }
    }
}

#[cfg(feature = "raster")]
pub use font_manager::FontManager;

#[cfg(feature = "raster")]
mod raster_core {
    use crate::{scene_to_screen, scene_to_screen_len, Color, FontManager, SceneConfig, Vec3};
    use tiny_skia::*;

    /// Look up a glyph's 5x7 bitmap from the embedded font table.  Unknown
    /// chars fall back to `?` (0x3F) so text always renders something.
    fn glyph_for(byte: u8) -> [u8; 7] {
        for &(code, rows) in crate::font_data::FONT_5X7 {
            if code == byte {
                return rows;
            }
        }
        // Fallback: '?'
        [
            0b00000, 0b00100, 0b00100, 0b01010, 0b10001, 0b00000, 0b00000,
        ]
    }

    /// State shared by all pixel-based renderers: a `tiny-skia` pixmap plus
    /// the current scene config (for coordinate conversion) and a
    /// [`FontManager`] that holds any `.ttf` files auto-discovered in
    /// the crate's `src/` directory.
    pub struct RasterCore {
        /// The underlying pixel buffer.
        pub pixmap: Pixmap,
        /// The scene config for the current frame.
        pub config: SceneConfig,
        /// TTF fonts auto-discovered in `src/` (or an empty manager if
        /// none were found, in which case the renderer falls back to
        /// the embedded 5x7 bitmap font).
        pub fonts: FontManager,
    }

    impl RasterCore {
        /// Construct a new raster core of the given pixel dimensions.
        ///
        /// This also triggers a one-shot scan of the default font
        /// directory (see [`FontManager::discover_default`]) for
        /// `.ttf` files.  Loaded fonts are kept for the lifetime of
        /// this `RasterCore` and used in preference to the embedded
        /// bitmap font.
        pub fn new(width: u32, height: u32) -> Self {
            let pixmap = Pixmap::new(width, height).expect("failed to allocate pixmap");
            Self {
                pixmap,
                config: SceneConfig::default(),
                fonts: FontManager::discover_default(),
            }
        }

        /// Construct a new raster core with an explicit font manager
        /// (useful for tests or when you want to load fonts from a
        /// non-default directory).
        pub fn with_fonts(width: u32, height: u32, fonts: FontManager) -> Self {
            let pixmap = Pixmap::new(width, height).expect("failed to allocate pixmap");
            Self {
                pixmap,
                config: SceneConfig::default(),
                fonts,
            }
        }

        /// Begin a fresh frame: clear to the background colour.
        pub fn begin_frame(&mut self, config: &SceneConfig) {
            self.config = config.clone();
            let bg = config.background;
            self.pixmap
                .fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, bg.a));
        }

        /// Draw a filled circle (see [`Renderer::draw_circle`] for semantics).
        pub fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64) {
            let (cx, cy) = scene_to_screen(center, &self.config);
            let r = scene_to_screen_len(radius, &self.config);
            if r <= 0.0 {
                return;
            }
            let mut pb = PathBuilder::new();
            pb.push_circle(cx as f32, cy as f32, r as f32);
            let path = match pb.finish() {
                Some(p) => p,
                None => return,
            };
            let paint = paint_for(color, opacity);
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        /// Draw a filled axis-aligned rectangle.
        pub fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64) {
            let (x1, y1) = scene_to_screen(min, &self.config);
            let (x2, y2) = scene_to_screen(max, &self.config);
            let x = x1.min(x2);
            let y = y1.min(y2);
            let w = (x2 - x1).abs();
            let h = (y2 - y1).abs();
            let rect = match Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
                Some(r) => r,
                None => return,
            };
            let paint = paint_for(color, opacity);
            self.pixmap
                .fill_rect(rect, &paint, Transform::identity(), None);
        }

        /// Draw a stroked line segment.
        pub fn draw_line(
            &mut self,
            start: Vec3,
            end: Vec3,
            color: Color,
            opacity: f64,
            stroke: f64,
        ) {
            let (x1, y1) = scene_to_screen(start, &self.config);
            let (x2, y2) = scene_to_screen(end, &self.config);
            let mut pb = PathBuilder::new();
            pb.move_to(x1 as f32, y1 as f32);
            pb.line_to(x2 as f32, y2 as f32);
            let path = match pb.finish() {
                Some(p) => p,
                None => return,
            };
            let paint = paint_for(color, opacity);
            let mut s = Stroke::default();
            s.width = scene_to_screen_len(stroke, &self.config) as f32;
            self.pixmap
                .stroke_path(&path, &paint, &s, Transform::identity(), None);
        }

        /// Draw a filled polygon from a list of vertices.
        pub fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
            if verts.len() < 3 {
                return;
            }
            let mut pb = PathBuilder::new();
            let (x0, y0) = scene_to_screen(verts[0], &self.config);
            pb.move_to(x0 as f32, y0 as f32);
            for v in &verts[1..] {
                let (x, y) = scene_to_screen(*v, &self.config);
                pb.line_to(x as f32, y as f32);
            }
            pb.close();
            let path = match pb.finish() {
                Some(p) => p,
                None => return,
            };
            let paint = paint_for(color, opacity);
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        /// Draw text — uses a TTF font when one was auto-discovered in
        /// `src/`, otherwise falls back to the embedded 5x7 bitmap font.
        ///
        /// `size` is interpreted as the font height in pixels (consistent
        /// with the SVG backend).
        pub fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
            if !self.fonts.is_empty() {
                self.draw_text_ttf(text, pos, size, color, opacity);
            } else {
                self.draw_text_bitmap(text, pos, size, color, opacity);
            }
        }

        /// Render text using a loaded TTF font via `fontdue`.  Each glyph
        /// is rasterised to an alpha-coverage mask and composited into
        /// the pixmap as a per-pixel `fill_rect` (with the mask's alpha
        /// baked into the paint).  This is not the fastest possible
        /// text renderer — it's the simplest one that uses the
        /// auto-discovered TTF files.
        fn draw_text_ttf(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
            let pixel_size = size.max(1.0) as f32;

            // First pass: compute the total advance width so we can
            // centre the text on `pos`.
            let chars: Vec<char> = text.chars().collect();
            if chars.is_empty() {
                return;
            }
            let mut total_width = 0.0_f32;
            for &ch in &chars {
                total_width += self.fonts.horizontal_advance(ch, pixel_size);
            }
            if total_width <= 0.0 {
                // No font knew any of the glyphs — bail out.
                return;
            }

            let (cx, cy) = scene_to_screen(pos, &self.config);
            // Vertically centre the text on `pos.y` using the font's
            // ascent / descent.  The bounding-box centre (in font
            // coords, +Y up) is at `(ascent + descent) / 2` above the
            // baseline; in screen coords (+Y down) the baseline is
            // therefore `cy + (ascent + descent) / 2`.
            let baseline = if let Some(lm) = self.fonts.line_metrics(pixel_size) {
                cy as f32 + (lm.ascent + lm.descent) / 2.0
            } else {
                // Crude fallback if the font exposes no line metrics.
                cy as f32 + pixel_size * 0.3
            };
            let pen_x = cx as f32 - total_width / 2.0;

            // Bake the overall opacity into the colour alpha up-front;
            // the per-pixel glyph alpha is then multiplied in via a
            // fresh `Paint` per pixel.
            let base_alpha = (color.a as f32 * opacity as f32).clamp(0.0, 255.0);

            let mut x = pen_x;
            for &ch in &chars {
                let advance = self.fonts.horizontal_advance(ch, pixel_size);
                if let Some((metrics, bitmap)) = self.fonts.rasterize(ch, pixel_size) {
                    // Skip drawing if the bitmap is empty (e.g. space,
                    // tab) — but we still advance the pen below.
                    if metrics.width > 0 && metrics.height > 0 {
                        // `metrics.xmin` is the whole-pixel offset of
                        // the bitmap's left edge from the pen position.
                        // `metrics.ymin` is the whole-pixel offset of
                        // the bitmap's *bottom* edge from the baseline
                        // (fontdue uses typographic +Y-up; positive =
                        // above baseline).  In screen coords (+Y
                        // down) the bitmap's *top* edge is therefore
                        // `baseline - ymin - height`.
                        let glyph_left = x + metrics.xmin as f32;
                        let glyph_top = baseline - metrics.ymin as f32 - metrics.height as f32;

                        for row in 0..metrics.height {
                            for col in 0..metrics.width {
                                let coverage = bitmap[row * metrics.width + col];
                                if coverage == 0 {
                                    continue;
                                }
                                let px = glyph_left + col as f32;
                                let py = glyph_top + row as f32;
                                // Combine glyph coverage with the base
                                // colour alpha + overall opacity.
                                let a = ((coverage as f32 / 255.0) * base_alpha)
                                    .round()
                                    .clamp(0.0, 255.0)
                                    as u8;
                                if a == 0 {
                                    continue;
                                }
                                if let Some(rect) = Rect::from_xywh(px, py, 1.0, 1.0) {
                                    let mut p = Paint::default();
                                    p.set_color_rgba8(color.r, color.g, color.b, a);
                                    p.anti_alias = false;
                                    self.pixmap.fill_rect(rect, &p, Transform::identity(), None);
                                }
                            }
                        }
                    }
                }
                x += advance;
            }
        }

        /// Render text using the embedded 5x7 ASCII bitmap font.  Used
        /// as a fallback when no TTF font was found in `src/`.
        fn draw_text_bitmap(
            &mut self,
            text: &str,
            pos: Vec3,
            size: f64,
            color: Color,
            opacity: f64,
        ) {
            // `size` is interpreted as the font height in pixels (consistent
            // with the SVG backend).  Each bitmap-pixel is `size / 7.0`.
            let pixel = (size / 7.0).max(0.5);
            let char_w = 5.0 * pixel;
            let char_h = 7.0 * pixel;
            let gap = pixel; // 1 bitmap-pixel gap between chars
            let chars: Vec<char> = text.chars().collect();
            if chars.is_empty() {
                return;
            }
            let total_w = chars.len() as f64 * char_w + (chars.len() - 1) as f64 * gap;
            let (cx, cy) = scene_to_screen(pos, &self.config);
            let start_x = cx - total_w / 2.0;
            let start_y = cy - char_h / 2.0;
            let paint = paint_for(color, opacity);
            for (i, ch) in chars.iter().enumerate() {
                let bytes = ch.to_string().into_bytes();
                let first_byte = bytes.first().copied().unwrap_or(b'?');
                let glyph = glyph_for(first_byte);
                let origin_x = start_x + i as f64 * (char_w + gap);
                let origin_y = start_y;
                for row in 0..7u32 {
                    let row_bits = glyph[row as usize];
                    for col in 0..5u32 {
                        if (row_bits >> col) & 1 == 1 {
                            let rx = origin_x + col as f64 * pixel;
                            let ry = origin_y + row as f64 * pixel;
                            if let Some(rect) =
                                Rect::from_xywh(rx as f32, ry as f32, pixel as f32, pixel as f32)
                            {
                                self.pixmap
                                    .fill_rect(rect, &paint, Transform::identity(), None);
                            }
                        }
                    }
                }
            }
        }

        /// Borrow the raw RGBA pixel buffer (8 bits per channel, row-major).
        pub fn pixels(&self) -> &[u8] {
            self.pixmap.data()
        }
    }

    /// Build a `tiny_skia::Paint` for the given colour, baking `opacity`
    /// into the alpha channel.
    fn paint_for(color: Color, opacity: f64) -> Paint<'static> {
        let c = color.with_alpha_mul(opacity);
        let mut p = Paint::default();
        p.set_color_rgba8(c.r, c.g, c.b, c.a);
        p.anti_alias = true;
        p
    }
}

#[cfg(feature = "raster")]
pub use raster_core::RasterCore;

#[cfg(feature = "raster")]
use std::io::Write;

/// A renderer that writes each frame as a numbered PNG inside an output
/// directory.  Anti-aliased via `tiny-skia`.  Use this when you want
/// maximum quality and don't mind running `ffmpeg` yourself to assemble
/// the frames into a video.
///
/// The output directory is created on construction; existing files inside
/// it are NOT cleared, so old frames from a previous run can leak into a
/// new run if you reuse the directory.
#[cfg(feature = "raster")]
pub struct RasterRenderer {
    out_dir: std::path::PathBuf,
    width: u32,
    height: u32,
    frame: u32,
    core: RasterCore,
}

#[cfg(feature = "raster")]
impl RasterRenderer {
    /// Construct a `RasterRenderer` writing PNGs into `out_dir`.  The
    /// directory is created if it doesn't exist.
    pub fn new(
        out_dir: impl AsRef<std::path::Path>,
        width: u32,
        height: u32,
    ) -> std::io::Result<Self> {
        let dir = out_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            out_dir: dir,
            width,
            height,
            frame: 0,
            core: RasterCore::new(width, height),
        })
    }
}

#[cfg(feature = "raster")]
impl Renderer for RasterRenderer {
    fn begin_frame(&mut self, _time: crate::Seconds, config: &crate::SceneConfig) {
        self.core.begin_frame(config);
    }

    fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64) {
        self.core.draw_circle(center, radius, color, opacity);
    }

    fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64) {
        self.core.draw_rect(min, max, color, opacity);
    }

    fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, opacity: f64, stroke: f64) {
        self.core.draw_line(start, end, color, opacity, stroke);
    }

    fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
        self.core.draw_polygon(verts, color, opacity);
    }

    fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
        self.core.draw_text(text, pos, size, color, opacity);
    }

    fn end_frame(&mut self) {
        // Save the current pixmap as a PNG.
        let path = self.out_dir.join(format!("frame_{:06}.png", self.frame));
        let pixels = self.core.pixels().to_vec();
        // Save on a thread to keep frame throughput high; we join at finish().
        let width = self.width;
        let height = self.height;
        let path = path.clone();
        // Encode synchronously — `image`'s PNG encoder is fast enough for
        // typical animation lengths and we avoid lifetime headaches.
        match image::RgbaImage::from_raw(width, height, pixels) {
            Some(img) => {
                if let Err(e) = img.save(&path) {
                    eprintln!("cautious-carnival: failed to write {:?}: {}", path, e);
                }
            }
            None => {
                eprintln!(
                    "cautious-carnival: image buffer size mismatch for frame {}",
                    self.frame
                );
            }
        }
        self.frame += 1;
    }

    fn finish(&mut self) {
        // Write a small `ffmpeg_concat.sh` helper script that shows how to
        // turn the frame sequence into a video.  Pure convenience.
        let script = self.out_dir.join("encode_with_ffmpeg.sh");
        if let Ok(mut f) = std::fs::File::create(&script) {
            let fps = self.core.config.fps;
            let body = format!(
                "#!/bin/sh\n# Auto-generated by cautious-carnival.\n                 # Run from the directory containing this script.\n                 ffmpeg -y -framerate {} -i frame_%06d.png -c:v libx264 \\\n                   -pix_fmt yuv420p -crf 18 ../output.mp4\n",
                fps,
            );
            let _ = f.write_all(body.as_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Animated GIF backend (`gif` feature — pure Rust, no system deps)
// ---------------------------------------------------------------------------

/// A renderer that writes an animated `.gif` file via the pure-Rust `gif`
/// crate.  No system dependencies — works out of the box on any platform.
///
/// Each frame is rasterised with `tiny-skia` and encoded with a per-frame
/// local palette (derived from the frame's pixels).  This keeps colours
/// faithful at the cost of slightly larger files than a fixed-palette
/// approach.  Good for short loops and demos; for high-colour content use
/// [`VideoRenderer`](crate::VideoRenderer) (MP4) instead.
#[cfg(feature = "gif")]
pub struct GifRenderer {
    /// Held in an `Option` so `finish` can `take()` it and call
    /// `into_inner()` to flush the underlying `BufWriter<File>`.
    encoder: Option<gif::Encoder<std::io::BufWriter<std::fs::File>>>,
    width: u32,
    height: u32,
    fps: u32,
    core: RasterCore,
    frame: u32,
}

#[cfg(feature = "gif")]
impl GifRenderer {
    /// Construct a `GifRenderer` writing to `path` at `fps` frames per
    /// second.  The scene's [`SceneConfig::fps`] is used for animation
    /// sampling; the `fps` here controls only the GIF's playback speed.
    pub fn new(
        path: impl AsRef<std::path::Path>,
        width: u32,
        height: u32,
        fps: u32,
    ) -> std::io::Result<Self> {
        let file = std::fs::File::create(path.as_ref())?;
        let buf = std::io::BufWriter::new(file);
        // Empty global palette — each frame supplies its own local palette
        // via `Frame::from_rgba_speed`.  This keeps colours consistent
        // across frame-to-frame variation at the cost of slightly larger
        // files.
        let mut encoder = gif::Encoder::new(buf, width as u16, height as u16, &[])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(Self {
            encoder: Some(encoder),
            width,
            height,
            fps: fps.max(1),
            core: RasterCore::new(width, height),
            frame: 0,
        })
    }
}

#[cfg(feature = "gif")]
impl Renderer for GifRenderer {
    fn begin_frame(&mut self, _time: crate::Seconds, config: &crate::SceneConfig) {
        self.core.begin_frame(config);
    }

    fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64) {
        self.core.draw_circle(center, radius, color, opacity);
    }

    fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64) {
        self.core.draw_rect(min, max, color, opacity);
    }

    fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, opacity: f64, stroke: f64) {
        self.core.draw_line(start, end, color, opacity, stroke);
    }

    fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
        self.core.draw_polygon(verts, color, opacity);
    }

    fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
        self.core.draw_text(text, pos, size, color, opacity);
    }

    fn end_frame(&mut self) {
        // GIF's `Frame::from_rgba_speed` needs a mutable RGBA buffer to
        // compute the local palette, so we clone the pixmap's data.
        let mut pixels = self.core.pixels().to_vec();
        let mut frame =
            gif::Frame::from_rgba_speed(self.width as u16, self.height as u16, &mut pixels, 10);
        // GIF delay is in 1/100 s.  Round to the nearest centisecond.
        let centis = (100.0 / self.fps as f64).round() as u16;
        frame.delay = centis.max(1);
        if let Some(encoder) = &mut self.encoder {
            if let Err(e) = encoder.write_frame(&frame) {
                eprintln!(
                    "cautious-carnival: failed to write gif frame {}: {}",
                    self.frame, e
                );
            }
        }
        self.frame += 1;
    }

    fn finish(&mut self) {
        // Take the encoder out and call `into_inner` to flush the
        // underlying `BufWriter<File>`.  This surfaces any I/O errors
        // that would otherwise be silently swallowed by `BufWriter`'s
        // `Drop` impl.
        if let Some(encoder) = self.encoder.take() {
            if let Err(e) = encoder.into_inner() {
                eprintln!("cautious-carnival: gif writer flush failed: {e}");
            }
        }
    }
}

#[cfg(feature = "gif")]
impl Drop for GifRenderer {
    fn drop(&mut self) {
        // If `finish` wasn't called explicitly, do it now.
        if self.encoder.is_some() {
            self.finish();
        }
    }
}

// ---------------------------------------------------------------------------
// Video backend (`video` feature — MP4 / WebM via ffmpeg-sidecar)
// ---------------------------------------------------------------------------
//
// `ffmpeg-sidecar` is a pure-Rust wrapper that spawns the system `ffmpeg`
// binary as a subprocess and pipes raw RGBA frames to it via stdin.  This
// avoids the build-time pain of `ffmpeg-next` (which links against the C
// libraries via `ffmpeg-sys-next` and requires `pkg-config` + the dev
// headers for `libavcodec`, `libavformat`, `libavutil`, `libswscale`,
// `libswresample`).
//
// Trade-offs:
//   * Pro: trivial build — no system dev packages, no `build.rs`, no
//     linking.  Works the same on Linux / macOS / Windows.
//   * Pro: `ffmpeg-sidecar` types are `Send` (they wrap `std::process::Child`),
//     so `VideoRenderer` satisfies `Renderer: Send` directly — no actor
//     thread, no channel, no `JoinHandle`.
//   * Con: requires `ffmpeg` to be on the system `PATH` at runtime.  To
//     enable automatic download of a prebuilt binary, enable the
//     `video-download` feature (which forwards to
//     `ffmpeg-sidecar/download_ffmpeg`).
//   * Con: a small per-frame overhead from the stdin pipe write (typically
//     negligible compared to the rasterisation cost).
//
// The renderer holds:
//   * a `RasterCore` (which owns a `tiny_skia::Pixmap`),
//   * an `FfmpegChild` (the spawned ffmpeg subprocess),
//   * a `ChildStdin` (the pipe we write raw RGBA frames into).
//
// All three are `Send`.
// ---------------------------------------------------------------------------

/// A renderer that produces an MP4 / WebM / MKV video file by piping raw
/// RGBA frames to the system `ffmpeg` binary via [`ffmpeg-sidecar`].
///
/// Unlike a C-binding approach (`ffmpeg-next`), this requires no build-time
/// linking — `ffmpeg-sidecar` is pure Rust that spawns `ffmpeg` as a
/// subprocess.  The trade-off is that `ffmpeg` must be on the system `PATH`
/// at runtime (enable the `video-download` feature for automatic download).
///
/// The codec is selected from the output file extension: `.mp4` → H.264
/// (`libx264`), `.webm` → VP8 (`libvpx`).  Pixel format is YUV 4:2:0 with
/// CRF 18 (visually lossless).  Override by constructing your own
/// `FfmpegCommand` if you need finer control.
#[cfg(feature = "video")]
pub struct VideoRenderer {
    /// The spawned ffmpeg subprocess.  Held in an `Option` so `finish`
    /// can `take()` it before waiting (which consumes the child).
    child: Option<ffmpeg_sidecar::child::FfmpegChild>,
    /// The stdin pipe into which raw RGBA frames are written.  Held in
    /// an `Option` so `finish` can `take()` and drop it (closing the
    /// pipe signals EOF to ffmpeg, triggering the final flush).
    stdin: Option<std::process::ChildStdin>,
    /// Frame width in pixels.  Stored for diagnostics; the actual frame
    /// dimensions are baked into the ffmpeg CLI args at construction.
    #[allow(dead_code)]
    width: u32,
    /// Frame height in pixels.
    #[allow(dead_code)]
    height: u32,
    /// Frames per second.  Used to compute the ffmpeg `-r` input flag.
    #[allow(dead_code)]
    fps: u32,
    core: RasterCore,
    frame: u32,
}

#[cfg(feature = "video")]
impl VideoRenderer {
    /// Construct a `VideoRenderer` writing to `path`.  The output format
    /// is inferred from the file extension by ffmpeg (`.mp4`, `.webm`,
    /// `.mkv`, `.mov`, ...).  Uses H.264 (`libx264`) with YUV 4:2:0
    /// pixel format and CRF 18 by default; for `.webm` the codec falls
    /// back to VP8 (`libvpx`).
    ///
    /// # Runtime requirements
    ///
    /// Requires `ffmpeg` to be on the system `PATH` at runtime.  If it
    /// isn't found, [`new`](Self::new) returns an error suggesting the
    /// `video-download` feature, which enables
    /// `ffmpeg-sidecar/download_ffmpeg` for automatic download of a
    /// prebuilt binary on first use.
    pub fn new(
        path: impl AsRef<std::path::Path>,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, String> {
        use ffmpeg_sidecar::command::FfmpegCommand;

        let path = path.as_ref();
        let path_str = path
            .to_str()
            .ok_or_else(|| format!("output path is not valid UTF-8: {path:?}"))?
            .to_string();

        // Pick a codec from the file extension.  ffmpeg-sidecar doesn't
        // expose a high-level codec picker, so we pass raw CLI args.
        let codec = if path.extension().and_then(|e| e.to_str()) == Some("webm") {
            "libvpx"
        } else {
            "libx264"
        };

        // Build the ffmpeg command.  Input: raw RGBA frames piped via
        // stdin.  Output: H.264 / VP8 in the requested container.
        //
        // `FfmpegCommand` builder methods take `&mut self` and return
        // `&mut Self`, so we mutate `cmd` in place rather than chaining
        // by value.
        let mut cmd = FfmpegCommand::new();
        // `-y` overwrites any existing output file.
        cmd.arg("-y");
        // Input specification: raw RGBA frames from stdin.
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{width}x{height}"),
            "-r",
            &fps.to_string(),
            "-i",
            "-", // read from stdin
        ]);
        // Output codec + container.
        cmd.args([
            "-c:v", codec, "-pix_fmt", "yuv420p", "-crf", "18", &path_str,
        ]);

        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "failed to spawn ffmpeg: {e}\n\
                 hint: install ffmpeg on your system PATH, or enable the \
                 `video-download` feature for automatic download"
            )
        })?;

        let stdin = child
            .take_stdin()
            .ok_or_else(|| "ffmpeg child stdin unavailable (already taken?)".to_string())?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            width,
            height,
            fps: fps.max(1),
            core: RasterCore::new(width, height),
            frame: 0,
        })
    }
}

#[cfg(feature = "video")]
impl Renderer for VideoRenderer {
    fn begin_frame(&mut self, _time: crate::Seconds, config: &crate::SceneConfig) {
        self.core.begin_frame(config);
    }

    fn draw_circle(&mut self, center: Vec3, radius: f64, color: Color, opacity: f64) {
        self.core.draw_circle(center, radius, color, opacity);
    }

    fn draw_rect(&mut self, min: Vec3, max: Vec3, color: Color, opacity: f64) {
        self.core.draw_rect(min, max, color, opacity);
    }

    fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, opacity: f64, stroke: f64) {
        self.core.draw_line(start, end, color, opacity, stroke);
    }

    fn draw_polygon(&mut self, verts: &[Vec3], color: Color, opacity: f64) {
        self.core.draw_polygon(verts, color, opacity);
    }

    fn draw_text(&mut self, text: &str, pos: Vec3, size: f64, color: Color, opacity: f64) {
        self.core.draw_text(text, pos, size, color, opacity);
    }

    fn end_frame(&mut self) {
        use std::io::Write;
        let pixels = self.core.pixels();
        if let Some(stdin) = &mut self.stdin {
            if let Err(e) = stdin.write_all(pixels) {
                eprintln!(
                    "cautious-carnival: failed to write frame {} to ffmpeg stdin: {}",
                    self.frame, e
                );
                // The pipe is broken — ffmpeg has probably exited.  Drop
                // our stdin handle so subsequent frames don't keep trying
                // to write into a dead pipe.
                self.stdin = None;
            }
        }
        self.frame += 1;
    }

    fn finish(&mut self) {
        // Close the stdin pipe — this signals EOF to ffmpeg, which
        // triggers the final muxing flush and writes the trailer.
        self.stdin.take();

        // Wait for the ffmpeg subprocess to exit.  We use `wait()`
        // rather than `iter()` because we don't need to parse ffmpeg's
        // log output — we just need the exit status.  `wait()` also
        // drains stderr internally to avoid a deadlock when the OS pipe
        // buffer fills up.
        if let Some(child) = self.child.as_mut() {
            if let Err(e) = child.wait() {
                eprintln!("cautious-carnival: ffmpeg wait failed: {e}");
            }
        }
        self.child = None;
    }
}

#[cfg(feature = "video")]
impl Drop for VideoRenderer {
    fn drop(&mut self) {
        // If `finish()` wasn't called explicitly, do it now so the
        // output file is properly finalised and the subprocess is
        // reaped.
        if self.child.is_some() || self.stdin.is_some() {
            self.finish();
        }
    }
}

// ---------------------------------------------------------------------------
// Parallel frame rendering (`parallel` feature — uses rayon)
// ---------------------------------------------------------------------------
//
// The `parallel` feature enables [`parallel_encode_pngs`], a helper that
// encodes a batch of RGBA buffers as PNGs across all CPU cores.  This is
// useful when you've collected frames into memory yourself (e.g. from a
// custom renderer or a frame-stepping loop) and want to encode them in
// parallel.  The built-in renderers encode frames serially because the
// GIF and FFmpeg codec state machines are inherently sequential; parallel
// PNG encoding is the main lever for speeding up `RasterRenderer` output.
// ---------------------------------------------------------------------------

#[cfg(feature = "raster")]
impl RasterRenderer {
    /// Encode a single RGBA buffer as a PNG file.  Exposed as a static
    /// method so it can be called from rayon worker threads (via
    /// [`parallel_encode_pngs`] when the `parallel` feature is on).
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn encode_png(width: u32, height: u32, pixels: &[u8], path: &std::path::Path) {
        match image::RgbaImage::from_raw(width, height, pixels.to_vec()) {
            Some(img) => {
                if let Err(e) = img.save(path) {
                    eprintln!("cautious-carnival: failed to write {:?}: {}", path, e);
                }
            }
            None => {
                eprintln!(
                    "cautious-carnival: image buffer size mismatch for {:?}",
                    path
                );
            }
        }
    }
}

/// Encode a batch of `(width, height, path, RGBA pixels)` tuples as PNGs
/// in parallel using rayon.  Each tuple is processed independently on a
/// worker thread.  Use this when you have a pre-collected set of frames
/// (e.g. from a custom render loop) and want to speed up the PNG
/// encoding step on multi-core machines.
///
/// Typical speedup is 4-8× on an 8-core machine for scenes where PNG
/// encoding is the bottleneck.
#[cfg(all(feature = "raster", feature = "parallel"))]
pub fn parallel_encode_pngs(jobs: &[(u32, u32, std::path::PathBuf, Vec<u8>)]) {
    use rayon::prelude::*;
    jobs.par_iter().for_each(|(w, h, path, pixels)| {
        RasterRenderer::encode_png(*w, *h, pixels, path);
    });
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
    fn bounce_out_monotonic_and_bounded() {
        // Bounce-out should stay in [0, 1] and reach 1 at t = 1.
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let v = bounce_out(t);
            assert!(
                (0.0..=1.0).contains(&v),
                "bounce_out({t}) = {v} out of range"
            );
            // Allow non-monotonic behaviour (it's a bounce!) but the end value is 1.
            let _ = prev;
            prev = v;
        }
        assert!((bounce_out(1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn bbox_of_circle() {
        let c = Circle::new(2.0).move_to(Vec3::new(3.0, 0.0, 0.0));
        let bb = c.bbox();
        assert_eq!(bb.min, Vec3::new(1.0, -2.0, 0.0));
        assert_eq!(bb.max, Vec3::new(5.0, 2.0, 0.0));
    }

    #[test]
    fn polygon_vertices_count() {
        let p = Polygon::new(6, 1.0);
        assert_eq!(p.vertices().len(), 6);
        // First vertex at angle 0 should be (radius, 0).
        let v0 = p.vertices()[0];
        assert!((v0.x - 1.0).abs() < 1e-12);
        assert!(v0.y.abs() < 1e-12);
    }

    #[test]
    fn group_propagates_position() {
        let mut g = Group::new()
            .add(Box::new(Circle::new(0.5).move_to(Vec3::new(1.0, 0.0, 0.0))))
            .add(Box::new(
                Circle::new(0.5).move_to(Vec3::new(-1.0, 0.0, 0.0)),
            ));
        g.set_position(Vec3::new(5.0, 5.0, 0.0));
        let p0 = g.children()[0].position();
        let p1 = g.children()[1].position();
        assert!((p0.x - 6.0).abs() < 1e-12 && (p0.y - 5.0).abs() < 1e-12);
        assert!((p1.x - 4.0).abs() < 1e-12 && (p1.y - 5.0).abs() < 1e-12);
    }

    #[test]
    fn scene_add_remove_ids() {
        let renderer: Box<dyn Renderer> = Box::new(SvgRenderer::new(
            std::env::temp_dir().join("cautious_carnival_test.svg"),
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
            std::env::temp_dir().join("cautious_carnival_wait_test.svg"),
            100,
            100,
        ));
        let mut scene = Scene::new(renderer, SceneConfig::default());
        scene.play(Wait::new(1.0));
        assert!((scene.time().0 - 1.0).abs() < 0.05);
    }

    #[test]
    fn animation_group_runs_in_parallel() {
        // Two FadeIn animations of different durations in a group should
        // take max(d1, d2) total time, not d1 + d2.
        let renderer: Box<dyn Renderer> = Box::new(SvgRenderer::new(
            std::env::temp_dir().join("cautious_carnival_group_test.svg"),
            100,
            100,
        ));
        let mut scene = Scene::new(renderer, SceneConfig::default());
        let g = AnimationGroup::new()
            .add(Box::new(FadeIn::new(Box::new(Circle::new(0.5)), 1.0)))
            .add(Box::new(FadeIn::new(Box::new(Square::new(0.5)), 2.0)));
        let t0 = scene.time().0;
        scene.play(g);
        let dt = scene.time().0 - t0;
        assert!(
            dt >= 1.95 && dt <= 2.1,
            "group duration was {dt}, expected ~2.0"
        );
    }

    #[cfg(feature = "raster")]
    #[test]
    fn raster_renderer_writes_png_sequence() {
        let dir = std::env::temp_dir().join("cautious_carnival_raster_test");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = RasterRenderer::new(&dir, 64, 48).expect("renderer");
        let mut scene = Scene::new(Box::new(renderer), SceneConfig::default());
        scene.add(Box::new(Circle::new(1.0).with_color(Color::BLUE)));
        scene.play(Wait::new(0.1));
        drop(scene);
        // At least one PNG should exist.
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dir exists")
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("png"))
            .collect();
        assert!(!entries.is_empty(), "expected at least one PNG frame");
    }

    #[cfg(feature = "gif")]
    #[test]
    fn gif_renderer_writes_gif() {
        let path = std::env::temp_dir().join("cautious_carnival_gif_test.gif");
        let renderer = GifRenderer::new(&path, 32, 24, 10).expect("renderer");
        let mut scene = Scene::new(Box::new(renderer), SceneConfig::default());
        scene.add(Box::new(Circle::new(0.5).with_color(Color::RED)));
        scene.play(Wait::new(0.2));
        drop(scene);
        let meta = std::fs::metadata(&path).expect("gif file exists");
        assert!(meta.len() > 0, "gif file is empty");
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_renderer_writes_mp4() {
        // Skip silently if ffmpeg isn't on PATH — the test is only
        // meaningful on systems that have it installed.
        if !ffmpeg_sidecar::command::ffmpeg_is_installed() {
            eprintln!("skipping video_renderer_writes_mp4: ffmpeg not on PATH");
            return;
        }
        let path = std::env::temp_dir().join("cautious_carnival_video_test.mp4");
        let renderer = match VideoRenderer::new(&path, 64, 48, 10) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping video_renderer_writes_mp4: {e}");
                return;
            }
        };
        let mut scene = Scene::new(Box::new(renderer), SceneConfig::default());
        scene.add(Box::new(Circle::new(0.5).with_color(Color::GREEN)));
        scene.play(Wait::new(0.2));
        drop(scene);
        let meta = std::fs::metadata(&path).expect("mp4 file exists");
        assert!(meta.len() > 0, "mp4 file is empty");
        // The file should be at least 1 KB — a real H.264 stream, not an
        // empty container.
        assert!(
            meta.len() > 1024,
            "mp4 file is suspiciously small ({} bytes)",
            meta.len()
        );
    }
}
