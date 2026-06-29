//! `hello_circle` — an end-to-end demo of `cautious-carnival`.
//!
//! Run with the backend of your choice:
//!
//! ```sh
//! # SVG storyboard (default — one .svg file with every frame stacked)
//! cargo run --example hello_circle
//!
//! # Animated GIF (pure Rust, no system deps)
//! cargo run --example hello_circle --features gif
//!
//! # MP4 video (requires FFmpeg shared libs on the system)
//! cargo run --example hello_circle --features video
//!
//! # PNG frame sequence (rasterise with tiny-skia, encode later)
//! cargo run --example hello_circle --features raster
//!
//! # Add text-to-speech narration (requires `espeak-ng` on PATH)
//! cargo run --example hello_circle --features "video tts"
//!
//! # Optimised release build
//! cargo run --example hello_circle --release --features gif
//! ```
//!
//! When more than one backend feature is enabled, the example prefers
//! `video`, then `gif`, then `raster`, then `svg`.
//!
//! The total animation length is the sum of every `play()` call's
//! duration: 0.8 + 0.8 + 1.2 + 2.0 + 1.5 + 0.8 + 0.5 + 0.6 = 8.2 s
//! (plus any voiceover clips added when the `tts` feature is on).

use cautious_carnival::{
    ease_in_out_cubic, there_and_back, AnimationGroup, Circle, Color, FadeIn, FadeOut,
    GrowFromCenter, Mobject, MobjectExt, MoveTo, Polygon, Pulse, Rotate, Scene, SceneConfig, Text,
    Vec3, Wait,
};

#[cfg(feature = "gif")]
use cautious_carnival::GifRenderer;
#[cfg(feature = "raster")]
use cautious_carnival::RasterRenderer;
#[cfg(feature = "video")]
use cautious_carnival::VideoRenderer;

// Voiceover support is only pulled in when the `tts` feature is on.
// The example still runs without it — the scene is just silent.
#[cfg(feature = "tts")]
use cautious_carnival::EspeakNgEngine;

fn build_scene(scene: &mut Scene) {
    // Optional: narrate the demo with text-to-speech.  Each call to
    // `add_voiceover` synthesises a WAV file via `espeak-ng`, records
    // it on the scene's voiceover track, and advances the scene clock
    // by the audio's duration so subsequent animations are timed
    // against the narration.
    #[cfg(feature = "tts")]
    let engine = EspeakNgEngine::new().with_voice("en-us");

    // 1. A title that fades in at the top of the frame.
    let mut title = Text::new("hello, cautious-carnival")
        .with_color(Color::YELLOW)
        .move_to(Vec3::xy(0.0, 3.0));
    title.set_font_size(36.0);
    scene.play(FadeIn::new(Box::new(title.clone()), 0.8));

    #[cfg(feature = "tts")]
    {
        let _ = scene.add_voiceover(
            &engine,
            "Hello! This is cautious-carnival, a programmable animation engine.",
            "hello_circle_audio",
        );
    }

    // 2. A blue circle fades in at the origin.
    let circle = Circle::new(1.0).with_color(Color::BLUE).move_to(Vec3::ZERO);
    scene.play(FadeIn::new(Box::new(circle.clone()), 0.8));

    #[cfg(feature = "tts")]
    {
        let _ = scene.add_voiceover(
            &engine,
            "Here is a blue circle, fading in at the origin.",
            "hello_circle_audio",
        );
    }

    // 3. Slide it to the right with a smooth ease-in-out.
    scene.play(
        MoveTo::new(Box::new(circle.clone()), Vec3::xy(3.0, 0.0), 1.2)
            .with_easing(ease_in_out_cubic),
    );

    // 4. Move it back through the origin to the left — a single `MoveTo`
    //    whose easing is `there_and_back` produces a clean round trip.
    scene.play(
        MoveTo::new(Box::new(circle.clone()), Vec3::xy(-3.0, 0.0), 2.0).with_easing(there_and_back),
    );

    #[cfg(feature = "tts")]
    {
        let _ = scene.add_voiceover(
            &engine,
            "Now it slides to the right, and back through the origin to the left.",
            "hello_circle_audio",
        );
    }

    // 5. A triangle grows from the centre while the circle rotates one
    //    full turn — both animations run in parallel via AnimationGroup.
    let triangle = Polygon::new(3, 1.0)
        .with_color(Color::PURPLE)
        .move_to(Vec3::xy(0.0, -2.5));
    scene.play(
        AnimationGroup::new()
            .add(Box::new(GrowFromCenter::new(Box::new(triangle), 1.5)))
            .add(Box::new(Rotate::new(
                circle.clone_box(),
                std::f64::consts::TAU,
                1.5,
            ))),
    );

    #[cfg(feature = "tts")]
    {
        let _ = scene.add_voiceover(
            &engine,
            "A triangle grows from the centre while the circle rotates a full turn.",
            "hello_circle_audio",
        );
    }

    // 6. Bring the circle home to the centre.
    scene.play(MoveTo::new(Box::new(circle.clone()), Vec3::ZERO, 0.8));

    // 7. A quick pulse to draw the eye, then a short hold.
    scene.play(Pulse::new(circle.clone_box(), 1.3, 0.5));
    scene.play(Wait::new(0.5));

    // 8. Fade the circle out.  The title remains on screen as a final card.
    scene.play(FadeOut::new(0.6));

    #[cfg(feature = "tts")]
    {
        let _ = scene.add_voiceover(
            &engine,
            "And that's the end of the demo. Thanks for watching!",
            "hello_circle_audio",
        );
    }
}

// ---------------------------------------------------------------------------
// Backend selection — picks whichever renderer is enabled at compile time.
// ---------------------------------------------------------------------------

#[allow(unreachable_code)]
fn main() {
    let config = SceneConfig {
        background: Color::rgb(0x12, 0x14, 0x1A), // soft dark navy
        ..SceneConfig::default()
    };

    // The example picks the "best" backend enabled at compile time.  When
    // more than one is enabled, video > gif > raster > svg.
    #[cfg(feature = "video")]
    {
        let renderer = Box::new(
            VideoRenderer::new("hello_circle.mp4", config.width, config.height, config.fps)
                .expect("failed to initialise video renderer"),
        );
        let mut scene = Scene::new(renderer, config.clone());
        build_scene(&mut scene);

        // If the `tts` feature is on, concatenate the voiceover track
        // into a single WAV and mux it into the silent video.
        //
        // We must drop `scene` *before* muxing so the `VideoRenderer`
        // finalises the silent MP4 (its `Drop` impl calls
        // `Renderer::finish`, which closes the ffmpeg pipe and writes
        // the MP4 trailer).  Capture the track first, then drop.
        #[cfg(feature = "tts")]
        let voiceover_track = scene.voiceover_track().clone();
        drop(scene);

        #[cfg(feature = "tts")]
        {
            if !voiceover_track.is_empty() {
                let audio_wav = std::path::Path::new("hello_circle_audio.wav");
                match voiceover_track.concatenate_into_wav(audio_wav) {
                    Ok(d) => println!("concatenated voiceover: {:.2}s", d.0),
                    Err(e) => eprintln!("voiceover concat failed: {e}"),
                }
                let final_mp4 = std::path::Path::new("hello_circle_narrated.mp4");
                match cautious_carnival::mux_audio_video(
                    std::path::Path::new("hello_circle.mp4"),
                    audio_wav,
                    final_mp4,
                ) {
                    Ok(()) => println!("wrote {} (video + narration)", final_mp4.display()),
                    Err(e) => eprintln!("audio mux failed: {e}"),
                }
            }
        }

        println!("wrote hello_circle.mp4 (~8.2s of animation)");
        return;
    }

    #[cfg(feature = "gif")]
    {
        // GIFs benefit from a smaller frame size to keep file size sane.
        let renderer = Box::new(
            GifRenderer::new("hello_circle.gif", 640, 360, 30)
                .expect("failed to initialise gif renderer"),
        );
        let mut gif_config = config.clone();
        gif_config.width = 640;
        gif_config.height = 360;
        gif_config.fps = 30;
        let mut scene = Scene::new(renderer, gif_config);
        build_scene(&mut scene);
        println!("wrote hello_circle.gif (~8.2s of animation)");
        return;
    }

    #[cfg(feature = "raster")]
    {
        let renderer = Box::new(
            RasterRenderer::new("hello_circle_frames", config.width, config.height)
                .expect("failed to initialise raster renderer"),
        );
        let mut scene = Scene::new(renderer, config.clone());
        build_scene(&mut scene);
        println!("wrote PNG frames to hello_circle_frames/ (~8.2s of animation)");
        println!("  assemble with: ffmpeg -framerate 60 -i hello_circle_frames/frame_%06d.png -c:v libx264 -pix_fmt yuv420p hello_circle.mp4");
        return;
    }

    // Fallback: SVG storyboard.
    let renderer = Box::new(cautious_carnival::SvgRenderer::new(
        "hello_circle.svg",
        config.width,
        config.height,
    ));
    let mut scene = Scene::new(renderer, config);
    build_scene(&mut scene);
    println!("wrote hello_circle.svg (~8.2s of animation)");
    println!(
        "  (re-run with `--features gif` for an animated GIF, or `--features video` for an MP4)"
    );
    #[cfg(not(feature = "tts"))]
    println!("  (re-run with `--features tts` to add text-to-speech narration)");
}
