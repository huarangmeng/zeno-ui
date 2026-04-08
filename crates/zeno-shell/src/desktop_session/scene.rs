use zeno_core::{Color, Rect};
use zeno_graphics::{Brush, DrawCommand, Scene, SceneSubmit, Shape};

pub(super) fn patch_stats(submit: &SceneSubmit) -> (usize, usize) {
    match submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0),
        SceneSubmit::Patch { patch, .. } => (patch.upserts.len(), patch.removes.len()),
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
pub(super) fn partial_scene_for_dirty_bounds(scene: &Scene, dirty_bounds: Rect) -> Scene {
    let mut commands = Vec::new();
    commands.push(DrawCommand::Fill {
        shape: Shape::Rect(dirty_bounds),
        brush: Brush::Solid(clear_color_for_scene(scene)),
    });
    for block in &scene.blocks {
        if block.bounds.intersects(&dirty_bounds) {
            commands.extend(block.commands.iter().cloned());
        }
    }
    Scene {
        size: scene.size,
        commands,
        blocks: scene
            .blocks
            .iter()
            .filter(|block| block.bounds.intersects(&dirty_bounds))
            .cloned()
            .collect(),
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn clear_color_for_scene(scene: &Scene) -> Color {
    scene
        .commands
        .iter()
        .find_map(|cmd| match cmd {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        })
        .unwrap_or(Color::TRANSPARENT)
}
