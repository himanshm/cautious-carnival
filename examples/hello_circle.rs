//! `hello_circle` — a minimal end-to-end demo of `rustimate`.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example hello_circle
//! ```
//!
//! Produces `hello_circle.svg` in the current working directory.  Each
//! animation frame is written as a stacked `<g>` element inside the SVG, so
//! opening the file in a browser shows every frame layered on top of each
//! other — a handy storyboard view.

use cautious_carnival::{
    ease_in_out_cubic, there_and_back, Circle, Color, FadeIn, FadeOut, MobjectExt, MoveTo, Scene,
    SceneConfig, SvgRenderer, Text, Vec3, Wait,
};

fn main() {
    // 1. Set up the renderer + scene.
    //
    // 1280×720 at 60 fps is the default `SceneConfig`.  The scene origin
    // (0, 0) maps to the centre of the frame; +X is rightward, +Y is upward.
    let renderer = Box::new(SvgRenderer::new("hello_circle.svg", 1280, 720));
    let config = SceneConfig {
        background: Color::rgb(0x12, 0x14, 0x1A), // soft dark navy
        ..SceneConfig::default()
    };
    let mut scene = Scene::new(renderer, config);

    // 2. A title that fades in at the top of the frame.
    let mut title = Text::new("hello, rustimate")
        .with_color(Color::YELLOW)
        .move_to(Vec3::xy(0.0, 3.0));
    title.set_font_size(48.0);
    scene.play(FadeIn::new(Box::new(title.clone()), 0.8));

    // 3. A blue circle fades in at the origin.
    let circle = Circle::new(1.0).with_color(Color::BLUE).move_to(Vec3::ZERO);
    scene.play(FadeIn::new(Box::new(circle.clone()), 0.8));

    // 4. Slide it to the right with a smooth ease-in-out.
    scene.play(
        MoveTo::new(Box::new(circle.clone()), Vec3::xy(3.0, 0.0), 1.2)
            .with_easing(ease_in_out_cubic),
    );

    // 5. Move it back through the origin to the left — a single `MoveTo`
    //    whose easing is `there_and_back` produces a clean round trip.
    scene.play(
        MoveTo::new(Box::new(circle.clone()), Vec3::xy(-3.0, 0.0), 2.0).with_easing(there_and_back),
    );

    // 6. Bring it home to the centre.
    scene.play(MoveTo::new(Box::new(circle.clone()), Vec3::ZERO, 0.8));

    // 7. Hold for a beat, then fade the circle out.  The title remains on
    //    screen as a final card.  (To fade the title too, capture its id
    //    from `scene.add(...)` and call `scene.play(FadeOut::of(id, 0.6))`.)
    scene.play(Wait::new(0.5));
    scene.play(FadeOut::new(0.6));

    // 8. Drop the scene — its `Drop` impl closes the SVG file.  The total
    //    animation length is the sum of every `play()` call's duration:
    //    0.8 + 0.8 + 1.2 + 2.0 + 0.8 + 0.5 + 0.6 = 6.7s.
    drop(scene);

    println!("wrote hello_circle.svg (~6.7s of animation)");
}
