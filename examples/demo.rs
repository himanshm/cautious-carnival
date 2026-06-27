use cautious_carnival::{Circle, Scene, Square};
use tiny_skia::Color;

fn main() -> anyhow::Result<()> {
    // 1. Initialize Scene (1920x1080 at 60 FPS)
    let mut scene = Scene::new(1920, 1080, 60);

    // 2. Create Mobjects
    let circle = Circle::new(1.5).with_color(Color::from_rgba8(52, 152, 219, 255)); // Blue

    let square = Square::new(2.0); // Default Red

    // 3. Add to Scene
    scene.add(Box::new(circle));
    scene.add(Box::new(square));

    // 4. Render to MP4
    // Note: Requires FFmpeg to be installed on your system PATH.
    scene.render_to_file("carnival_demo.mp4")?;

    Ok(())
}
