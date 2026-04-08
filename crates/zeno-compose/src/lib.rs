mod layout;
mod node;
mod render;
mod style;
mod widgets;
mod invalidation;
mod tree;

pub use invalidation::{DirtyFlags, DirtyReason};
pub use node::{Node, NodeKind, SpacerNode, TextNode};
pub use node::NodeId;
pub use render::{compose_scene, ComposeEngine, ComposeRenderer, ComposeStats};
pub use style::{Axis, EdgeInsets, Style};
pub use widgets::{column, container, row, spacer, text};

#[cfg(test)]
mod tests {
    use super::{column, compose_scene, container, row, spacer, text, ComposeEngine, DirtyReason};
    use zeno_core::{Color, Size};
    use zeno_graphics::{DrawCommand, SceneSubmit};
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
    fn compose_submit_tracks_removed_blocks_for_keyed_rebuilds() {
        let first = column(vec![text("Title").key("title"), text("Body").key("body")])
            .spacing(4.0)
            .key("root");
        let second = column(vec![text("Title").key("title")])
            .spacing(4.0)
            .key("root");
        let mut engine = ComposeEngine::new(&FallbackTextSystem);

        let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
        let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

        match submit {
            SceneSubmit::Patch { patch, .. } => {
                assert_eq!(patch.removes, vec![text("Body").key("body").id().0]);
                assert!(!patch.upserts.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
        assert_eq!(engine.stats().layout_passes, 2);
    }
}
