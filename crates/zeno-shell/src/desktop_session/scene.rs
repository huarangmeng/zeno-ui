use zeno_core::{Color, Rect};
use zeno_graphics::{Brush, DrawCommand, Scene, SceneBlock, SceneSubmit, SceneTransform, Shape};

pub(super) fn patch_stats(submit: &SceneSubmit) -> (usize, usize) {
    match submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0),
        SceneSubmit::Patch { patch, .. } => (patch.upserts.len(), patch.removes.len()),
    }
}

pub(super) fn default_clear_color(transparent: bool) -> Color {
    if transparent {
        Color::TRANSPARENT
    } else {
        Color::WHITE
    }
}

pub(super) fn ensure_clear_command(scene: &Scene, fallback: Color) -> Scene {
    if scene.clear_color.is_some()
        || scene
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Clear(_)))
    {
        return scene.clone();
    }
    Scene {
        size: scene.size,
        clear_color: Some(fallback),
        commands: scene.commands.clone(),
        layers: scene.layers.clone(),
        blocks: scene.blocks.clone(),
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
pub(super) fn partial_scene_for_dirty_bounds(scene: &Scene, dirty_bounds: Rect) -> Scene {
    if scene.layers.len() > 1 {
        return scene.clone();
    }
    let mut blocks = vec![SceneBlock::new(
        u64::MAX,
        Scene::ROOT_LAYER_ID,
        0,
        dirty_bounds,
        SceneTransform::identity(),
        None,
        vec![DrawCommand::Fill {
            shape: Shape::Rect(dirty_bounds),
            brush: Brush::Solid(clear_color_for_scene(scene)),
        }],
    )];
    blocks.extend(
        scene
            .blocks
            .iter()
            .filter(|block| block.bounds.intersects(&dirty_bounds))
            .cloned()
            .enumerate()
            .map(|(index, mut block)| {
                block.order = index as u32 + 1;
                block
            }),
    );
    Scene::from_layers_and_blocks(scene.size, None, scene.layers.clone(), blocks)
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn clear_color_for_scene(scene: &Scene) -> Color {
    scene
        .clear_color
        .or_else(|| scene.commands.iter().find_map(|cmd| match cmd {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        }))
        .unwrap_or(Color::TRANSPARENT)
}

#[cfg(test)]
mod tests {
    use super::{default_clear_color, ensure_clear_command};
    use zeno_core::{Color, Rect, Size};
    use zeno_graphics::{Brush, DrawCommand, Scene, SceneBlock, SceneTransform, Shape};

    #[test]
    fn opaque_windows_default_to_white_clear() {
        assert_eq!(default_clear_color(false), Color::WHITE);
        assert_eq!(default_clear_color(true), Color::TRANSPARENT);
    }

    #[test]
    fn ensure_clear_command_prepends_fallback_clear_once() {
        let scene = Scene::from_blocks(
            Size::new(200.0, 100.0),
            None,
            vec![SceneBlock::new(
                1,
                Scene::ROOT_LAYER_ID,
                0,
                Rect::new(0.0, 0.0, 50.0, 50.0),
                SceneTransform::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 50.0, 50.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
        );
        let prepared = ensure_clear_command(&scene, Color::rgba(10, 20, 30, 255));
        assert_eq!(prepared.clear_color, Some(Color::rgba(10, 20, 30, 255)));
        assert_eq!(prepared.commands.len(), scene.commands.len());
        assert_eq!(prepared.blocks, scene.blocks);
        let prepared_again = ensure_clear_command(&prepared, Color::WHITE);
        assert_eq!(prepared_again, prepared);
    }
}
