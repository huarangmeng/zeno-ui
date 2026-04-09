#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use std::collections::{HashMap, HashSet};

use zeno_core::{Color, Rect};
use zeno_scene::{
    Brush, DrawCommand, Scene, SceneBlock, SceneLayer, SceneSubmit, SceneTransform, Shape,
};

pub(super) fn patch_stats(submit: &SceneSubmit) -> (usize, usize) {
    match submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0),
        SceneSubmit::Patch { patch, .. } => (
            patch.upserts.len()
                + patch.reorders.len()
                + patch.layer_upserts.len()
                + patch.layer_reorders.len(),
            patch.removes.len() + patch.layer_removes.len(),
        ),
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
    if scene.clear_color.is_some() || scene.clear_command().is_some() {
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
    let layers = partial_layers_for_dirty_bounds(scene, dirty_bounds);
    let included_layer_ids: HashSet<u64> = layers.iter().map(|layer| layer.layer_id).collect();
    let mut commands = scene.commands.clone();
    let clear_commands = vec![DrawCommand::Fill {
        shape: Shape::Rect(dirty_bounds),
        brush: Brush::Solid(clear_color_for_scene(scene)),
    }];
    let clear_start = commands.len();
    commands.extend_from_slice(&clear_commands);
    let mut blocks = vec![SceneBlock::from_commands(
        u64::MAX,
        Scene::ROOT_LAYER_ID,
        0,
        dirty_bounds,
        SceneTransform::identity(),
        None,
        clear_commands.clone(),
    )
    .with_normalized_commands(zeno_scene::CommandRange {
        start: clear_start,
        len: clear_commands.len(),
    })];
    blocks.extend(
        scene
            .blocks
            .iter()
            .filter(|block| {
                partial_scene_should_include_block(scene, block, &included_layer_ids, dirty_bounds)
            })
            .cloned()
            .enumerate()
            .map(|(index, mut block)| {
                block.order = index as u32 + 1;
                block
            }),
    );
    Scene::from_layers_and_blocks_with_commands(scene.size, None, layers, commands, blocks)
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn partial_layers_for_dirty_bounds(scene: &Scene, dirty_bounds: Rect) -> Vec<SceneLayer> {
    let parents: HashMap<u64, Option<u64>> = scene
        .layers
        .iter()
        .map(|layer| (layer.layer_id, layer.parent_layer_id))
        .collect();
    let mut included = HashSet::from([Scene::ROOT_LAYER_ID]);
    for layer_id in scene
        .layers
        .iter()
        .filter_map(|layer| {
            scene
                .effective_bounds_for_layer(layer.layer_id)
                .filter(|bounds| bounds.intersects(&dirty_bounds))
                .map(|_| layer.layer_id)
        })
        .chain(scene.blocks.iter().filter_map(|block| {
            scene
                .effective_bounds_for_block(block)
                .filter(|bounds| bounds.intersects(&dirty_bounds))
                .map(|_| block.layer_id)
        }))
    {
        include_layer_ancestors(layer_id, &parents, &mut included);
    }
    scene
        .layers
        .iter()
        .filter(|layer| included.contains(&layer.layer_id))
        .cloned()
        .collect()
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn partial_scene_should_include_block(
    scene: &Scene,
    block: &SceneBlock,
    included_layer_ids: &HashSet<u64>,
    dirty_bounds: Rect,
) -> bool {
    if !included_layer_ids.contains(&block.layer_id) {
        return false;
    }
    let block_intersects = matches!(
        scene.effective_bounds_for_block(block),
        Some(bounds) if bounds.intersects(&dirty_bounds)
    );
    let layer_intersects = matches!(
        scene.effective_bounds_for_layer(block.layer_id),
        Some(bounds) if bounds.intersects(&dirty_bounds)
    );
    block_intersects
        || (layer_intersects && layer_chain_requires_full_block_replay(scene, block.layer_id))
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn layer_chain_requires_full_block_replay(scene: &Scene, mut layer_id: u64) -> bool {
    loop {
        let Some(layer) = scene.layers.iter().find(|layer| layer.layer_id == layer_id) else {
            return false;
        };
        if layer_requires_full_block_replay(layer) {
            return true;
        }
        let Some(parent_id) = layer.parent_layer_id else {
            return false;
        };
        layer_id = parent_id;
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn layer_requires_full_block_replay(layer: &SceneLayer) -> bool {
    layer.offscreen
        || layer.clip.is_some()
        || layer.opacity < 1.0
        || layer.blend_mode != zeno_scene::SceneBlendMode::Normal
        || !layer.effects.is_empty()
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn include_layer_ancestors(
    mut layer_id: u64,
    parents: &HashMap<u64, Option<u64>>,
    included: &mut HashSet<u64>,
) {
    loop {
        if !included.insert(layer_id) {
            break;
        }
        let Some(Some(parent_id)) = parents.get(&layer_id).copied() else {
            break;
        };
        layer_id = parent_id;
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
fn clear_color_for_scene(scene: &Scene) -> Color {
    scene
        .clear_color
        .or_else(|| scene.clear_command())
        .unwrap_or(Color::TRANSPARENT)
}

#[cfg(test)]
mod tests {
    use super::{default_clear_color, ensure_clear_command};
    use zeno_core::{Color, Rect, Size};
    use zeno_scene::{
        Brush, DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneClip, SceneLayer, SceneSubmit,
        SceneTransform, Shape,
    };

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
        assert_eq!(prepared.command_count(), scene.command_count());
        assert_eq!(prepared.blocks, scene.blocks);
        let prepared_again = ensure_clear_command(&prepared, Color::WHITE);
        assert_eq!(prepared_again, prepared);
    }

    #[test]
    fn patch_stats_report_full_and_patch_counts() {
        let scene = Scene::from_blocks(Size::new(10.0, 10.0), None, Vec::new());
        let full = super::patch_stats(&SceneSubmit::Full(scene.clone()));
        let patch = super::patch_stats(&SceneSubmit::Patch {
            patch: zeno_scene::ScenePatch {
                size: scene.size,
                commands: Vec::new(),
                base_layer_count: scene.layers.len(),
                base_block_count: scene.blocks.len(),
                layer_upserts: Vec::new(),
                layer_reorders: Vec::new(),
                layer_removes: Vec::new(),
                upserts: vec![
                    SceneBlock::new(
                        1,
                        Scene::ROOT_LAYER_ID,
                        0,
                        Rect::new(0.0, 0.0, 5.0, 5.0),
                        SceneTransform::identity(),
                        None,
                        Vec::new(),
                    ),
                    SceneBlock::new(
                        2,
                        Scene::ROOT_LAYER_ID,
                        1,
                        Rect::new(5.0, 0.0, 5.0, 5.0),
                        SceneTransform::identity(),
                        None,
                        Vec::new(),
                    ),
                ],
                reorders: Vec::new(),
                removes: vec![3],
            },
            current: scene,
        });

        assert_eq!(full, (0, 0));
        assert_eq!(patch, (2, 1));
    }

    #[cfg(all(target_os = "macos", feature = "desktop_winit"))]
    #[test]
    fn partial_scene_keeps_only_dirty_layers_and_blocks() {
        let size = Size::new(200.0, 100.0);
        let child_layer = SceneLayer::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(20.0, 10.0, 80.0, 40.0),
            Rect::new(20.0, 10.0, 80.0, 40.0),
            SceneTransform::identity(),
            None,
            1.0,
            SceneBlendMode::Normal,
            Vec::new(),
            false,
        );
        let untouched_layer = SceneLayer::new(
            20,
            20,
            Some(Scene::ROOT_LAYER_ID),
            2,
            Rect::new(120.0, 10.0, 60.0, 40.0),
            Rect::new(120.0, 10.0, 60.0, 40.0),
            SceneTransform::identity(),
            None,
            1.0,
            SceneBlendMode::Normal,
            Vec::new(),
            false,
        );
        let dirty_block = SceneBlock::new(
            101,
            10,
            0,
            Rect::new(25.0, 12.0, 20.0, 20.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(25.0, 12.0, 20.0, 20.0)),
                brush: Brush::Solid(Color::rgba(10, 20, 30, 255)),
            }],
        );
        let untouched_block = SceneBlock::new(
            202,
            20,
            1,
            Rect::new(130.0, 12.0, 20.0, 20.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(130.0, 12.0, 20.0, 20.0)),
                brush: Brush::Solid(Color::rgba(200, 210, 220, 255)),
            }],
        );
        let scene = Scene::from_layers_and_blocks(
            size,
            Some(Color::WHITE),
            vec![SceneLayer::root(size), child_layer.clone(), untouched_layer],
            vec![dirty_block, untouched_block],
        );

        let partial =
            super::partial_scene_for_dirty_bounds(&scene, Rect::new(20.0, 10.0, 40.0, 30.0));

        assert_eq!(partial.layers.len(), 2);
        assert!(
            partial
                .layers
                .iter()
                .any(|layer| layer.layer_id == Scene::ROOT_LAYER_ID)
        );
        assert!(
            partial
                .layers
                .iter()
                .any(|layer| layer.layer_id == child_layer.layer_id)
        );
        assert_eq!(partial.blocks.len(), 2);
        assert_eq!(partial.blocks[0].node_id, u64::MAX);
        assert_eq!(partial.blocks[1].node_id, 101);
        assert_eq!(partial.blocks[1].layer_id, child_layer.layer_id);
    }

    #[cfg(all(target_os = "macos", feature = "desktop_winit"))]
    #[test]
    fn partial_scene_replays_non_intersecting_blocks_for_offscreen_layers() {
        let size = Size::new(240.0, 120.0);
        let offscreen_layer = SceneLayer::new(
            30,
            30,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(20.0, 10.0, 120.0, 60.0),
            Rect::new(20.0, 10.0, 120.0, 60.0),
            SceneTransform::identity(),
            None,
            0.8,
            SceneBlendMode::Normal,
            Vec::new(),
            true,
        );
        let leading_block = SceneBlock::new(
            301,
            30,
            0,
            Rect::new(24.0, 14.0, 24.0, 24.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(24.0, 14.0, 24.0, 24.0)),
                brush: Brush::Solid(Color::rgba(10, 20, 30, 255)),
            }],
        );
        let dirty_tail_block = SceneBlock::new(
            302,
            30,
            1,
            Rect::new(110.0, 18.0, 20.0, 20.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(110.0, 18.0, 20.0, 20.0)),
                brush: Brush::Solid(Color::rgba(220, 80, 60, 255)),
            }],
        );
        let scene = Scene::from_layers_and_blocks(
            size,
            Some(Color::WHITE),
            vec![SceneLayer::root(size), offscreen_layer.clone()],
            vec![leading_block, dirty_tail_block],
        );

        let partial =
            super::partial_scene_for_dirty_bounds(&scene, Rect::new(108.0, 16.0, 24.0, 24.0));
        let partial_ids: Vec<u64> = partial.blocks.iter().map(|block| block.node_id).collect();

        assert!(
            partial
                .layers
                .iter()
                .any(|layer| layer.layer_id == offscreen_layer.layer_id)
        );
        assert!(partial_ids.contains(&301));
        assert!(partial_ids.contains(&302));
    }

    #[cfg(all(target_os = "macos", feature = "desktop_winit"))]
    #[test]
    fn partial_scene_replays_clipped_descendants_when_ancestor_clip_is_dirty() {
        let size = Size::new(240.0, 160.0);
        let clipped_layer = SceneLayer::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, 120.0, 120.0),
            Rect::new(20.0, 20.0, 120.0, 120.0),
            SceneTransform::translation(20.0, 20.0),
            Some(SceneClip::Rect(Rect::new(0.0, 0.0, 60.0, 60.0))),
            1.0,
            SceneBlendMode::Normal,
            Vec::new(),
            false,
        );
        let nested_layer = SceneLayer::new(
            20,
            20,
            Some(10),
            2,
            Rect::new(0.0, 0.0, 80.0, 80.0),
            Rect::new(40.0, 40.0, 80.0, 80.0),
            SceneTransform::translation(20.0, 20.0),
            None,
            1.0,
            SceneBlendMode::Normal,
            Vec::new(),
            false,
        );
        let leading_block = SceneBlock::new(
            401,
            20,
            0,
            Rect::new(42.0, 42.0, 12.0, 12.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(42.0, 42.0, 12.0, 12.0)),
                brush: Brush::Solid(Color::WHITE),
            }],
        );
        let clipped_block = SceneBlock::new(
            402,
            20,
            1,
            Rect::new(88.0, 88.0, 24.0, 24.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(88.0, 88.0, 24.0, 24.0)),
                brush: Brush::Solid(Color::rgba(200, 20, 30, 255)),
            }],
        );
        let scene = Scene::from_layers_and_blocks(
            size,
            Some(Color::WHITE),
            vec![SceneLayer::root(size), clipped_layer, nested_layer],
            vec![leading_block, clipped_block],
        );

        let partial =
            super::partial_scene_for_dirty_bounds(&scene, Rect::new(70.0, 70.0, 20.0, 20.0));
        let partial_ids: Vec<u64> = partial.blocks.iter().map(|block| block.node_id).collect();

        assert!(partial_ids.contains(&401));
        assert!(partial_ids.contains(&402));
    }
}
