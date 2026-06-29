# cautious-carnival

A programmable mathematical animation engine for Rust — a Manim-inspired
crate that turns pure Rust code into SVG storyboards, animated GIFs, PNG
sequences, or MP4/WebM videos.

The crate is organised around four abstractions:

* **`Mobject`** — anything that can be drawn (shapes, text, groups).
* **`Animation`** — a time-parameterised transformation of a mobject.
* **`Scene`** — the timeline that plays animations and owns the render queue.
* **`Renderer`** — the sink that turns a frame into pixels / SVG / video.

Everything lives in a single self-contained `src/lib.rs`.  No build scripts,
no proc macros, no required system dependencies — just `cargo build`.

## Feature flags

| Feature    | What it enables                                   | System deps                |
|------------|---------------------------------------------------|----------------------------|
| `svg`      | `SvgRenderer` — SVG storyboard (default)          | none                       |
| `raster`   | `RasterRenderer` — PNG frame sequence             | none                       |
| `gif`      | `GifRenderer` — animated GIF                      | none (pure Rust)           |
| `video`    | `VideoRenderer` — MP4 / WebM via FFmpeg           | `libavcodec`, `libavformat`, `libavutil`, `libswscale`, `libswresample` |
| `parallel` | `parallel_encode_pngs` — batch PNG encoding       | none (uses `rayon`)        |
| `tts`      | `VoiceoverEngine` + 3 reference engines           | `espeak-ng` or `pico2wave` on `PATH` (or any TTS binary via `CommandEngine`) |

`gif` and `video` both pull in `raster` automatically (they need the
`tiny-skia` rasteriser to produce pixel frames).  `tts` is independent —
it works with any backend, but is most useful combined with `video` so
the narration can be muxed into the final MP4.

## Font discovery (auto-scan of `src/`)

The raster / gif / video backends ship with a built-in 5x7 ASCII bitmap
font so they work out-of-the-box with zero configuration.  When you
need nicer text rendering, drop one or two `.ttf` files into the
crate's `src/` directory and they will be auto-discovered at runtime
by `FontManager::discover_default`.

Two placeholder constants control which filenames are looked up first:

```rust
// In src/lib.rs — replace these strings with your actual filenames.
pub const PRIMARY_FONT_FILENAME: &str = "REPLACE_WITH_PRIMARY_FONT_FILENAME.ttf";
pub const SECONDARY_FONT_FILENAME: &str = "REPLACE_WITH_SECONDARY_FONT_FILENAME.ttf";
```

Replace the placeholder strings with the actual filenames of the `.ttf`
files you placed in `src/` (e.g. `"Roboto-Regular.ttf"` and
`"Roboto-Bold.ttf"`).  Until you do, the raster backend falls back to
the built-in bitmap font — placeholder strings starting with
`"REPLACE_WITH_"` are silently skipped so the crate always compiles
and runs.

Any *other* `.ttf` files found in `src/` are also loaded as additional
fallbacks, in lexicographic order.  Override the scan directory at
runtime with the `CAUTIOUS_CARNIVAL_FONT_DIR` environment variable, or
use `FontManager::discover_in(path)` to scan an arbitrary directory.

## Text-to-speech voiceover (`tts` feature)

Inspired by [`kokoro-manim-voiceover`](https://pypi.org/project/kokoro-manim-voiceover/)
for Python Manim, `cautious-carnival` ships a small voiceover
subsystem that synthesises narration from text and synchronises it
with the scene timeline.

The core abstraction is the `VoiceoverEngine` trait — an engine takes
a piece of text and produces a WAV file.  Three reference
implementations are provided:

| Engine            | Wraps                | Install                                                |
|-------------------|----------------------|--------------------------------------------------------|
| `EspeakNgEngine`  | `espeak-ng` binary   | `sudo apt install espeak-ng` / `brew install espeak-ng` |
| `Pico2WaveEngine` | `pico2wave` binary   | `sudo apt install libttspico-utils`                    |
| `CommandEngine`   | any TTS command      | n/a — plug in Kokoro / Piper / Coqui / cloud CLIs       |

All engines are subprocess-based — they spawn an external TTS binary
via `std::process::Command` and read back the WAV file it produces.
This keeps the crate itself free of any C dependencies.

### Adding narration to a scene

```rust
use cautious_carnival::*;
use std::path::Path;

let renderer = Box::new(VideoRenderer::new("out.mp4", 1280, 720, 60).unwrap());
let mut scene = Scene::new(renderer, SceneConfig::default());

let engine = EspeakNgEngine::new().with_voice("en-us");

// Synthesise "Hello, world!" into voiceovers/voiceover_0000.wav,
// push it onto the scene's voiceover track, and advance the scene
// clock by the audio's duration so the next animation is timed
// against the narration.
scene.add_voiceover(&engine, "Hello, world!", Path::new("voiceovers")).unwrap();

// This animation now starts after the narration finishes.
let circle = Circle::new(1.0).with_color(Color::BLUE);
scene.play(FadeIn::new(Box::new(circle), 1.0));
```

### Muxing narration into the final video

After the scene is rendered, concatenate the voiceover track into a
single WAV and mux it into the silent video:

```rust
use cautious_carnival::mux_audio_video;
use std::path::Path;

// `scene` must be dropped before muxing so VideoRenderer finalises
// the silent MP4.  Capture the track first:
let track = scene.voiceover_track().clone();
drop(scene);

if !track.is_empty() {
    track.concatenate_into_wav("narration.wav").unwrap();
    mux_audio_video(
        Path::new("out.mp4"),       // silent video
        Path::new("narration.wav"), // concatenated voiceover
        Path::new("out_narrated.mp4"),
    ).unwrap();
}
```

`mux_audio_video` requires the `video` feature (for `ffmpeg-sidecar`)
and `ffmpeg` on the system `PATH` at runtime.  The video stream is
copied without re-encoding (`-c:v copy`); the audio is encoded as AAC
for MP4 or Vorbis for WebM.

### Plugging in Kokoro / Piper / any other TTS

Use `CommandEngine` to wrap any TTS binary — the substrings `{text}`
and `{out}` in each argument are replaced at synthesis time:

```rust
use cautious_carnival::CommandEngine;

// Wrap a hypothetical `kokoro` CLI:
let engine = CommandEngine::new("kokoro")
    .with_arg("--voice").with_arg("af_heart")
    .with_arg("-o").with_arg("{out}")
    .with_arg("{text}");
```

## Quick start

```sh
# Clone and run the demo with the default SVG backend
git clone https://github.com/himanshm/cautious-carnival
cd cautious-carnival
cargo run --example hello_circle

# Animated GIF (pure Rust — no system deps!)
cargo run --example hello_circle --features gif

# MP4 video (requires FFmpeg shared libs)
cargo run --example hello_circle --features video

# MP4 video + text-to-speech narration (requires espeak-ng on PATH)
cargo run --example hello_circle --features "video tts"

# PNG frame sequence (rasterise now, encode later)
cargo run --example hello_circle --features raster

# Optimised release build for any backend
cargo run --example hello_circle --release --features gif
```

## Hello, circle

```rust
use cautious_carnival::*;

let renderer = Box::new(GifRenderer::new("out.gif", 640, 360, 30).unwrap());
let mut scene = Scene::new(renderer, SceneConfig::default());

let circle = Circle::new(1.0).with_color(Color::BLUE);
scene.play(FadeIn::new(circle.clone_box(), 1.0));
scene.play(Rotate::new(circle.clone_box(), std::f64::consts::TAU, 2.0));
scene.play(FadeOut::new(0.5));
```

## Renderers

### `SvgRenderer` (default, zero-dep)

```rust
let renderer = Box::new(SvgRenderer::new("out.svg", 1280, 720));
```

Writes a single `.svg` file with every frame stacked as a `<g>` layer.
Great as a storyboard view: open the file in a browser to see every frame
overlaid.

### `RasterRenderer` (PNG frame sequence, anti-aliased)

```rust
let renderer = Box::new(RasterRenderer::new("frames/", 1280, 720).unwrap());
```

Writes `frames/frame_000000.png`, `frames/frame_000001.png`, ... using
`tiny-skia` for high-quality anti-aliased rasterisation.  A helper script
`frames/encode_with_ffmpeg.sh` is generated that shows how to assemble the
frames into a video:

```sh
ffmpeg -y -framerate 60 -i frame_%06d.png -c:v libx264 -pix_fmt yuv420p -crf 18 ../output.mp4
```

### `GifRenderer` (animated GIF, pure Rust, no system deps)

```rust
let renderer = Box::new(GifRenderer::new("out.gif", 640, 360, 30).unwrap());
```

Produces an animated `.gif` using the `gif` crate.  No system dependencies
whatsoever — works out of the box on any platform.  Ideal for short loops,
demos, and README badges.  GIF's 256-colour-per-frame palette means
gradients and photographs don't look great; for those use `video`.

### `VideoRenderer` (MP4 / WebM via FFmpeg)

```rust
let renderer = Box::new(VideoRenderer::new("out.mp4", 1280, 720, 60).unwrap());
```

Produces an `.mp4` (H.264) or `.webm` (VP8) file via `ffmpeg-next`.
Requires FFmpeg shared libraries on the system:

* Debian/Ubuntu: `sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev`
* macOS (Homebrew): `brew install ffmpeg`
* Arch: `sudo pacman -S ffmpeg`

The output codec is selected from the file extension: `.mp4` → H.264,
`.webm` → VP8.  YUV 4:2:0 pixel format is used by default for broad
compatibility.

### Parallel PNG encoding (`parallel` feature)

When you've collected a batch of frames in memory and want to encode them
as PNGs in parallel across all CPU cores:

```rust
use cautious_carnival::parallel_encode_pngs;

let jobs: Vec<(u32, u32, std::path::PathBuf, Vec<u8>)> = vec![
    // (width, height, output_path, RGBA pixels)
    (1280, 720, "frame_000000.png".into(), pixels_for_frame_0),
    (1280, 720, "frame_000001.png".into(), pixels_for_frame_1),
    // ...
];
parallel_encode_pngs(&jobs);
```

Typical speedup is 4–8× on an 8-core machine for scenes where PNG
encoding is the bottleneck.

## Mobjects

| Mobject      | Constructor                                   | Notes                                            |
|--------------|-----------------------------------------------|--------------------------------------------------|
| `Circle`     | `Circle::new(radius)`                         |                                                  |
| `Square`     | `Square::new(side)`                           |                                                  |
| `Rectangle`  | `Rectangle::new(width, height)`               |                                                  |
| `Polygon`    | `Polygon::new(n_sides, radius)`               | Regular polygon; first vertex at angle 0          |
| `Dot`        | `Dot::new(pos)` / `.with_radius(r)`           | Small filled point                               |
| `Line`       | `Line::new(start, end)`                       | `.set_stroke_width(w)`                           |
| `Arrow`      | `Arrow::new(start, end)`                      | `.with_head_size(s)`, `.with_stroke(s)`          |
| `Text`       | `Text::new("...")`                            | `.set_font_size(s)`                              |
| `Group`      | `Group::new().add(...).add(...)`              | Composite; transforms propagate to children      |

Every `Mobject` gets the fluent setters `move_to`, `scaled`, `rotated`,
`with_color`, `with_opacity` via the `MobjectExt` trait.

```rust
let g = Group::new()
    .add(Box::new(Circle::new(0.5).with_color(Color::BLUE).move_to(Vec3::xy(-1.0, 0.0))))
    .add(Box::new(Square::new(1.0).with_color(Color::RED).move_to(Vec3::xy(1.0, 0.0))));
scene.add(Box::new(g));
```

## Animations

| Animation          | Constructor                                                  |
|--------------------|--------------------------------------------------------------|
| `FadeIn`           | `FadeIn::new(mob, duration)`                                 |
| `FadeOut`          | `FadeOut::new(duration)` / `FadeOut::of(id, duration)`       |
| `MoveTo`           | `MoveTo::new(mob, target, duration).with_easing(easing)`     |
| `Rotate`           | `Rotate::new(mob, delta_radians, duration)`                  |
| `ScaleTo`          | `ScaleTo::new(mob, target_scale, duration)`                  |
| `ColorShift`       | `ColorShift::new(mob, target_color, duration)`               |
| `Wiggle`           | `Wiggle::new(mob, amplitude, frequency, axis, duration)`     |
| `Pulse`            | `Pulse::new(mob, peak_scale, duration)`                      |
| `GrowFromCenter`   | `GrowFromCenter::new(mob, duration)`                         |
| `Transform`        | `Transform::new(from, to, duration)`                         |
| `Wait`             | `Wait::new(duration)`                                        |
| `AnimationGroup`   | `AnimationGroup::new().add(a1).add(a2)` (parallel)           |

### Easing functions

`linear`, `smooth`, `ease_in_out_cubic`, `ease_in_quad`, `ease_out_quad`,
`there_and_back`, `elastic_out`, `bounce_out`.

### Composition

```rust
// Run two animations in parallel — total duration is max(d1, d2).
scene.play_together(
    AnimationGroup::new()
        .add(Box::new(FadeIn::new(Box::new(circle), 1.0)))
        .add(Box::new(Rotate::new(square.clone_box(), std::f64::consts::PI, 2.0))),
);

// Run a sequence back-to-back.
scene.play_sequence(vec![
    Box::new(FadeIn::new(Box::new(circle), 0.5)),
    Box::new(MoveTo::new(circle.clone_box(), Vec3::xy(2.0, 0.0), 1.0)),
    Box::new(FadeOut::new(0.5)),
]);

// Shortcut for Wait.
scene.wait(0.5);
```

## Scene model

A `Scene` owns the mobject list, the renderer, and the current scene time.
Call `scene.play(anim)` to advance the timeline by one animation; the
scene samples the animation at the configured `fps` and renders one frame
per sample.

```rust
let config = SceneConfig {
    width: 1280,
    height: 720,
    fps: 60,
    background: Color::rgb(0x12, 0x14, 0x1A),
    units_per_short_edge: 8.0,
};
```

The scene origin `(0, 0)` maps to the centre of the frame; `+X` is
rightward, `+Y` is upward (screen-space Y is flipped at render time).
`units_per_short_edge` controls the zoom — `8.0` means 8 scene units fit
along the shorter screen dimension.

`Scene::add(mob)` returns an `id` you can pass to `FadeOut::of`,
`MoveTo::of`, `Rotate::of`, etc. to target specific mobjects.  Without
an explicit id, animations target the most-recently-added mobject.

## Text rendering

The SVG backend renders text natively via `<text>` elements with a
sans-serif font.

The raster / gif / video backends look for `.ttf` files in the crate's
`src/` directory at runtime (see [Font discovery](#font-discovery-auto-scan-of-src)
above).  If TTF fonts are found, they are used via `fontdue` for
high-quality anti-aliased text rendering.  If no TTF fonts are found,
the backends fall back to a built-in 5x7 ASCII bitmap font (covering
printable ASCII 0x20–0x7E) so text always renders *something*.

The bitmap font is generated from a system monospace font at build
time via `scripts/gen_font.py`.  To use real TTF fonts, drop one or
two `.ttf` files into `src/` and update the
`PRIMARY_FONT_FILENAME` / `SECONDARY_FONT_FILENAME` constants in
`lib.rs` to match.

## Examples

### Bouncing ball

```rust
use cautious_carnival::*;

let renderer = Box::new(GifRenderer::new("bounce.gif", 320, 240, 30).unwrap());
let mut scene = Scene::new(renderer, SceneConfig::default());

let ball = Circle::new(0.3).with_color(Color::RED);
scene.play(FadeIn::new(Box::new(ball.clone()), 0.3));

for _ in 0..3 {
    scene.play(
        MoveTo::new(Box::new(ball.clone()), Vec3::xy(0.0, -2.0), 0.6)
            .with_easing(linear),
    );
    scene.play(
        MoveTo::new(Box::new(ball.clone()), Vec3::xy(0.0, 0.0), 0.6)
            .with_easing(bounce_out),
    );
}
```

### Polygon morphing (fade-based)

```rust
let shapes = vec![
    Box::new(Polygon::new(3, 1.0).with_color(Color::BLUE)) as Box<dyn Mobject>,
    Box::new(Polygon::new(4, 1.0).with_color(Color::BLUE)),
    Box::new(Polygon::new(5, 1.0).with_color(Color::BLUE)),
    Box::new(Polygon::new(6, 1.0).with_color(Color::BLUE)),
];

for next in shapes {
    scene.play(FadeIn::new(next.clone_box(), 0.5));
    scene.play(Wait::new(0.3));
    scene.play(FadeOut::new(0.4));
}
```

### Rotating star

```rust
let star = Polygon::new(5, 1.0).with_color(Color::YELLOW);
scene.play(FadeIn::new(Box::new(star.clone()), 0.5));
scene.play(Rotate::new(star.clone_box(), std::f64::consts::TAU, 3.0)
    .with_easing(linear));
```

## Tests

```sh
cargo test                       # core tests (SVG only)
cargo test --features gif        # + GIF round-trip test
cargo test --features raster     # + PNG sequence test
cargo test --features video      # + MP4 test (skips if ffmpeg not on PATH)
cargo test --all-features        # everything
```

## License

GPL-3.0.

## Acknowledgements

Inspired by [Manim](https://www.manim.community/) — the Python math
animation engine by 3Blue1Brown.
