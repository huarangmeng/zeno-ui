mod invalidation;
mod layout;
mod modifier;
mod node;
mod render;
mod style;
mod tree;
mod widgets;

pub use invalidation::{DirtyFlags, DirtyReason};
pub use modifier::{BlendMode, ClipMode, DropShadow, Modifier, Modifiers, TransformOrigin};
pub use node::NodeId;
pub use node::{Node, NodeKind, SpacerNode, TextNode};
pub use render::{
    ComposeEngine, ComposeRenderer, ComposeStats, compose_scene, dump_layout, dump_scene,
};
pub use style::{Axis, EdgeInsets, Style};
pub use widgets::{column, container, row, spacer, text};

#[cfg(test)]
mod tests {
    use super::{
        BlendMode, ComposeEngine, DirtyReason, EdgeInsets, Modifier, column, compose_scene,
        container, dump_layout, dump_scene, row, spacer, text,
    };
    use zeno_core::{Color, Size, Transform2D};
    use zeno_graphics::{DrawCommand, Scene, SceneBlendMode, SceneClip, SceneEffect, SceneSubmit};
    use zeno_text::FallbackTextSystem;

    #[test]
    fn builds_scene_from_column_tree() {
        let root = column(vec![
            text("Hello").font_size(20.0),
            spacer(0.0, 8.0),
            text("World"),
        ])
        .padding_all(12.0)
        .spacing(6.0)
        .background(Color::rgba(245, 247, 250, 255));

        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert_eq!(scene.commands.len(), 3);
        assert!(matches!(scene.commands[0], DrawCommand::Fill { .. }));
        assert!(matches!(scene.commands[1], DrawCommand::Text { .. }));
        assert!(matches!(scene.commands[2], DrawCommand::Text { .. }));
    }

    #[test]
    fn builds_scene_from_nested_container_and_row() {
        let root = container(
            row(vec![text("A"), spacer(12.0, 0.0), text("B")])
                .spacing(8.0)
                .foreground(Color::WHITE),
        )
        .padding_all(16.0)
        .background(Color::rgba(39, 110, 241, 255))
        .corner_radius(18.0);

        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert_eq!(scene.commands.len(), 3);
        assert!(matches!(
            scene.commands[0],
            DrawCommand::Fill {
                shape: zeno_graphics::Shape::RoundedRect { .. },
                ..
            }
        ));
    }

    #[test]
    fn compose_engine_reuses_retained_scene_until_invalidated() {
        let root = column(vec![text("Cache"), text("Hit")]).spacing(4.0);
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let first = engine.compose(&root, Size::new(320.0, 240.0));
        let second = engine.compose(&root, Size::new(320.0, 240.0));

        assert_eq!(first, second);
        assert_eq!(engine.stats().compose_passes, 1);
        assert_eq!(engine.stats().layout_passes, 1);
        assert_eq!(engine.stats().cache_hits, 1);

        engine.invalidate(DirtyReason::Paint);
        let third = engine.compose(&root, Size::new(320.0, 240.0));

        assert_eq!(third.commands.len(), second.commands.len());
        assert_eq!(engine.stats().compose_passes, 2);
        assert_eq!(engine.stats().layout_passes, 1);
        assert_eq!(engine.stats().cache_hits, 1);
    }

    #[test]
    fn compose_engine_can_repaint_single_dirty_node_without_layout() {
        let title = text("Title").foreground(Color::WHITE);
        let title_id = title.id();
        let root = column(vec![title, text("Body")])
            .spacing(4.0)
            .background(Color::rgba(39, 110, 241, 255));
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let baseline = engine.compose(&root, Size::new(320.0, 240.0));
        engine.invalidate_node(title_id, DirtyReason::Paint);
        let repainted = engine.compose(&root, Size::new(320.0, 240.0));

        assert_eq!(baseline.commands.len(), repainted.commands.len());
        assert_eq!(engine.stats().layout_passes, 1);
        assert_eq!(engine.stats().compose_passes, 2);
    }

    #[test]
    fn keyed_nodes_keep_stable_ids_across_rebuilds() {
        let first = text("Label").key("title");
        let second = text("Label").key("title");
        let third = text("Label").key("body");

        assert_eq!(first.id(), second.id());
        assert_ne!(first.id(), third.id());
    }

    #[test]
    fn compose_submit_returns_full_scene_when_paint_invalidation_keeps_commands_identical() {
        let title = text("Title").key("title");
        let title_id = title.id();
        let root = column(vec![title, text("Body").key("body")]).spacing(4.0);
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&root, Size::new(320.0, 240.0));
        engine.invalidate_node(title_id, DirtyReason::Paint);
        let submit = engine.compose_submit(&root, Size::new(320.0, 240.0));

        assert!(matches!(submit, SceneSubmit::Full(_)));
    }

    #[test]
    fn compose_submit_reconciles_keyed_rebuild_as_paint_patch() {
        let first = column(vec![text("Title").key("title"), text("Body").key("body")])
            .spacing(4.0)
            .key("root");
        let second = column(vec![
            text("Title").key("title").foreground(Color::WHITE),
            text("Body").key("body"),
        ])
        .spacing(4.0)
        .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                assert_eq!(patch.upserts.len(), 1);
                assert!(patch.removes.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
        assert_eq!(engine.stats().layout_passes, 1);
        assert_eq!(engine.stats().compose_passes, 2);
    }

    #[test]
    fn compose_submit_keeps_keyed_structure_removal_on_patch_path() {
        let first = column(vec![text("Title").key("title"), text("Body").key("body")])
            .spacing(4.0)
            .key("root");
        let removed_id = text("Body").key("body").id().0;
        let second = column(vec![text("Title").key("title")])
            .spacing(4.0)
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                assert!(patch.layer_removes.is_empty());
                assert!(patch.removes.contains(&removed_id));
                assert!(
                    current
                        .blocks
                        .iter()
                        .all(|block| block.node_id != removed_id)
                );
                assert!(
                    current
                        .blocks
                        .iter()
                        .any(|block| block.node_id == text("Title").key("title").id().0)
                );
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
        assert_eq!(engine.stats().layout_passes, 2);
    }

    #[test]
    fn keyed_structure_insert_with_layer_stays_on_patch_path() {
        let first = column(vec![text("Base").key("base")])
            .spacing(4.0)
            .key("root");
        let inserted = container(text("Overlay").key("overlay-text"))
            .key("overlay")
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer();
        let inserted_layer_id = inserted.id().0;
        let inserted_text_id = text("Overlay").key("overlay-text").id().0;
        let second = column(vec![text("Base").key("base"), inserted])
            .spacing(4.0)
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                assert!(
                    patch
                        .layer_upserts
                        .iter()
                        .any(|layer| layer.layer_id == inserted_layer_id)
                );
                assert!(
                    patch
                        .upserts
                        .iter()
                        .any(|block| block.node_id == inserted_text_id)
                );
                assert!(
                    current
                        .layers
                        .iter()
                        .any(|layer| layer.layer_id == inserted_layer_id)
                );
                let text_block = current
                    .blocks
                    .iter()
                    .find(|block| block.node_id == inserted_text_id)
                    .expect("inserted text block");
                assert_eq!(text_block.layer_id, inserted_layer_id);
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn compose_submit_keeps_text_baseline_in_sync_with_text_metrics() {
        let root = text("Baseline").font_size(20.0).padding_all(10.0);
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        match &scene.commands[0] {
            DrawCommand::Text {
                position, layout, ..
            } => {
                assert_eq!(position.y, 10.0 + layout.metrics.ascent);
                assert!(layout.metrics.ascent > 0.0);
                assert!(layout.metrics.descent >= 0.0);
            }
            _ => panic!("expected text command"),
        }
    }

    #[test]
    fn compose_submit_reconciles_keyed_layout_change_as_layout_work() {
        let first = column(vec![text("Title").key("title"), text("Body").key("body")])
            .spacing(4.0)
            .key("root");
        let second = column(vec![text("Title").key("title"), text("Body").key("body")])
            .spacing(12.0)
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        assert!(matches!(submit, SceneSubmit::Patch { .. }));
        assert_eq!(engine.stats().layout_passes, 2);
    }

    #[test]
    fn layout_patch_skips_unaffected_leading_siblings() {
        let first_title = text("One").key("one");
        let first_title_id = first_title.id().0;
        let second_title = text("Two").key("two");
        let second_title_id = second_title.id().0;
        let third_title = text("Three").key("three");
        let third_title_id = third_title.id().0;
        let first = column(vec![first_title, second_title, third_title])
            .spacing(4.0)
            .key("root");
        let second = column(vec![
            text("One").key("one"),
            text("Two").key("two").font_size(32.0),
            text("Three").key("three"),
        ])
        .spacing(4.0)
        .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                let upsert_ids: Vec<u64> =
                    patch.upserts.iter().map(|block| block.node_id).collect();
                assert!(!upsert_ids.contains(&first_title_id));
                assert!(upsert_ids.contains(&second_title_id));
                assert!(upsert_ids.contains(&third_title_id));
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn fixed_container_layout_change_stays_local_to_subtree() {
        let first = row(vec![
            container(text("Short").key("text"))
                .key("card")
                .padding_all(8.0)
                .width(140.0)
                .height(48.0)
                .background(Color::WHITE),
            text("Stable").key("sibling"),
        ])
        .spacing(8.0)
        .key("root");
        let second = row(vec![
            container(text("A much longer body").key("text"))
                .key("card")
                .padding_all(8.0)
                .width(140.0)
                .height(48.0)
                .background(Color::WHITE),
            text("Stable").key("sibling"),
        ])
        .spacing(8.0)
        .key("root");
        let sibling_id = text("Stable").key("sibling").id().0;
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                let upsert_ids: Vec<u64> =
                    patch.upserts.iter().map(|block| block.node_id).collect();
                assert!(!upsert_ids.contains(&sibling_id), "upserts={upsert_ids:?}");
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn keyed_reorder_stays_on_layout_patch_path() {
        let first = column(vec![
            text("One").key("one"),
            text("Two").key("two"),
            text("Three").key("three"),
        ])
        .spacing(4.0)
        .key("root");
        let second = column(vec![
            text("Three").key("three"),
            text("One").key("one"),
            text("Two").key("two"),
        ])
        .spacing(4.0)
        .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                let reordered_ids: Vec<u64> = patch
                    .reorders
                    .iter()
                    .map(|reorder| reorder.node_id)
                    .collect();
                assert_eq!(reordered_ids.len(), 3);
                assert!(reordered_ids.contains(&text("One").key("one").id().0));
                assert!(reordered_ids.contains(&text("Two").key("two").id().0));
                assert!(reordered_ids.contains(&text("Three").key("three").id().0));
                assert!(patch.upserts.len() <= 3);
                for reorder in &patch.reorders {
                    let current_block = current
                        .blocks
                        .iter()
                        .find(|block| block.node_id == reorder.node_id)
                        .expect("current reordered block");
                    assert_eq!(reorder.order, current_block.order);
                }
                assert!(patch.removes.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected reorder to stay on patch submit"),
        }
    }

    #[test]
    fn keyed_layer_reorder_uses_layer_order_patch() {
        let first = column(vec![
            container(text("One").key("one-text"))
                .key("one")
                .padding_all(8.0)
                .background(Color::WHITE)
                .opacity(0.5)
                .layer(),
            container(text("Two").key("two-text"))
                .key("two")
                .padding_all(8.0)
                .background(Color::WHITE)
                .opacity(0.5)
                .layer(),
        ])
        .spacing(4.0)
        .key("root");
        let second = column(vec![
            container(text("Two").key("two-text"))
                .key("two")
                .padding_all(8.0)
                .background(Color::WHITE)
                .opacity(0.5)
                .layer(),
            container(text("One").key("one-text"))
                .key("one")
                .padding_all(8.0)
                .background(Color::WHITE)
                .opacity(0.5)
                .layer(),
        ])
        .spacing(4.0)
        .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                let reordered_ids: Vec<u64> = patch
                    .layer_reorders
                    .iter()
                    .map(|reorder| reorder.layer_id)
                    .collect();
                assert_eq!(reordered_ids.len(), 2);
                assert!(
                    reordered_ids.contains(
                        &container(text("One").key("one-text"))
                            .key("one")
                            .padding_all(8.0)
                            .background(Color::WHITE)
                            .opacity(0.5)
                            .layer()
                            .id()
                            .0
                    )
                );
                assert!(
                    reordered_ids.contains(
                        &container(text("Two").key("two-text"))
                            .key("two")
                            .padding_all(8.0)
                            .background(Color::WHITE)
                            .opacity(0.5)
                            .layer()
                            .id()
                            .0
                    )
                );
                for reorder in &patch.layer_reorders {
                    let current_layer = current
                        .layers
                        .iter()
                        .find(|layer| layer.layer_id == reorder.layer_id)
                        .expect("current reordered layer");
                    assert_eq!(reorder.order, current_layer.order);
                }
                assert!(patch.layer_upserts.len() <= 2);
                assert!(patch.removes.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected reorder to stay on patch submit"),
        }
    }

    #[test]
    fn paint_only_patch_updates_only_dirty_blocks() {
        let title = text("Title").key("title");
        let title_id = title.id().0;
        let body = text("Body").key("body");
        let body_id = body.id().0;
        let first = column(vec![title.foreground(Color::WHITE), body])
            .spacing(4.0)
            .key("root");
        let second = column(vec![
            text("Title")
                .key("title")
                .foreground(Color::rgba(255, 220, 120, 255)),
            text("Body").key("body"),
        ])
        .spacing(4.0)
        .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                let upsert_ids: Vec<u64> =
                    patch.upserts.iter().map(|block| block.node_id).collect();
                assert_eq!(upsert_ids, vec![title_id]);
                assert!(!upsert_ids.contains(&body_id));
                assert!(patch.removes.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn modifier_chain_resolves_same_style_as_legacy_builder_api() {
        let via_builder = text("Hello")
            .padding_all(12.0)
            .background(Color::WHITE)
            .foreground(Color::rgba(10, 20, 30, 255))
            .corner_radius(8.0)
            .width(120.0)
            .height(40.0);
        let via_modifiers = text("Hello").modifiers([
            Modifier::Padding(EdgeInsets::all(12.0)),
            Modifier::Background(Color::WHITE),
            Modifier::Foreground(Color::rgba(10, 20, 30, 255)),
            Modifier::CornerRadius(8.0),
            Modifier::Width(120.0),
            Modifier::Height(40.0),
        ]);

        assert_eq!(via_builder.resolved_style(), via_modifiers.resolved_style());
        assert_eq!(
            via_builder.modifiers.resolve_style(),
            via_modifiers.resolved_style()
        );
    }

    #[test]
    fn modifier_api_builds_same_scene_as_legacy_style_api() {
        let via_builder = container(text("Hello").foreground(Color::WHITE).key("text"))
            .key("root")
            .padding_all(12.0)
            .background(Color::rgba(39, 110, 241, 255))
            .corner_radius(18.0);
        let via_modifier = container(
            text("Hello")
                .modifier(Modifier::Foreground(Color::WHITE))
                .key("text"),
        )
        .key("root")
        .modifiers([
            Modifier::Padding(EdgeInsets::all(12.0)),
            Modifier::Background(Color::rgba(39, 110, 241, 255)),
            Modifier::CornerRadius(18.0),
        ]);

        let builder_scene =
            compose_scene(&via_builder, Size::new(320.0, 240.0), &FallbackTextSystem);
        let modifier_scene =
            compose_scene(&via_modifier, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert_eq!(builder_scene, modifier_scene);
    }

    #[test]
    fn clip_and_transform_modifiers_emit_structured_scene_state() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .clip_rounded(12.0)
            .translate(16.0, 24.0)
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let layer = scene
            .layers
            .iter()
            .find(|layer| layer.layer_id == root.id().0)
            .expect("layer");
        let block = &scene.blocks[0];

        assert_eq!(layer.transform, Transform2D::translation(16.0, 24.0));
        assert_eq!(
            layer.clip,
            Some(SceneClip::RoundedRect {
                rect: layer.local_bounds,
                radius: 12.0,
            })
        );
        assert_eq!(block.bounds, layer.transform.map_rect(layer.local_bounds));
    }

    #[test]
    fn scale_and_rotate_modifiers_emit_affine_transform_bounds() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .scale(2.0, 1.5)
            .rotate_degrees(90.0)
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let layer = scene
            .layers
            .iter()
            .find(|layer| layer.layer_id == root.id().0)
            .expect("layer");
        let expected_transform =
            Transform2D::scale(2.0, 1.5).then(Transform2D::rotation_degrees(90.0));

        assert_eq!(layer.transform, expected_transform);
        assert_eq!(
            layer.bounds,
            expected_transform.map_rect(layer.local_bounds)
        );
    }

    #[test]
    fn transform_origin_changes_affine_transform_pivot() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .transform_origin(0.5, 0.5)
            .rotate_degrees(90.0)
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let layer = scene
            .layers
            .iter()
            .find(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
            .expect("layer");
        let pivot = Transform2D::translation(
            -layer.local_bounds.size.width * 0.5,
            -layer.local_bounds.size.height * 0.5,
        )
        .then(Transform2D::rotation_degrees(90.0))
        .then(Transform2D::translation(
            layer.local_bounds.size.width * 0.5,
            layer.local_bounds.size.height * 0.5,
        ));

        assert_eq!(layer.transform, pivot);
    }

    #[test]
    fn opacity_and_layer_modifiers_create_compositor_layer() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer()
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let layer = scene
            .layers
            .iter()
            .find(|layer| layer.layer_id == root.id().0)
            .expect("opacity layer");

        assert_eq!(layer.opacity, 0.5);
        assert!(layer.offscreen);
        assert!(
            scene
                .blocks
                .iter()
                .all(|block| block.layer_id == root.id().0)
        );
    }

    #[test]
    fn effect_modifiers_emit_layer_blend_and_effect_stack() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .blend_mode(BlendMode::Multiply)
            .blur(6.0)
            .drop_shadow(4.0, 6.0, 8.0, Color::rgba(0, 0, 0, 120))
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let layer = scene
            .layers
            .iter()
            .find(|layer| layer.layer_id == root.id().0)
            .expect("effect layer");

        assert_eq!(layer.blend_mode, SceneBlendMode::Multiply);
        assert_eq!(
            layer.effects,
            vec![
                SceneEffect::Blur { sigma: 6.0 },
                SceneEffect::DropShadow {
                    dx: 4.0,
                    dy: 6.0,
                    blur: 8.0,
                    color: Color::rgba(0, 0, 0, 120),
                },
            ]
        );
        assert_eq!(
            layer.bounds,
            zeno_core::Rect::new(
                layer.local_bounds.origin.x - 38.0,
                layer.local_bounds.origin.y - 36.0,
                layer.local_bounds.size.width + 84.0,
                layer.local_bounds.size.height + 84.0,
            )
        );
        assert!(layer.offscreen);
    }

    #[test]
    fn layer_creating_paint_change_emits_direct_layer_patch() {
        let first = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .key("root");
        let root_id = first.id().0;
        let second = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer()
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                assert!(
                    patch
                        .layer_upserts
                        .iter()
                        .any(|layer| layer.layer_id == root_id)
                );
                assert!(patch.layer_removes.is_empty());
                assert!(patch.upserts.iter().any(|block| block.node_id == root_id));
                assert!(current.layers.iter().any(|layer| layer.layer_id == root_id));
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn adding_layer_rehomes_descendant_blocks_in_patch() {
        let first = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .key("root");
        let root_id = first.id().0;
        let text_id = text("Hello").key("text").id().0;
        let second = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer()
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                let text_block = patch
                    .upserts
                    .iter()
                    .find(|block| block.node_id == text_id)
                    .expect("text block upsert");
                assert_eq!(text_block.layer_id, root_id);
                let current_text_block = current
                    .blocks
                    .iter()
                    .find(|block| block.node_id == text_id)
                    .expect("current text block");
                assert_eq!(current_text_block.layer_id, root_id);
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn removing_layer_rehomes_descendant_blocks_in_patch() {
        let first = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer()
            .key("root");
        let text_id = text("Hello").key("text").id().0;
        let second = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, current } => {
                let text_block = patch
                    .upserts
                    .iter()
                    .find(|block| block.node_id == text_id)
                    .expect("text block upsert");
                assert_eq!(text_block.layer_id, Scene::ROOT_LAYER_ID);
                let current_text_block = current
                    .blocks
                    .iter()
                    .find(|block| block.node_id == text_id)
                    .expect("current text block");
                assert_eq!(current_text_block.layer_id, Scene::ROOT_LAYER_ID);
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn layer_effect_change_emits_direct_layer_upsert() {
        let first = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .layer()
            .key("root");
        let root_id = first.id().0;
        let second = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .layer()
            .blend_mode(BlendMode::Multiply)
            .drop_shadow(4.0, 6.0, 8.0, Color::rgba(0, 0, 0, 120))
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                let layer = patch
                    .layer_upserts
                    .iter()
                    .find(|layer| layer.layer_id == root_id)
                    .expect("layer upsert");
                assert_eq!(layer.blend_mode, SceneBlendMode::Multiply);
                assert_eq!(
                    layer.effects,
                    vec![SceneEffect::DropShadow {
                        dx: 4.0,
                        dy: 6.0,
                        blur: 8.0,
                        color: Color::rgba(0, 0, 0, 120),
                    }]
                );
                assert!(layer.offscreen);
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn dump_helpers_report_scene_and_layout_structure() {
        let root = container(text("Hello").key("text"))
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer()
            .key("root");
        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
        let scene_dump = dump_scene(&scene);
        let layout_dump = dump_layout(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert!(scene_dump.contains("layer id="));
        assert!(scene_dump.contains("blend="));
        assert!(layout_dump.contains("node id="));
        assert!(layout_dump.contains("text lines="));
    }
}
