use super::super::*;

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
