use cautious_carnival::{Animation, Circle, Scene, Square, Text};
use glam::Vec2;
use peniko::Color;

fn main() -> anyhow::Result<()> {
    let mut scene = Scene::new(1920, 1080, 60);

    let circle = scene.add(Box::new(
        Circle::new(1.5).with_color(Color::new([0.204, 0.596, 0.859, 1.0])),
    ));
    let title = scene.add(Box::new(Text::new("Cautious Carnival", 0.8)));
    let subtitle = scene.add(Box::new(
        Text::new("A Manim replacement in Rust").with_color(Color::new([0.74, 0.76, 0.78, 1.0])),
    ));

    scene.play(Animation::move_to(title, Vec2::new(0.0, 3.0), 0.0));
    scene.play(Animation::move_to(subtitle, Vec2::new(0.0, 2.2), 0.0));
    scene.play(Animation::fade_in(title, 1.0));
    scene.play(Animation::fade_in(subtitle, 1.0));
    scene.play(Animation::move_to(circle, Vec2::new(4.0, -1.0), 3.0));
    scene.play(Animation::fade_out(title, 1.0));

    scene.render_to_file("carnival_demo.mp4")?;
    Ok(())
}
